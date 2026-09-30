//! 主循环：heartbeat → poll → 逐个 exec → result，定时一轮。
//! 收到 SIGTERM / ctrl-c 后优雅退出（当前任务做完才停，不中断执行中的 LGSM）。

use std::time::Duration;

use anyhow::Result;

use crate::backend::{BackendClient, JobResult, PowerJob};
use crate::lgsm::{run_power, validate_instance};

pub struct Agent {
    client: BackendClient,
    lgsm_dir: String,
    instances: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RunSummary {
    pub jobs: usize,
    pub succeeded: usize,
    pub failed: usize,
}

impl Agent {
    pub fn new(client: BackendClient, lgsm_dir: String, instances: Vec<String>) -> Self {
        Self {
            client,
            lgsm_dir,
            instances,
        }
    }

    /// 单轮：上报存活 → 拉任务 → 逐个执行并回写。
    /// 任务间串行，避免并发重启同一宿主机上的多个服。
    pub async fn run_once(&self, hostname: &str) -> Result<RunSummary> {
        self.client.heartbeat(hostname, &self.instances).await?;
        let jobs = self.client.poll_jobs(5).await?;
        let mut summary = RunSummary {
            jobs: jobs.len(),
            ..Default::default()
        };
        for job in &jobs {
            match self.handle_job(job).await {
                Ok(()) => summary.succeeded += 1,
                Err(error) => {
                    summary.failed += 1;
                    tracing::warn!(job_id = %job.job_id, "任务执行失败：{:#}", error);
                }
            }
        }
        Ok(summary)
    }

    async fn handle_job(&self, job: &PowerJob) -> Result<()> {
        validate_instance(&job.instance, &self.instances)?;
        let output = run_power(&self.lgsm_dir, &job.instance, job.action).await?;
        if output.timed_out {
            tracing::warn!(
                job_id = %job.job_id,
                instance = %job.instance,
                "电源执行超时，已上报 timeout"
            );
        }
        self.client
            .report_result(&job.job_id, &JobResult::from(&output))
            .await?;
        Ok(())
    }

    /// 常驻循环：固定间隔跑单轮；单轮失败记 warn 不退出，下轮继续。
    pub async fn run_forever(&self, hostname: &str, interval: Duration) -> Result<()> {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = shutdown_signal() => {
                    tracing::info!("收到退出信号，停止主循环");
                    return Ok(());
                }
                _ = ticker.tick() => {
                    match self.run_once(hostname).await {
                        Ok(summary) if summary.jobs > 0 => {
                            tracing::info!(
                                jobs = summary.jobs,
                                succeeded = summary.succeeded,
                                failed = summary.failed,
                                "轮询周期完成"
                            );
                        }
                        Ok(_) => {}
                        Err(error) => tracing::warn!("轮询周期失败：{:#}", error),
                    }
                }
            }
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("注册 SIGTERM 监听失败");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
