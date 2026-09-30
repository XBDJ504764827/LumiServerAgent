//! Agent 运行配置：三层来源按优先级合并后校验。
//!
//! 优先级（高 → 低）：CLI 参数 > 环境变量（`LUMI_*`）> 配置文件（默认
//! `/etc/lumi-agent.env`）> 内置默认值。
//! `agent_token` 没有默认值：不提供就启动失败，避免无口令空跑。

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// 默认配置文件路径。
pub const DEFAULT_CONFIG_PATH: &str = "/etc/lumi-agent.env";
/// 内置默认后端地址（本地联调）。
pub const DEFAULT_BACKEND_URL: &str = "http://127.0.0.1:3001";
/// 内置默认 LGSM 目录。
pub const DEFAULT_LGSM_DIR: &str = "/home/steam/lgsm";

/// CLI 层覆盖值，`None` 表示该层未提供。
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    pub config_path: Option<String>,
    pub backend_url: Option<String>,
    pub agent_token: Option<String>,
    pub lgsm_dir: Option<String>,
    /// 逗号分隔的实例清单，如 `csgoserver,csgo2server`。
    pub instances: Option<String>,
}

/// 生效后的 Agent 运行配置（已校验）。
#[derive(Debug, Clone)]
pub struct Config {
    pub backend_url: String,
    pub agent_token: String,
    pub lgsm_dir: String,
    pub instances: Vec<String>,
}

fn instance_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{1,32}$").expect("实例名正则写死，应恒成立"))
}

/// 校验单个实例名（与网站侧落库前的校验对齐，防 `../../bin/sh` 类注入）。
pub fn is_valid_instance_name(name: &str) -> bool {
    instance_regex().is_match(name)
}

/// 解析 `KEY=VALUE` 文本：跳过空行与 `#` 注释，值两侧成对引号自动脱去。
pub fn parse_env_text(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        map.insert(key.to_string(), unquote(value.trim()));
    }
    map
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// 读取配置文件；文件不存在视为"未配置"（返回空表），由环境变量/CLI 补齐。
pub fn load_from_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    warn_if_permissive(path);
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
    Ok(parse_env_text(&text))
}

#[cfg(unix)]
fn warn_if_permissive(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mode = meta.permissions().mode();
        if mode & 0o077 != 0 {
            tracing::warn!(
                path = %path.display(),
                mode = format!("{:o}", mode & 0o777),
                "配置文件含口令却可被同机其他用户读取，建议 chmod 600"
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_permissive(_path: &Path) {}

/// 取文件层的值：优先 `LUMI_<KEY>`，兼容无前缀写法（与网站文档里的 `BACKEND_URL` 对齐）。
fn file_get(file: &HashMap<String, String>, key: &str) -> Option<String> {
    let prefixed = format!("LUMI_{key}");
    file.get(&prefixed).or_else(|| file.get(key)).cloned()
}

fn resolve(
    cli: Option<String>,
    env_name: &str,
    file: &HashMap<String, String>,
    file_key: &str,
    default: &str,
) -> String {
    if let Some(v) = cli {
        if !v.trim().is_empty() {
            return v;
        }
    }
    if let Ok(v) = std::env::var(env_name) {
        if !v.trim().is_empty() {
            return v;
        }
    }
    if let Some(v) = file_get(file, file_key) {
        if !v.trim().is_empty() {
            return v;
        }
    }
    default.to_string()
}

fn parse_instances(raw: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for item in raw.split(',') {
        let name = item.trim();
        if name.is_empty() {
            continue;
        }
        if !is_valid_instance_name(name) {
            bail!(
                "实例名非法: {name:?}（仅允许 1-32 位字母/数字/_/-，且必须登记在本 Agent 清单内）"
            );
        }
        if !out.iter().any(|s| s == name) {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

impl Config {
    /// 按三层优先级加载并校验；`config_path` 为空时看 `LUMI_CONFIG`，再为空用默认路径。
    pub fn load(overrides: &Overrides) -> Result<Self> {
        let config_path = overrides
            .config_path
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::env::var("LUMI_CONFIG")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());

        let file = load_from_file(Path::new(&config_path))?;

        let backend_url = resolve(
            overrides.backend_url.clone(),
            "LUMI_BACKEND_URL",
            &file,
            "BACKEND_URL",
            DEFAULT_BACKEND_URL,
        );
        let agent_token = resolve(
            overrides.agent_token.clone(),
            "LUMI_AGENT_TOKEN",
            &file,
            "AGENT_TOKEN",
            "",
        );
        let lgsm_dir = resolve(
            overrides.lgsm_dir.clone(),
            "LUMI_LGSM_DIR",
            &file,
            "LGSM_DIR",
            DEFAULT_LGSM_DIR,
        );
        let instances_raw = resolve(
            overrides.instances.clone(),
            "LUMI_INSTANCES",
            &file,
            "INSTANCES",
            "",
        );

        if agent_token.trim().is_empty() {
            bail!(
                "LUMI_AGENT_TOKEN 未设置：通过 --token、环境变量或 {config_path} 提供（由网站 Agent控制 页签发）"
            );
        }

        Ok(Self {
            backend_url,
            agent_token,
            lgsm_dir,
            instances: parse_instances(&instances_raw)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// 环境变量是进程全局的：凡是读写 `LUMI_*` 的测试都串行化，避免并行互相踩。
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    const ENV_KEYS: [&str; 5] = [
        "LUMI_BACKEND_URL",
        "LUMI_AGENT_TOKEN",
        "LUMI_LGSM_DIR",
        "LUMI_INSTANCES",
        "LUMI_CONFIG",
    ];

    /// 拿锁 + 清空 `LUMI_*`，drop 时恢复现场。
    struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        saved: Vec<(String, Option<String>)>,
    }

    impl EnvGuard {
        fn take() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut saved = Vec::new();
            for key in ENV_KEYS {
                saved.push((key.to_string(), std::env::var(key).ok()));
                std::env::remove_var(key);
            }
            Self { _lock: lock, saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.saved.drain(..) {
                match value {
                    Some(v) => std::env::set_var(&key, v),
                    None => std::env::remove_var(&key),
                }
            }
        }
    }

    #[test]
    fn parse_env_text_basic() {
        let map = parse_env_text(
            "# 注释行\n\
             \n\
             LUMI_BACKEND_URL = http://10.0.0.1:3001\n\
             AGENT_TOKEN=\"s3cret with space\"\n\
             LGSM_DIR='/home/steam/lgsm'\n\
             INSTANCES=csgoserver,csgo2server\n\
             URL_WITH_EQ=http://x/?a=b&c=d\n\
             无等号的行直接忽略\n\
             =空键忽略\n",
        );
        assert_eq!(map["LUMI_BACKEND_URL"], "http://10.0.0.1:3001");
        assert_eq!(map["AGENT_TOKEN"], "s3cret with space");
        assert_eq!(map["LGSM_DIR"], "/home/steam/lgsm");
        assert_eq!(map["INSTANCES"], "csgoserver,csgo2server");
        assert_eq!(map["URL_WITH_EQ"], "http://x/?a=b&c=d");
        assert_eq!(map.len(), 5);
    }

    #[test]
    fn instance_names_valid() {
        for name in [
            "csgoserver",
            "csgo2server",
            "a",
            "A",
            "0",
            "a-_Z09",
            &"x".repeat(32),
        ] {
            assert!(is_valid_instance_name(name), "应合法: {name}");
        }
    }

    #[test]
    fn instance_names_invalid() {
        for name in [
            "",
            &"x".repeat(33),
            "a;rm -rf /",
            "../../bin/sh",
            "../bin/sh",
            "a/b",
            "a b",
            "a.b",
            "a$b",
            "服1",
            " csgoserver",
            "csgoserver ",
        ] {
            assert!(!is_valid_instance_name(name), "应非法: {name:?}");
        }
    }

    fn write_temp_env(content: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("lumi-agent-test-{}-{id}.env", std::process::id()));
        std::fs::write(&path, content).expect("写测试配置文件");
        path
    }

    fn load_with_file(content: &str, overrides: Overrides) -> Result<Config> {
        let path = write_temp_env(content);
        let mut ov = overrides;
        ov.config_path = Some(path.to_string_lossy().into_owned());
        let result = Config::load(&ov);
        let _ = std::fs::remove_file(&path);
        result
    }

    #[test]
    fn precedence_cli_over_env_over_file() {
        let _guard = EnvGuard::take();
        std::env::set_var("LUMI_BACKEND_URL", "http://env:3001");
        std::env::set_var("LUMI_AGENT_TOKEN", "env-token");

        let cfg = load_with_file(
            "LUMI_BACKEND_URL=http://file:3001\nAGENT_TOKEN=file-token\nLUMI_LGSM_DIR=/file/lgsm\nLUMI_INSTANCES=csgoserver\n",
            Overrides {
                backend_url: Some("http://cli:3001".to_string()),
                ..Default::default()
            },
        )
        .expect("加载应成功");

        assert_eq!(cfg.backend_url, "http://cli:3001"); // CLI 胜出
        assert_eq!(cfg.agent_token, "env-token"); // 环境变量胜过文件
        assert_eq!(cfg.lgsm_dir, "/file/lgsm"); // 文件层生效
        assert_eq!(cfg.instances, vec!["csgoserver".to_string()]);
    }

    #[test]
    fn unprefixed_file_keys_supported() {
        let _guard = EnvGuard::take();
        let cfg = load_with_file(
            "BACKEND_URL=http://file:3001\nAGENT_TOKEN=file-token\nLGSM_DIR=/srv/lgsm\nINSTANCES=csgo2server\n",
            Overrides::default(),
        )
        .expect("加载应成功");
        assert_eq!(cfg.backend_url, "http://file:3001");
        assert_eq!(cfg.agent_token, "file-token");
        assert_eq!(cfg.lgsm_dir, "/srv/lgsm");
        assert_eq!(cfg.instances, vec!["csgo2server".to_string()]);
    }

    #[test]
    fn missing_file_and_defaults_ok() {
        let _guard = EnvGuard::take();
        std::env::set_var("LUMI_AGENT_TOKEN", "env-token");
        let cfg = Config::load(&Overrides {
            config_path: Some("/nonexistent/lumi-agent-test.env".to_string()),
            ..Default::default()
        })
        .expect("缺失配置文件不应报错");
        assert_eq!(cfg.backend_url, DEFAULT_BACKEND_URL);
        assert_eq!(cfg.lgsm_dir, DEFAULT_LGSM_DIR);
        assert!(cfg.instances.is_empty());
    }

    #[test]
    fn missing_token_fails_fast() {
        let _guard = EnvGuard::take();
        let err = Config::load(&Overrides {
            config_path: Some("/nonexistent/lumi-agent-test.env".to_string()),
            ..Default::default()
        })
        .expect_err("无口令应启动失败");
        assert!(err.to_string().contains("LUMI_AGENT_TOKEN"), "{err}");
    }

    #[test]
    fn invalid_instance_fails_with_name() {
        let _guard = EnvGuard::take();
        let err = load_with_file(
            "AGENT_TOKEN=t\nINSTANCES=csgoserver,evil;rm\n",
            Overrides::default(),
        )
        .expect_err("非法实例名应报错");
        let msg = err.to_string();
        assert!(msg.contains("实例名非法"), "{msg}");
        assert!(msg.contains("evil;rm"), "{msg}");
    }

    #[test]
    fn instances_trimmed_and_deduped() {
        let _guard = EnvGuard::take();
        let cfg = load_with_file(
            "AGENT_TOKEN=t\nINSTANCES= csgoserver ,csgoserver,,csgo2server \n",
            Overrides::default(),
        )
        .expect("加载应成功");
        assert_eq!(
            cfg.instances,
            vec!["csgoserver".to_string(), "csgo2server".to_string()]
        );
    }
}
