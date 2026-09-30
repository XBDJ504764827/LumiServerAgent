# LumiServerAgent 架构

> 详细需求见 LumiAdmin 仓库 `docs/host-agent-control.md`，本文只记录本仓库侧的形态与接口契约。

## 决策

- 每台游戏宿主机 1 个 Agent 进程，管理本机 N 个 LGSM 实例（如 `csgoserver`、`csgo2server`）。
- 拉模式为主 + 主动上报为辅：`heartbeat` → `poll` → `exec` → `result`，默认每 10s 一轮。
- 仅电源操作：`restart` / `start` / `stop`，不做任意命令执行。
- 安装包由网站 `Agent控制` 页托管下发，不走 GitHub。

## 接口契约（网站后端按此实现）

Agent 一律 `Authorization: Bearer <agent_token>`，JSON 通信，控制面请求超时 15s。

| 方向 | 方法与路径 | 请求体 | 成功响应 |
|------|-----------|--------|----------|
| 上报 | POST `/api/host-agent/heartbeat` | `{hostname, instances: [...]}` | 2xx 即可 |
| 拉任务 | POST `/api/host-agent/jobs/poll` | `{max}` | `{jobs: [{job_id, instance, action}]}` |
| 回结果 | POST `/api/host-agent/jobs/:id/result` | `{exit_code, output, timed_out}` | 2xx 即可 |

非法任务（未知动作/非法实例名/缺 job_id）由 Agent 记 warn 跳过，不堵队列；
任务是否登记由执行前的 `validate_instance` 把关。

## 实现进度

- [x] `feat:` 配置加载（`/etc/lumi-agent.env` + 环境变量 + 实例清单校验）
- [x] `feat:` heartbeat 上报存活 + 实例清单
- [x] `feat:` poll 拉取电源任务
- [x] `feat:` LGSM allowlist 执行（argv、不走 shell、超时杀掉、输出截断）
- [x] `feat:` result 回写执行结果
- [x] `test:` allowlist 合法/非法用例
- [ ] `feat:` 主循环 + SIGTERM 优雅退出 + systemd 单元
- [ ] `chore:` 一键安装脚本 + CI 流水线
