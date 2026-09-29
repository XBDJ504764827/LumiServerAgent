use anyhow::Result;

/// Agent 运行配置（占位，后续补全实例清单、心跳间隔、超时等）。
#[derive(Debug, Clone)]
pub struct Config {
    pub backend_url: String,
    pub agent_token: String,
    pub lgsm_dir: String,
    pub instances: Vec<String>,
}

impl Config {
    pub fn from_args(
        backend: Option<String>,
        token: Option<String>,
        lgsm_dir: Option<String>,
    ) -> Result<Self> {
        // TODO(feat): 从文件 /etc/lumi-agent.env 与环境变量合并加载；
        // TODO(feat): instances 清单校验（正则 ^[a-zA-Z0-9_-]{1,32}$）。
        Ok(Self {
            backend_url: backend.unwrap_or_else(|| "http://127.0.0.1:3001".to_string()),
            agent_token: token.unwrap_or_default(),
            lgsm_dir: lgsm_dir.unwrap_or_else(|| "/home/steam/lgsm".to_string()),
            instances: Vec::new(),
        })
    }
}
