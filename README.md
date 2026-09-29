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

## 提交规范

使用 Conventional Commits，便于版本记录与自动化：

- `feat:` 新功能
- `fix:` 修复问题
- `docs:` 文档变更
- `chore:` 维护/依赖
- `refactor:` 重构且不改行为
- `test:` 测试相关
