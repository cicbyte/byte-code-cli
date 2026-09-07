# bcode

**简体中文 | [English](README_EN.md)**

> ByteCode 平台的 CLI——人在终端看任务、认领、回报，不打断心流开 Web；AI agent 通过 `--json` 单行契约直接消费。

![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Rust](https://img.shields.io/badge/Rust-2024%20edition-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Win%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)
[![CI](https://github.com/cicbyte/byte-code-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/cicbyte/byte-code-cli/actions/workflows/ci.yml)

<!-- screenshot: 在此处添加终端录屏（start 开工包 / tasks 列表） -->

配套平台：[byte-code](https://github.com/cicbyte/byte-code)

## 目录

- [功能特性](#-功能特性)
- [安装](#-安装)
- [快速开始](#-快速开始)
- [使用方法](#-使用方法)
- [配置](#️-配置)
- [Agent 集成](#-agent-集成)
- [终端兼容](#终端兼容)
- [发版](#发版)
- [参与贡献](#-参与贡献)
- [开源许可证](#-开源许可证)

## ✨ 功能特性

- **免参工作流** — 一次 `start` 握手后，日常命令（任务/评论/通知）不带身份与项目信息，会话承载上下文
- **双输出模式** — 人类可读表格为默认；`--json` 输出单行 compact JSON，是编排脚本与 AI 消费的稳定契约
- **退出码契约** — `0/2/3/4/5/6` 六类语义出口，脚本按码分支而非解析文本
- **会话自愈** — 缓存会话被平台废弃时自动重建重试；连接 10s / 请求 30s 超时，SSE 断线指数退避重连
- **多 agent 并行** — 身份按 profile 隔离（`--profile` / `BC_AGENT`），项目目录只有可进 git 的指针文件，换 agent 零冲突
- **全链留痕** — 认领原子化、过程日志、artifacts 回报、评论与 SSE 实时通知

从身份注册、项目接入、任务闭环，到评论通知与文档/记忆消费，命令面完整覆盖日常协作全流程。

## 📦 安装

```bash
# 从源码（当前仓库阶段）
cargo install --path .

# 发布后：cargo-binstall 直装（repository 已配置，binstall 元数据就绪）
cargo binstall bcode

# 发布后：GitHub Releases 三平台单二进制 + sha256（见发版节）
```

环境要求：Rust 1.85+（2024 edition）；运行期零外部依赖（rustls 静态链接）。

## 🚀 快速开始

```bash
$ bcode init http://127.0.0.1:8000/api     # 写 config.toml（探测连通性）
$ bcode register my-agent                  # 一次性拿 bc_ key（落盘，不再显示）
$ bcode join <接入码>                        # owner 在 Web 项目设置页生成；写 .bc/project
$ bcode start                              # 建立会话 + 开工包
```

`start` 的输出（开工包三段）：

```
  session         bcsh_dfb29c17ec4a02c5b433871c05809a2d
  project         byte-code-cli (id=4)

── 约定（1 条）──
  [global] conventions.code-style = 全局约定：Go 代码统一 gofmt 排序……

── 我的任务（0）──
  （无）

── 待审（0）──
  （无）
```

然后是日常循环：

```bash
bcode tasks                                   # 免参列表
bcode claim 42                                # 原子认领（被抢则明确失败）
bcode log 42 "进展描述"                         # 过程留痕，兼作 2h 租约心跳
bcode complete 42 --artifacts-file out.md     # 完成 → review，artifacts=markdown 产出
```

## 📖 使用方法

按协议域分组的完整命令面（`bcode --help` 一屏全貌）：

| 域 | 命令 | 说明 |
|---|---|---|
| 身份 | `register` / `whoami` / `status` / `profiles` | 注册拿 key（只此一次落盘）/ 本地概览 / 在线校验 / profile 清单 |
| 项目 | `join <code>` / `start` / `context` | 接入码换准入（写 `.bc/project`）/ 会话+开工包 / 约定原样输出 |
| 任务 | `tasks` / `task <id>` / `claim <id>` / `complete <id>` / `log <id> <msg>` | 免参列表 / 详情 / 原子认领 / 完成+artifacts / 过程留痕（租约心跳） |
| 通信 | `comment` / `comments [--follow]` / `notify [--watch]` | 评论 / 评论流增量轮询 / 通知列表与 SSE 实时流 |
| 上下文 | `docs [path]` / `memory <key>` / `memories` / `search <kw>` | 文档树与正文 / 项目记忆 / 记忆列表 / 全局搜索 |
| 便利 | `init [url]` / `open <task\|board>` / `completion <shell>` / `man` | 引导写配置 / Web 深链 / shell 补全 / 手册 |

监听实时通知（编排消费，一行一事件）：

```bash
$ bcode notify --watch --json
{"content":"项目新增未指派任务「...」","sourceId":33,"sourceType":"task","title":"新任务可认领","type":"info"}
```

## ⚙️ 配置

配置文件 `<数据根>/config.toml`：

| 键 | 说明 | 默认 |
|---|---|---|
| `server_url` | 平台地址（含 `/api` 前缀） | —（`bcode init` 引导写入） |
| `default_profile` | 默认身份 profile | `"default"` |
| `insecure` | 跳过 TLS 证书校验（自签/调试） | `false`（`--insecure` 旗标优先） |

本地布局（协议 4A：身份与项目指向正交）：

```
~/.cicbyte/apps/byte-code-cli/config.toml                    配置
~/.cicbyte/apps/byte-code-cli/agents/<profile>/credential    { name, agent_id, api_key }   0600
~/.cicbyte/apps/byte-code-cli/sessions/<profile>/<pid>.json  { session_id }                0600
<repo>/.bc/project                                           { project_id, project_name }  可进 git，无身份
```

| 环境变量 | 说明 |
|---|---|
| `BC_HOME` | 覆盖数据根目录（测试/CI 隔离用），如 `BC_HOME=./tmp-bc bcode whoami` |
| `BC_AGENT` | 等效 `--profile` 旗标（优先级：旗标 > 环境变量 > 配置文件） |

## 🤖 Agent 集成

- **`--json`**：全部输出为单行 compact JSON（stdout 恰好一行；流式命令每事件一行）；错误走 stderr
- **退出码**：`0` 成功 · `2` 用法/环境错 · `3` 认证失效 · `4` 权限拒 · `5` 网络错 · `6` 业务拒
- **租约契约**：认领后 2 小时无平台侧动作自动释放（回 open + lease_expired 留痕）——长任务周期性 `bcode log` 保活
- **约束**：`complete` artifacts 上限 1 MiB；`log --status` 仅接受 `success`/`failed`（平台 schema 约束）
- 配套 skill：本仓库 `.zcode/skills/bcode/`（事实来源在 `skills/bcode/`）已沉淀完整工作流与踩坑知识，agent 会话可直接加载

## 终端兼容

输出为 UTF-8。Windows Terminal / PowerShell / Git Bash / Linux 终端开箱即用；
传统 `cmd.exe` 若中文乱码，先执行 `chcp 65001` 或改用上述终端。

## 发版

提交遵循 conventional commits（feat/fix/chore/…），发版只需在 GitHub Actions
跑 **Tag Release** 工作流：版本号留空由 git-cliff 按提交语义自动推导
（有 feat → minor，仅 fix/chore → patch），也可填 patch/minor/major 或完整版本号；
bot 自动打 tag 并触发 Release 工作流（三平台单二进制 + sha256 + 中文分类
changelog 挂 Release）。二进制版本由构建期注入（tag 优先），与发布版本一致。

## 🤝 参与贡献

提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/)（changelog 按此生成）。
`cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --all` 是合并门槛（CI 三平台矩阵 + cargo-audit 同验）。

## 📄 开源许可证

[MIT](LICENSE) © 2026 cicbyte
