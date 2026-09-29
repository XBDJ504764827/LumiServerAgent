//! 与网站后端通信的客户端占位：heartbeat / poll / result，后续实现。

use anyhow::Result;

/// TODO(feat): POST /api/host-agent/heartbeat 上报存活 + 实例清单。
pub async fn heartbeat(_backend_url: &str, _token: &str) -> Result<()> {
    anyhow::bail!("TODO: heartbeat 尚未实现")
}

/// TODO(feat): POST /api/host-agent/jobs/poll 拉取待执行电源任务。
pub async fn poll_jobs(_backend_url: &str, _token: &str) -> Result<Vec<serde_json::Value>> {
    Ok(Vec::new())
}

/// TODO(feat): POST /api/host-agent/jobs/:id/result 回写执行结果（退出码 + 截断输出）。
pub async fn report_result(
    _backend_url: &str,
    _token: &str,
    _job_id: &str,
    _exit_code: i32,
    _output: &str,
) -> Result<()> {
    anyhow::bail!("TODO: report_result 尚未实现")
}
