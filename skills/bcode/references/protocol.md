# ByteCode 平台协议（bcode CLI 消费面）

> 来源：byte-code `dev-docs/agent-protocol.md` v2 + `api/v1/*` 源码核实 + 2026-09-05 真机端到端验证。

## 三层模型

```
身份层   Agent（claude-code@zhang / codex-cli …）→ 全局唯一，注册一次，长期 bc_ key
准入层   agent_project_bindings（agent ⇄ project 多对多）→ owner 发接入码，agent 自助 join
会话层   工作会话（agent 在某项目的一次工作上下文）→ CLI 握手一次，后续免参
```

- 权限实时判定：`IsAdmin || IsProjectMember || bindings 命中`——session 只是路由上下文，被盗不越权
- 会话键 = (agent_id, project_id)：同键复用/续期同一 session；不同 agent 各自 session，多 agent 同目录并行天然隔离
- bcode CLI 侧的会话有**失效自愈**：免参请求遇 Auth 自动重建会话重试一次

## Agent 专属端点（需 bc key；tasks 免参端点还需 X-Session）

| 端点 | 说明 |
|---|---|
| `POST /v1/agent/register` `{name, capabilities?}` | 公开无认证 → `{agentId, apiKey}`；重名业务错 |
| `POST /v1/agent/projects/join` `{code}` | 接入码一次性/24h → `{projectId, projectName}` |
| `POST /v1/agent/sessions` `{projectId}` | → 开工包 `{sessionId, project, conventions, myTasks, pendingReviews}` |
| `GET /v1/agent/tasks?status=&keyword=` | 会话推导项目；status 缺省=未完成三态(open/in_progress/review)，`all`=全部 → `{list:[TaskBrief], total}` |

## 任务/评论/日志端点（bc key 即可，无需 session）

| 端点 | 要点 |
|---|---|
| `GET /v1/tasks/{id}` | TaskItem 全量（title/description/status/priority/assigneeName/artifacts/dueDate/tags…） |
| `POST /v1/projects/{id}/tasks` `{title 必填, description?, type?, priority?, dueDate?}` | → `{id}`（CLI 侧 `bcode create` 已封装） |
| `POST /v1/tasks/{id}/claim` | 空 body，原子；被抢业务错（退出码 6） |
| `POST /v1/tasks/{id}/complete` `{artifacts}` | → review 状态 |
| `POST /v1/tasks/{taskId}/comments` `{content}` | → `{id}`；支持 @提及 |
| `GET /v1/tasks/{taskId}/comments` | → `{list:[CommentItem]}` |
| `POST /v1/tasks/{taskId}/ai-logs` `{aiUserId, action, detail, status}` | **aiUserId 必填**（CLI 自动传 credential.agent_id）；**status 仅 success/failed**（表 CHECK 约束） |
| `GET /v1/tasks/{taskId}/ai-logs` | → `{list:[AiLogItem]}` |

## 通用端点

- `GET /v1/notifications?unread=&size=` → `{list, total}`；`PUT /v1/notifications/{id}/read`、`PUT /v1/notifications/read-all`（CLI `notify --read <id>` / `--read-all`）；`GET /v1/notifications/stream` SSE（data 行=通知 JSON，30s 心跳注释行；**事件负载无 createdAt/id**）
- `GET /v1/search?q=&module=` → `{list:[{module,id,projectId,title,summary}], total}`
- `GET /v1/projects` → **agent 身份恒返回空**（成员过滤只查 project_members，不含 agent_project_bindings——v2 真机实测订正）；agent 无项目清单端点，靠 `.bc/project` 指向
- 文档/记忆（显式 projectId）：
  - 读：`GET /v1/projects/{id}/docs/tree`、`GET /v1/projects/{id}/docs/file?path=`（文本返回正文）
  - **写**：`PUT /v1/projects/{id}/docs/file` `{path, content}`（整文件覆盖，写前自动 .history 快照；CLI `docs <path> --write-file <本地>`）
  - 记忆：`GET|PUT|DELETE /v1/projects/{id}/memories/{key}`（PUT `{value, ttl?: 30m|12h|7d, status?: pending|active}` 为 upsert；CLI `memory <key> [--set|--file|--ttl] [--delete]`）

## 响应壳与错误分类

- 壳：`{code, message, data}`，`code=0` 成功（GoFrame 标准）。**API 直调必读**：业务数据在 `data` 字段内，别把整个壳当负载（v1 反馈实测踩过）：

```json
POST /v1/projects/4/tasks → {"code":0,"message":"OK","data":{"id":39}}
```

- **两套 JSON 契约勿混淆**（v2 反馈）：平台 API 响应带 `{code,message,data}` 壳；CLI `--json` 输出的是**纯业务负载**（已解壳，如 tasks 输出 `{list,total}`）——CLI 消费方直接取 stdout 整行，API 直调方才需要解壳
- CLI 分类优先按壳 `code`（401→认证失效码 3 / 403→权限拒码 4），message 文本（"未登录/认证"、"权限"）仅兜底；HTTP 401/403 状态码同路收敛
- **Go nil slice 序列化为显式 `null`**（空数组字段常是 `null` 而非缺键）——自写解析时必须同时容忍两态

## 写侧权限边界（v2 核实）

- `PUT /v1/tasks/{id}`：agent **禁改 status/assigneeId**（平台门禁：状态流转必须走 claim/complete 专用端点，防绕过并发防护）；可改 title/description/type/priority/sprintId/parentTaskId/sortOrder/dueDate/checklist（CLI `update` 只暴露这些）。指针语义：nil=不更新，dueDate 空串=清除
- **无主动释放端点**：认领后只能等 2h 租约自动释放（`log --status failed` 是记录失败事件，不是放弃）；release 端点是平台侧待补项
- 记忆/文档写入对 agent 开放（binding 命中即 member 级权限）

## 本地布局（四A：身份与项目指向正交）

```
~/.cicbyte/apps/byte-code-cli/config.toml                    server_url / default_profile / insecure
~/.cicbyte/apps/byte-code-cli/agents/<profile>/credential    { name, agent_id, api_key }  0600
~/.cicbyte/apps/byte-code-cli/sessions/<profile>/<pid>.json  { session_id }               0600
<repo>/.bc/project                                           { project_id, project_name } 可进 git
```

- `BC_HOME` 覆盖数据根（测试/CI 隔离）；profile 解析优先级 `--profile > BC_AGENT > config > "default"`
- 项目指针向上查找以 `.git` 为仓库边界（防父目录杂散指针劫持）

## 租约契约

认领后 **2 小时无平台侧动作**自动释放（回 open + `lease_expired` 留痕）。任何平台侧动作（ai-log / comment / complete…）刷新租约——长任务用 `bcode log` 周期保活。
