mod backend;
mod config;
mod lgsm;

use anyhow::Result;
use clap::Parser;
use config::Config;

#[derive(Debug, Parser)]
#[command(name = "lumi-server-agent", version, about = "Lumi 宿主机 Agent（占位主循环，后续实现拉模式电源任务）")]
struct Args {
    /// 后端地址，如 http://127.0.0.1:3001
    #[arg(long, env = "LUMI_BACKEND_URL")]
    backend: Option<String>,

    /// Agent 口令（后续由网站 Agent控制 页签发）
    #[arg(long, env = "LUMI_AGENT_TOKEN")]
    token: Option<String>,

    /// LGSM 目录，如 /home/steam/lgsm
    #[arg(long, env = "LUMI_LGSM_DIR")]
    lgsm_dir: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args = Args::parse();
    let cfg = Config::from_args(args.backend, args.token, args.lgsm_dir)?;

    tracing::info!(
        backend = %cfg.backend_url,
        lgsm_dir = %cfg.lgsm_dir,
        instances = ?cfg.instances,
        "agent 启动（占位）：后续实现 heartbeat -> poll -> exec -> result 主循环"
    );

    // TODO(feat): 实现 heartbeat -> poll -> exec -> result 主循环。
    // TODO(feat): 实现 SIGTERM 优雅退出。
    Ok(())
}
