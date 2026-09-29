# LumiServerAgent 架构（占位）

> 详细需求见 LumiAdmin 仓库 `docs/host-agent-control.md`，本文只记录本仓库侧的形态，功能后续实现。

## 决策

- 每台游戏宿主机 1 个 Agent 进程，管理本机 N 个 LGSM 实例（如 `csgoserver`、`csgo2server`）。
- 拉模式为主 + 主动上报为辅：`heartbeat` → `poll` → `exec` → `result`，约每 10s 一轮。
- 仅电源操作：`restart` / `start` / `stop`，不做任意命令执行。
- 安装包由网站 `Agent控制` 页托管下发，不走 GitHub。

## 后续实现清单（占位）

- [x] `feat:` 配置加载（`/etc/lumi-agent.env` + 环境变量 + 实例清单校验）
- [ ] `feat:` heartbeat 上报存活 + 实例清单
- [ ] `feat:` poll 拉取电源任务
- [ ] `feat:` LGSM allowlist 执行（argv、不走 shell、超时杀掉、输出截断）
- [ ] `feat:` result 回写执行结果
- [ ] `feat:` SIGTERM 优雅退出 + systemd 单元
- [ ] `test:` allowlist 合法/非法用例
