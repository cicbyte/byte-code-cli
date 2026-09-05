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

> M0（当前）：whoami / status / profiles + 凭证布局 + 错误码体系。
> 任务工作流（claim/complete/log）与通信（comment/notify）见 M1/M2。

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

## 退出码

`0` 成功 · `2` 用法错 · `3` 认证失效 · `4` 权限拒 · `5` 网络错 · `6` 业务拒

## 租约契约

认领后 2 小时无平台侧动作会被自动释放（回 open + lease_expired 留痕）——
长任务请周期性 `bcode log <id> <msg>` 保持心跳。
