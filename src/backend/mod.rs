//! 与网站后端通信的客户端：heartbeat 上报存活、poll 拉取电源任务、result 回写结果。
//!
//! 接口契约（网站侧按此实现，见 docs/ARCHITECTURE.md）：
//! - POST /api/host-agent/heartbeat `{hostname, instances}` → 2xx
//! - POST /api/host-agent/jobs/poll `{max}` → `{jobs: [{job_id, instance, action}]}`

use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::lgsm::{PowerAction, PowerOutput};

/// 后端请求默认超时（秒）：控制面请求应快进快出，不与电源执行的 120s 混用。
pub const REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Clone)]
pub struct BackendClient {
    base_url: String,
    token: String,
    http: reqwest::Client,
}

impl BackendClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("创建 HTTP 客户端失败")?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        })
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> Result<reqwest::Response> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("请求后端失败：POST {}", path))?;
        let status = response.status();
        anyhow::ensure!(
            status.is_success(),
            "后端返回异常状态：{}（POST {}）",
            status,
            path
        );
        Ok(response)
    }

    /// 上报存活 + 本机实例清单；非 2xx 即报错，由主循环记 warn 后继续。
    pub async fn heartbeat(&self, hostname: &str, instances: &[String]) -> Result<()> {
        self.post(
            "/api/host-agent/heartbeat",
            serde_json::json!({ "hostname": hostname, "instances": instances }),
        )
        .await?;
        Ok(())
    }

    /// 拉取待执行电源任务；非法任务（未知动作/非法实例名）记 warn 跳过，
    /// 不让一条坏任务堵住整个队列。
    pub async fn poll_jobs(&self, max: u8) -> Result<Vec<PowerJob>> {
        let response = self
            .post(
                "/api/host-agent/jobs/poll",
                serde_json::json!({ "max": max }),
            )
            .await?;
        let body: PollResponse = response.json().await.context("解析任务列表失败")?;
        let mut jobs = Vec::with_capacity(body.jobs.len());
        for raw in body.jobs {
            match PowerJob::try_from(raw) {
                Ok(job) => jobs.push(job),
                Err(error) => tracing::warn!("跳过非法任务：{:#}", error),
            }
        }
        Ok(jobs)
    }

    /// 回写执行结果（含超时标记），由后端落为 success/failed/timeout。
    pub async fn report_result(&self, job_id: &str, result: &JobResult) -> Result<()> {
        self.post(
            &format!("/api/host-agent/jobs/{}/result", job_id),
            serde_json::json!({
                "exit_code": result.exit_code,
                "output": result.output,
                "timed_out": result.timed_out,
            }),
        )
        .await?;
        Ok(())
    }
}

/// 下发到本机的电源任务（已做动作解析与实例名形状校验；
/// 是否登记仍由执行前的 validate_instance 把关）。
#[derive(Debug, Clone)]
pub struct PowerJob {
    pub job_id: String,
    pub instance: String,
    pub action: PowerAction,
}

#[derive(Debug, Deserialize)]
struct RawJob {
    #[serde(default)]
    job_id: serde_json::Value,
    #[serde(default)]
    instance: String,
    #[serde(default)]
    action: String,
}

#[derive(Debug, Deserialize)]
struct PollResponse {
    #[serde(default)]
    jobs: Vec<RawJob>,
}

impl TryFrom<RawJob> for PowerJob {
    type Error = anyhow::Error;

    fn try_from(raw: RawJob) -> Result<Self> {
        let job_id = raw.job_id.as_str().unwrap_or("").trim().to_string();
        anyhow::ensure!(!job_id.is_empty(), "任务缺少 job_id");
        let action = PowerAction::parse(&raw.action)?;
        let instance = raw.instance.trim().to_string();
        anyhow::ensure!(
            crate::config::is_valid_instance_name(&instance),
            "任务实例名非法：{}",
            raw.instance
        );
        Ok(Self {
            job_id,
            instance,
            action,
        })
    }
}

/// 回写给后端的结果；超时照常上报，由后端记为 timeout。
#[derive(Debug, Clone)]
pub struct JobResult {
    pub exit_code: i32,
    pub output: String,
    pub timed_out: bool,
}

impl From<&PowerOutput> for JobResult {
    fn from(output: &PowerOutput) -> Self {
        Self {
            exit_code: output.exit_code,
            output: output.output.clone(),
            timed_out: output.timed_out,
        }
    }
}

/// 本机主机名：优先 HOSTNAME 环境变量，拿不到则报 unknown（后端仅作展示）。
pub fn local_hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused_client() -> BackendClient {
        // 端口 1 恒定拒绝连接：不断网也可稳定复现请求失败路径。
        BackendClient::new("http://127.0.0.1:1", "test-token").unwrap()
    }

    #[tokio::test]
    async fn requests_surface_connection_failure() {
        let client = refused_client();
        assert!(client.heartbeat("host", &[]).await.is_err());
        assert!(client.poll_jobs(5).await.is_err());
        let result = JobResult {
            exit_code: 0,
            output: String::new(),
            timed_out: false,
        };
        assert!(client.report_result("job-1", &result).await.is_err());
    }

    #[test]
    fn poll_response_converts_valid_jobs() {
        let body: PollResponse = serde_json::from_value(serde_json::json!({
            "jobs": [
                { "job_id": "a", "instance": "csgoserver", "action": "restart" },
                { "job_id": "b", "instance": " csgo2server ", "action": "STOP" },
            ]
        }))
        .unwrap();
        let jobs: Vec<PowerJob> = body
            .jobs
            .into_iter()
            .map(PowerJob::try_from)
            .collect::<Result<_>>()
            .unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].action, PowerAction::Restart);
        assert_eq!(jobs[1].instance, "csgo2server");
        assert_eq!(jobs[1].action, PowerAction::Stop);
    }

    #[test]
    fn poll_response_rejects_bad_jobs_with_reason() {
        for (job, reason) in [
            (
                serde_json::json!({ "job_id": "", "instance": "csgoserver", "action": "restart" }),
                "job_id",
            ),
            (
                serde_json::json!({ "job_id": "a", "instance": "csgoserver", "action": "status" }),
                "不支持的电源动作",
            ),
            (
                serde_json::json!({ "job_id": "a", "instance": "a;reboot", "action": "restart" }),
                "实例名非法",
            ),
        ] {
            let raw: RawJob = serde_json::from_value(job).unwrap();
            let err = PowerJob::try_from(raw).unwrap_err();
            assert!(err.to_string().contains(reason), "实际：{}", err);
        }
    }

    #[test]
    fn job_result_carries_power_output_fields() {
        let output = PowerOutput {
            exit_code: -1,
            output: "o".to_string(),
            timed_out: true,
        };
        let result = JobResult::from(&output);
        assert_eq!(result.exit_code, -1);
        assert!(result.timed_out);
    }
}
