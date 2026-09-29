#!/usr/bin/env bash
# LumiServerAgent 一键安装脚本（占位）。
# 注意：安装包由网站 `Agent控制` 页直接下发，本脚本不从 GitHub 拉取。
set -euo pipefail

# TODO(feat): 参数解析 --url（网站后端地址） --token（Agent 口令） --dir（安装目录）。
# TODO(feat): 从 "$BACKEND_URL/api/host-agent/download/..." 下载 agent 二进制/service 文件。
# TODO(feat): 写入 /etc/lumi-agent.env（600 权限）并 systemctl enable --now lumi-host-agent。

echo "TODO: install.sh 尚未实现，仅占位。"
