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

- **免参工作流** — 一次 `start` 握手后，日常命令（任务/评论/通知）不带身份与项目信息，会话承载上下文；开工包携带约定/任务/待审/QA/反馈/专题六段协作上下文
- **双输出模式** — 人类可读表格为默认；`--json` 输出单行 compact JSON，是编排脚本与 AI 消费的稳定契约
- **退出码契约** — `0/2/3/4/5/6` 六类语义出口，脚本按码分支而非解析文本
- **会话自愈** — 缓存会话被平台废弃时自动重建重试；连接 10s / 请求 30s 超时，SSE 断线指数退避重连
- **多 agent 并行** — 身份按 profile 隔离（`--profile` / `BC_AGENT`），项目目录只有可进 git 的指针文件（项目短码跨环境稳定），换 agent 零冲突
- **长任务工作流** — checklist 步骤清单（打勾续租约）、handoff 三行交接摘要（跨会话断点恢复）、block/unblock 阻塞豁免租约回收、watch 任务动态订阅
- **跨项目协作** — 反馈通道（关联即投递、convert 转任务血缘回填）、QA 库沉淀与检索、专题阶段化（PRD 拆解→阶段→转任务池）
- **测试执行上报** — `bcode test --run -- <cmd>` 包裹执行（`--junit` 顺手上报、退出码透传）；`--upload` 离线补传 junit XML / byte-code-pytest dump；`--cases --pull|--push` 平台用例与仓库 YAML 双向同步（AI 读用例写代码）

从身份注册、项目接入、任务闭环，到评论通知、文档/记忆消费与测试上报，命令面完整覆盖日常协作全流程。

## 📦 安装

```bash
# cargo-binstall 直装（推荐；拉取 GitHub Releases 预编译单二进制）
cargo binstall --git https://github.com/cicbyte/byte-code-cli bcode

# 从源码
cargo install --path .

# 或直接下载：GitHub Releases 三平台单二进制 + sha256（Linux x64 / Windows x64 / macOS 双架构）
```

环境要求：Rust 1.85+（2024 edition）；运行期零外部依赖（rustls 静态链接）。

## 🚀 快速开始

```bash
$ bcode init http://127.0.0.1:8000         # 写 config.toml（纯 host 自动补 /api）
$ bcode register my-agent                  # 一次性拿 bc_ key（落盘，不再显示）
$ bcode join <接入码>                        # owner 在 Web 项目设置页生成；写 .bc/project
$ bcode start                              # 建立会话 + 开工包
```

`start` 的输出（开工包，含平台侧协作上下文）：

```
  session         bcsh_dfb29c17ec4a02c5b433871c05809a2d
  project         byte-code-cli (id=4, code=d453e8e5)

── 约定（1 条）──
  [global] conventions.release-discipline = 严禁自动发版……

── 我的任务（0）──
  （无）

── 待审（0）──
  （无）

── 高频 QA（0）── 遇到问题先查 QA 库再问人
── 待处理反馈（0）── 阅读后 convert 建任务或 dismiss
── 进行中专题（0）── bcode topic work 推进
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
| 身份 | `register` / `whoami` / `status` / `profiles` / `projects` | 注册拿 key（只此一次落盘）/ 本地概览 / 在线校验 / profile 清单 / 已加入项目（含能力集） |
| 项目 | `join <code>` / `start` / `context` | 接入码换准入（写 `.bc/project`，存项目短码跨环境稳定）/ 会话+开工包 / 约定原样输出 |
| 任务 | `tasks` / `task` / `create` / `update` / `claim` / `release` / `watch`·`unwatch` / `block`·`unblock` / `complete` / `log` / `reopen` | 列表（`--mine` 跨项目）/ 详情（子任务·关注者）/ 建（`--type` 必带）/ 改字段 / 原子认领 / 放回任务池 / 订阅动态 / 阻塞豁免租约 / 完成+artifacts / 留痕（`--action handoff` 交接）/ 重开（人） |
| 通信 | `comment` / `comments [--follow]` / `notify` | 评论 / 评论流增量轮询 / 通知（`--unread`·`--read`·`--watch` SSE 实时流） |
| 上下文 | `docs [path]` / `memory <key>` / `memories` / `search` | 文档树/正文/`--search`/`--write-file` 写入 / 项目记忆（读写）与全局记忆（读）/ 记忆列表 / 全局搜索 |
| QA 与反馈 | `qa [kw]` / `feedback` | 问答检索·沉淀·命中计数 / 跨项目反馈（投递·收件箱·convert 转任务·dismiss 回告） |
| 专题 | `topic --create` / `--phases` / `--detail` / `work` / `log` / `convert` | 长期任务阶段化全生命周期（PRD 拆解导入、阶段推进、handoff 交接、阶段转任务） |
| 测试 | `test --run` / `--upload` / `--cases` | 包裹执行+junit 上报 / 离线补传（自动嗅探格式）/ 用例 ↔ 仓库 YAML 双向同步 |
| 便利 | `init [url]` / `open <task\|board>` / `completion <shell>` / `man` | 引导写配置（纯 host 自动补 `/api`）/ Web 深链 / shell 补全 / 手册 |

监听实时通知（编排消费，一行一事件）：

```bash
$ bcode notify --watch --json
{"content":"项目新增未指派任务「...」","sourceId":33,"sourceType":"task","title":"新任务可认领","type":"info"}
```

跨项目反馈（发现问题投给对应项目的 owner，双向关联即可投递）：

```bash
$ bcode feedback --send byte-code --title "..." --file report.md
  已投递            反馈 #6 → 关联项目 id=1
```

长任务工作流（跨会话进度持久化——checklist 打勾续租约、handoff 三行交接、阻塞豁免）：

```bash
bcode claim 42 && bcode update 42 --checklist-file plan.json   # 拆步骤
bcode block 42 --reason "等待 CI"                               # 预期长时间无动作
bcode log 42 "1) 完成到哪… 2) 未竟… 3) 环境注意…" --action handoff  # 会话收尾交接
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
<repo>/.bc/project                                           { project_id, project_code, project_name }  可进 git，无身份
```

| 环境变量 | 说明 |
|---|---|
| `BC_HOME` | 覆盖数据根目录（测试/CI 隔离用），如 `BC_HOME=./tmp-bc bcode whoami` |
| `BC_AGENT` | 等效 `--profile` 旗标（优先级：旗标 > 环境变量 > 配置文件） |

## 🤖 Agent 集成

- **`--json`**：全部输出为单行 compact JSON（stdout 恰好一行；流式命令每事件一行）；错误走 stderr
- **退出码**：`0` 成功 · `2` 用法/环境错 · `3` 认证失效 · `4` 权限拒 · `5` 网络错 · `6` 业务拒
- **租约契约**：认领后 2 小时无平台侧动作自动释放（回 open + lease_expired 留痕）——checklist 打勾、`log` 留痕、改描述均算动作；预期长时间等待（等 CI/等人）先 `block`（豁免回收）
- **会话项目约束**：任务域操作的目标项目 ≠ 当前会话项目会被平台拒绝——跨项目操作请到对应项目目录执行
- **约束**：`complete` artifacts 上限 1 MiB；`log --status` 仅接受 `success`/`failed`（平台 schema 约束）；`create` 务必带 `--type`（缺省 feature 会使统计失真）
- 配套 skill：本仓库 `skills/bcode/`（ZCode 项目内 `.zcode/skills/bcode` 为发现入口）沉淀了完整工作流、协议细节与踩坑知识，agent 会话可直接加载；长任务方法论另见平台仓库的 `long-running-agent` skill

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
