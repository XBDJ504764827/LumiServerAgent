use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};

/// LGSM 电源操作默认超时（秒）：LGSM stop/start 常需数十秒，120s 留有余量。
pub const POWER_TIMEOUT_SECS: u64 = 120;
/// 回传输出上限（字节）：超限按字符边界截断并打标记，避免大输出打爆任务表。
pub const MAX_OUTPUT_BYTES: usize = 8 * 1024;

/// LGSM 电源操作允许名单：仅 restart/start/stop，不做任意命令执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Restart,
    Start,
    Stop,
}

impl PowerAction {
    pub fn as_arg(self) -> &'static str {
        match self {
            Self::Restart => "restart",
            Self::Start => "start",
            Self::Stop => "stop",
        }
    }

    /// 解析任务下发的动作字符串；大小写不敏感，前后空白忽略。
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "restart" => Ok(Self::Restart),
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            _ => anyhow::bail!("不支持的电源动作：{}", raw.trim()),
        }
    }
}

/// 电源执行结果：超时也作为正常结果返回（timed_out=true），
/// 由调用方上报为任务超时，而非当成 Agent 崩溃。
#[derive(Debug, Clone)]
pub struct PowerOutput {
    pub exit_code: i32,
    pub output: String,
    pub timed_out: bool,
}

/// 校验实例名：正则合法 + 已在该 Agent 登记清单内，两项缺一不可。
/// 正则规则与配置加载共用 [`crate::config::is_valid_instance_name`]，
/// 防止 `../` 跳目录与 shell 元字符注入。
pub fn validate_instance(instance: &str, allowlist: &[String]) -> Result<()> {
    anyhow::ensure!(
        crate::config::is_valid_instance_name(instance),
        "非法实例名：{}",
        instance
    );
    anyhow::ensure!(
        allowlist.iter().any(|name| name == instance),
        "实例未在该 Agent 登记：{}",
        instance
    );
    Ok(())
}

/// 以 argv 方式执行 `./<instance> <action>`：
/// cwd 为 LGSM 目录，不走 shell；stdout/stderr 合并截断后返回；
/// 超时见 [`POWER_TIMEOUT_SECS`]。
/// 调用方须先做 [`validate_instance`]（主循环在领到任务后校验），
/// 本函数只做正则兜底，不检查登记清单。
pub async fn run_power(lgsm_dir: &str, instance: &str, action: PowerAction) -> Result<PowerOutput> {
    run_power_with_timeout(
        lgsm_dir,
        instance,
        action,
        Duration::from_secs(POWER_TIMEOUT_SECS),
    )
    .await
}

/// 可注入超时的执行入口（供测试用小超时覆盖超时分支）。
pub async fn run_power_with_timeout(
    lgsm_dir: &str,
    instance: &str,
    action: PowerAction,
    timeout: Duration,
) -> Result<PowerOutput> {
    anyhow::ensure!(
        crate::config::is_valid_instance_name(instance),
        "非法实例名：{}",
        instance
    );
    let child = tokio::process::Command::new(format!("./{}", instance))
        .arg(action.as_arg())
        .current_dir(lgsm_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("启动 LGSM 实例失败：{}（目录 {}）", instance, lgsm_dir))?;

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok(PowerOutput {
            exit_code: output.status.code().unwrap_or(-1),
            output: truncate_output(&merge_output(&output)),
            timed_out: false,
        }),
        Ok(Err(error)) => Err(anyhow::anyhow!("读取 LGSM 输出失败：{:#}", error)),
        Err(_) => {
            // 超时：child 已随超时的 future 被 drop，kill_on_drop 负责终止进程。
            // 残留输出不再回收，直接标记超时正常返回，由调用方上报为任务超时。
            Ok(PowerOutput {
                exit_code: -1,
                output: "[timeout, 执行超时已终止]".to_string(),
                timed_out: true,
            })
        }
    }
}

fn merge_output(output: &std::process::Output) -> String {
    let mut merged = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        if !merged.is_empty() && !merged.ends_with('\n') {
            merged.push('\n');
        }
        merged.push_str("[stderr]\n");
        merged.push_str(&stderr);
    }
    merged
}

fn truncate_output(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…\n[truncated, 输出超限已截断]", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    /// 建一个装着假 LGSM 脚本的临时目录，返回目录路径。
    /// 临时目录不删除，/tmp 由系统回收，避免测试间竞争 unlink。
    fn fake_lgsm_dir(script_name: &str, script_body: &str) -> String {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("lumi-lgsm-test-{}-{}", std::process::id(), seq));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(script_name);
        std::fs::write(&path, script_body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir.to_string_lossy().into_owned()
    }

    #[test]
    fn parse_accepts_power_actions_case_insensitive() {
        assert_eq!(PowerAction::parse("restart").unwrap(), PowerAction::Restart);
        assert_eq!(PowerAction::parse("START").unwrap(), PowerAction::Start);
        assert_eq!(PowerAction::parse("  stop  ").unwrap(), PowerAction::Stop);
    }

    #[test]
    fn parse_rejects_non_power_commands() {
        for bad in ["", "status", "restart now", "rm -rf /", "details"] {
            assert!(PowerAction::parse(bad).is_err(), "应拒绝：{}", bad);
        }
    }

    #[test]
    fn validate_accepts_registered_instance() {
        let allow = vec!["csgoserver".to_string(), "csgo2server".to_string()];
        validate_instance("csgoserver", &allow).unwrap();
        validate_instance("csgo2server", &allow).unwrap();
    }

    #[test]
    fn validate_rejects_injection_and_unregistered() {
        let allow = vec!["csgoserver".to_string()];
        let long = "a".repeat(33);
        for evil in [
            "a;reboot",
            "$(reboot)",
            "../csgoserver",
            "csgo server",
            "",
            long.as_str(),
        ] {
            let err = validate_instance(evil, &allow).unwrap_err();
            assert!(err.to_string().contains("非法实例名"), "应拦截：{}", evil);
        }
        let err = validate_instance("csgo3server", &allow).unwrap_err();
        assert!(err.to_string().contains("未在该 Agent 登记"));
    }

    #[tokio::test]
    async fn run_power_returns_exit_code_and_merged_output() {
        let dir = fake_lgsm_dir(
            "csgoserver",
            "#!/bin/sh\necho \"out-$1\"\necho \"err-$1\" >&2\nexit 3\n",
        );
        let out = run_power_with_timeout(
            &dir,
            "csgoserver",
            PowerAction::Restart,
            Duration::from_secs(10),
        )
        .await
        .unwrap();
        assert!(!out.timed_out);
        assert_eq!(out.exit_code, 3);
        assert!(out.output.contains("out-restart"));
        assert!(out.output.contains("err-restart"));
    }

    #[tokio::test]
    async fn run_power_reports_timeout_instead_of_hanging() {
        let dir = fake_lgsm_dir("slowserver", "#!/bin/sh\nsleep 30\n");
        let out = run_power_with_timeout(
            &dir,
            "slowserver",
            PowerAction::Stop,
            Duration::from_secs(1),
        )
        .await
        .unwrap();
        assert!(out.timed_out);
        assert_eq!(out.exit_code, -1);
    }

    #[tokio::test]
    async fn run_power_rejects_bad_instance_before_spawn() {
        let err = run_power_with_timeout(
            "/tmp",
            "a;reboot",
            PowerAction::Restart,
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("非法实例名"));
    }

    #[tokio::test]
    async fn run_power_reports_spawn_failure() {
        let err = run_power_with_timeout(
            "/nonexistent-lumi-lgsm-dir",
            "csgoserver",
            PowerAction::Restart,
            Duration::from_secs(5),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("启动 LGSM 实例失败"));
    }

    #[tokio::test]
    async fn run_power_truncates_huge_output() {
        let dir = fake_lgsm_dir(
            "noisyserver",
            "#!/bin/sh\ni=0; while [ $i -lt 20000 ]; do echo \"0123456789ABCDEF\"; i=$((i+1)); done\n",
        );
        let out = run_power_with_timeout(
            &dir,
            "noisyserver",
            PowerAction::Start,
            Duration::from_secs(15),
        )
        .await
        .unwrap();
        assert!(!out.timed_out);
        assert!(out.output.len() <= MAX_OUTPUT_BYTES + 64);
        assert!(out.output.contains("[truncated"));
    }
}
