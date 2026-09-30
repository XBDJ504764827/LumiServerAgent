# LumiServerAgent

LumiAdmin 游戏服宿主机 Agent（Rust）：一台宿主机跑一个常驻进程，管理本机全部 LGSM 实例的电源操作（`restart` / `start` / `stop`），拉模式领任务。

> 当前为目录结构 + 占位文件阶段，功能尚未实现。安装包由网站 `Agent控制` 页直接托管下发，不走 GitHub 下载。

## 目录结构

```text
LumiServerAgent/
├── Cargo.toml
├── src/
│   ├── main.rs        # 入口：参数解析 + 占位主循环
│   ├── config.rs      # 配置加载占位（后端地址/口令/LGSM目录/实例清单）
│   ├── lgsm.rs        # LGSM 电源执行占位（allowlist + argv + 超时）
│   └── backend/
│       └── mod.rs     # 后端通信占位（heartbeat/poll/result）
├── tests/
│   └── lgsm_allowlist.rs
├── deploy/
│   └── lumi-host-agent.service
├── scripts/
│   └── install.sh
├── docs/
│   └── ARCHITECTURE.md
└── .github/workflows/ci.yml
```

## 快速开始（宿主机）

```bash
# 1. 一键安装（包来自网站 Agent控制 页，不走 GitHub）
sudo bash install.sh \
  --url https://admin.example.com \
  --token <网站签发的Agent口令> \
  --lgsm-dir /home/steam/lgsm \
  --instances csgoserver,csgo2server

# 2. 看日志 / 确认在线
journalctl -u lumi-host-agent -f
```

本地构建：`cargo build --release`，二进制在 `target/release/lumi-server-agent`。

## 提交规范

使用 Conventional Commits，便于版本记录与自动化：

- `feat:` 新功能
- `fix:` 修复问题
- `docs:` 文档变更
- `chore:` 维护/依赖
- `refactor:` 重构且不改行为
- `test:` 测试相关
