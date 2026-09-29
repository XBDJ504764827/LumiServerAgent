use anyhow::Result;

/// LGSM 电源操作允许名单（占位，后续实现 argv 执行与超时杀掉）。
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
}

/// TODO(feat): 校验 instance 名称合法性（正则 + 登记清单双检）。
pub fn validate_instance(_instance: &str, _allowlist: &[String]) -> Result<()> {
    anyhow::bail!("TODO: instance allowlist 尚未实现")
}

/// TODO(feat): 以 argv 方式执行 `./<instance> <action>`，cwd 为 LGSM 目录，不走 shell，超时杀掉并截断输出。
pub async fn run_power(
    _lgsm_dir: &str,
    _instance: &str,
    _action: PowerAction,
) -> Result<(i32, String)> {
    anyhow::bail!("TODO: LGSM 电源执行尚未实现")
}
