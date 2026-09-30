//! 主循环端到端测试：桩后端（标准库 TCP）+ 假 LGSM 脚本，
//! 验证 heartbeat → poll → exec → result 一轮能跑通。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};

use lumi_server_agent::agent::{Agent, RunSummary};
use lumi_server_agent::backend::BackendClient;

static SEQ: AtomicU64 = AtomicU64::new(0);

/// 启动只回 200 的桩后端：按请求路径返回固定报文，处理完 `expect` 个连接后退出。
/// 返回 (backend_url, 收到的请求行列表)。
fn start_stub(poll_body: String, expect: usize) -> (String, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut paths = Vec::new();
        for stream in listener.incoming().take(expect) {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(value) = line
                    .trim()
                    .to_ascii_lowercase()
                    .strip_prefix("content-length:")
                {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            if content_length > 0 {
                let mut body = vec![0u8; content_length];
                reader.read_exact(&mut body).unwrap();
            }
            paths.push(request_line.trim().to_string());
            let resp_body = if request_line.contains("/jobs/poll") {
                poll_body.clone()
            } else {
                "{}".to_string()
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
        paths
    });
    (url, handle)
}

/// 建一个装着假 LGSM 脚本的临时目录；脚本把收到的动作追加到 log 文件。
fn fake_lgsm_dir(log_path: &str) -> String {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("lumi-agent-loop-{}-{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("csgoserver"),
        format!("#!/bin/sh\necho \"$1\" >> \"{}\"\nexit 0\n", log_path),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.join("csgoserver"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    dir.to_string_lossy().into_owned()
}

#[tokio::test]
async fn run_once_executes_polled_job_end_to_end() {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let log = std::env::temp_dir().join(format!("lumi-invoked-{}-{}.log", std::process::id(), seq));
    let dir = fake_lgsm_dir(log.to_str().unwrap());
    let poll_body =
        r#"{"jobs": [{"job_id": "job-1", "instance": "csgoserver", "action": "restart"}]}"#
            .to_string();
    let (url, stub) = start_stub(poll_body, 3);

    let client = BackendClient::new(&url, "test-token").unwrap();
    let agent = Agent::new(client, dir, vec!["csgoserver".to_string()]);
    let summary: RunSummary = agent.run_once("test-host").await.unwrap();

    assert_eq!(
        summary,
        RunSummary {
            jobs: 1,
            succeeded: 1,
            failed: 0
        }
    );
    let invoked = std::fs::read_to_string(&log).unwrap();
    assert!(
        invoked.lines().any(|line| line.trim() == "restart"),
        "假 LGSM 应收到 restart，实际：{}",
        invoked
    );
    let paths = stub.join().unwrap();
    assert_eq!(paths.len(), 3);
    assert!(
        paths[0].contains("/api/host-agent/heartbeat"),
        "实际：{:?}",
        paths
    );
    assert!(
        paths[1].contains("/api/host-agent/jobs/poll"),
        "实际：{:?}",
        paths
    );
    assert!(
        paths[2].contains("/api/host-agent/jobs/job-1/result"),
        "实际：{:?}",
        paths
    );
}

#[tokio::test]
async fn run_once_surfaces_backend_failure() {
    let client = BackendClient::new("http://127.0.0.1:1", "test-token").unwrap();
    let agent = Agent::new(client, "/tmp".to_string(), vec![]);
    assert!(agent.run_once("test-host").await.is_err());
}
