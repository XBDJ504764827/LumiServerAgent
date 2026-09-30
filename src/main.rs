use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use lumi_server_agent::agent::Agent;
use lumi_server_agent::backend::{local_hostname, BackendClient};
use lumi_server_agent::config::{Config, Overrides};

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

    /// 轮询间隔（秒），默认 10
    #[arg(long, env = "LUMI_POLL_INTERVAL_SECS", default_value_t = 10)]
    poll_interval_secs: u64,
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

    let client = BackendClient::new(&cfg.backend_url, &cfg.agent_token)?;
    let agent = Agent::new(client, cfg.lgsm_dir.clone(), cfg.instances.clone());
    tracing::info!(
        backend = %cfg.backend_url,
        lgsm_dir = %cfg.lgsm_dir,
        instances = ?cfg.instances,
        interval_secs = args.poll_interval_secs.max(1),
        "agent 启动"
    );
    agent
        .run_forever(
            &local_hostname(),
            Duration::from_secs(args.poll_interval_secs.max(1)),
        )
        .await
}
