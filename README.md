# bcode

[ByteCode](../byte-code) 平台的 CLI——外部 coding agent 与平台之间的本地桥。

- 人类在终端直接看任务/认领/回报，不打断心流开 Web
- AI agent 通过 `--json` 结构化输出消费（MCP 化见路线图）

协议依据：`byte-code/dev-docs/agent-protocol.md` v2（三层模型：注册 → 项目准入 → 工作会话）。
需求文档：`byte-code/dev-docs/cli-requirements.md`。

## 快速开始

```bash
cargo install --path .        # 或直接 cargo build --release

bcode register my-agent       # 一次性拿 bc_ key（落数据目录 agents/<profile>/）
bcode join <接入码>            # owner 在 Web 项目设置页生成；写 .bc/project
bcode start                   # 建立会话 + 展示开工包（conventions/my_tasks）
bcode tasks                   # 免参列表（会话承载当前项目）
bcode claim 42 && bcode complete 42 --artifacts-file out.md
```

> 覆盖 M0-M3（F01-F20）：身份/接入/任务闭环 + 通信 + 上下文消费 + 便利命令；
> MCP 化（F21）独立里程碑。

## 命令面

| 域 | 命令 | 说明 |
|---|---|---|
| 身份 | `register` / `whoami` / `status` / `profiles` | 注册拿 key（只此一次落盘）/ 本地概览 / 在线校验 / profile 清单 |
| 项目 | `join <code>` / `start` / `context` | 接入码换准入（写 `.bc/project`）/ 会话+开工包 / 约定原样输出 |
| 任务 | `tasks` / `task <id>` / `claim <id>` / `complete <id>` / `log <id> <msg>` | 免参列表 / 详情 / 原子认领 / 完成+artifacts / 过程留痕（租约心跳） |
| 通信 | `comment` / `comments [--follow]` / `notify [--watch]` | 评论 / 评论流增量轮询 / 通知列表与 SSE 实时流 |
| 上下文 | `docs [path]` / `memory <key>` / `memories` / `search <kw>` | 文档树与正文 / 项目记忆 / 记忆列表 / 全局搜索 |
| 便利 | `init [url]` / `open <task\|board>` / `completion <shell>` / `man` | 引导写配置 / Web 深链 / shell 补全 / 手册 |

## 本地布局（四A 模型：身份与项目指向正交）

```
~/.cicbyte/apps/byte-code-cli/config.toml                    server_url / default_profile
~/.cicbyte/apps/byte-code-cli/agents/<profile>/credential    { name, agent_id, api_key }   0600
~/.cicbyte/apps/byte-code-cli/sessions/<profile>/<pid>.json  { session_id }
<repo>/.bc/project                   { project_id, project_name }  可进 git，无身份
```

换 agent = 换 profile（`--profile` / `BC_AGENT`）；多 agent 同目录并行天然隔离。

> `BC_HOME` 环境变量可覆盖数据根目录 `~/.cicbyte/apps/byte-code-cli`（测试/CI 指向临时目录用），
> 如 `BC_HOME=./tmp-bc bcode whoami`。

## 终端兼容

输出为 UTF-8。Windows Terminal / PowerShell / Git Bash / Linux 终端开箱即用；
传统 `cmd.exe` 若中文乱码，先执行 `chcp 65001` 或改用上述终端。自签 HTTPS 部署用
`--insecure`（会打警告）或 config.toml 的 `insecure = true`。

## 退出码

`0` 成功 · `2` 用法错 · `3` 认证失效 · `4` 权限拒 · `5` 网络错 · `6` 业务拒

## 租约契约

认领后 2 小时无平台侧动作会被自动释放（回 open + lease_expired 留痕）——
长任务请周期性 `bcode log <id> <msg>` 保持心跳。
