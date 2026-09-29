mod backend;
mod config;
mod lgsm;

use anyhow::Result;
use clap::Parser;
use config::{Config, Overrides};

#[derive(Debug, Parser)]
#[command(
    name = "lumi-server-agent",
    version,
    about = "Lumi 宿主机 Agent：管理本机 LGSM 实例电源操作（拉模式）"
)]
struct Args {
    /// 后端地址，如 http://127.0.0.1:3001
    #[arg(long, env = "LUMI_BACKEND_URL")]
    backend: Option<String>,

    /// Agent 口令（由网站 Agent控制 页签发）
    #[arg(long, env = "LUMI_AGENT_TOKEN")]
    token: Option<String>,

    /// LGSM 目录，如 /home/steam/lgsm
    #[arg(long, env = "LUMI_LGSM_DIR")]
    lgsm_dir: Option<String>,

    /// 实例清单（逗号分隔），如 csgoserver,csgo2server
    #[arg(long, env = "LUMI_INSTANCES")]
    instances: Option<String>,

    /// 配置文件路径，默认 /etc/lumi-agent.env
    #[arg(long, env = "LUMI_CONFIG")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();
    let cfg = Config::load(&Overrides {
        config_path: args.config,
        backend_url: args.backend,
        agent_token: args.token,
        lgsm_dir: args.lgsm_dir,
        instances: args.instances,
    })?;

    tracing::info!(
        backend = %cfg.backend_url,
        lgsm_dir = %cfg.lgsm_dir,
        instances = ?cfg.instances,
        "agent 启动（占位）：后续实现 heartbeat -> poll -> exec -> result 主循环"
    );

    // TODO(feat, 阶段4): 实现 heartbeat -> poll -> exec -> result 主循环。
    // TODO(feat, 阶段4): 实现 SIGTERM 优雅退出。
    Ok(())
}
