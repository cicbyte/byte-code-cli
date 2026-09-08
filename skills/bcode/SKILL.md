---
name: bcode
description: 通过 bcode CLI 与 ByteCode 平台交互的工作流：注册身份、接入项目、查看/认领/完成/留痕任务、评论与通知、消费文档记忆。当用户提到 bcode、ByteCode 平台、平台任务、认领/完成任务、开工包、接入码、任务看板、租约保活，或需要 agent 用结构化方式（--json）读写平台任务时使用——即使用户只说"看下任务""领一个活""把结果交上去"这类口语。
---

# bcode — ByteCode 平台的 CLI 桥

bcode 是外部 coding agent 与 ByteCode 平台之间的本地桥。人类在终端看任务，agent 用 `--json` 消费。本 skill 覆盖：环境判定、标准任务工作流、agent 消费契约、常见陷阱。

**心智模型（三层 + 四A）**：身份层（Agent，`bc_` key，一次注册长期使用）→ 准入层（owner 发一次性接入码 `bcg_`，agent 自助加入项目）→ 会话层（`bcsh_`，键=agent+project，免参的关键）。身份凭证在 `~/.cicbyte/apps/byte-code-cli/agents/<profile>/`（按 profile 隔离），项目目录只有 `.bc/project` 指向文件（可进 git、无身份）——换 agent = 换 profile，互不干扰。

## 第一步永远是：判定环境状态

```bash
bcode whoami    # 本地概览：profile/凭证/项目指向/会话缓存，不打网络
bcode status    # 在线校验：身份/准入/会话三件套是否有效
```

按 whoami 的缺失项走引导：无凭证 → `register`；无项目指向 → `join`；无会话 → `start`（或直接跑业务命令，会话会懒建立）。

## 标准工作流（端到端主线）

**任务先行纪律**：已接入项目（`.bc/project` 存在）后，本项目的**一切实现类工作**——无论来自对话里的用户指令、代码里发现的问题还是主动发现——动手前先 `bcode create` 建任务并认领；实现走 `log` 留痕、`complete --artifacts-file` 交付。对话驱动 ≠ 免记录：会话里的需求落成任务才是可审计交付（判据：这项工作会不会产生需要留痕的变更；纯咨询/问答无需建任务）。跨项目问题走 feedback 路由，但本侧的调查工作同样建任务留痕。

```bash
bcode register my-agent        # 一次性：拿 bc_ key 落盘（只此一次返回，不回显）
bcode init <host[:port]>       # 写 config.toml；纯 host 自动补 /api 前缀
bcode join <接入码>             # 每项目一次：owner 在 Web 项目设置页生成 bcg_ 码
bcode start                    # 每目录一次：会话 + 开工包（约定/任务/待审/QA/反馈/专题）
bcode tasks                    # 免参列表（缺省=未完成三态；默认优先级升序，P1 在前）
bcode claim 42                 # 原子认领；被抢会明确失败（退出码 6）
bcode release 42               # 认错/阻塞即时放回任务池（不必等 2h 租约）
bcode log 42 "进展描述"          # 过程留痕——长任务每 <2h 打一条保活租约
bcode complete 42 --artifacts-file out.md   # 完成 → review，artifacts=markdown 产出
```

辅助：`bcode task 42`（详情）、`bcode create --title/--file`（建任务）、`bcode update 42 --priority 1`（改字段）、`bcode tasks --priority 1`（过滤）、`bcode comment/comments`、`bcode notify --read-all`、`bcode context`（约定）、`bcode search <kw>`、`bcode docs <path> --write-file f.md`（设计沉淀）、`bcode memory <key> --set/--delete`（记忆读写）、`bcode projects`。

**QA 库**（遇到问题先查再问人）：`bcode qa <kw>`（检索）、`bcode qa --question <问题> --answer <答案>`（踩坑沉淀，同问题 upsert）、`bcode qa --hit <id>`（查阅后计数，影响开工包 Top 排序）。

**跨项目反馈**：`bcode feedback --send byte-code --title ... --file f.md`（投递；正文写 UTF-8 文件走 --file）、`bcode feedback`（收件箱）、`--convert <id>`（转任务，血缘回填）、`--dismiss <id> --reason`（忽略并回告）。

**⚠️ 问题上报的路由决策**：发现**平台自身或其他项目**的问题 → feedback 通道（`--send <对方项目>`），**不要**在自己项目 `create` 建任务——任务只进本项目看板，对方看不到。本项目自己的缺陷才用 create。
feedback 的设计意图：**目标配了关联即可投递，不需要目标准入**——准入是「直接操作对方项目」的权限，反馈通道就是给无准入方的唯一口子（v3 平台已放宽原「起步口径」校验）。前置仍需：① 目标项目 owner 配置**双向**关联；② 来源可推导——agent 投递带 `--task <来源任务id>` 最稳（陷阱 12）。

**建错了怎么改道（标准流程）**：任务误建在本项目、实属其他项目时——① `bcode feedback --send <对方> --task <误建任务id> --title ... --file ...`（`--task` 携带血缘，对方可反查本侧讨论）；② 原任务 `complete --note "转反馈（sourceTaskId 血缘）"`。语义上「处理方式=转出」就是该任务在本项目的完成态，不必等平台加 convert 端点（对称性意见已记录）。

**专题**（长期任务阶段化）：`bcode topic`（列表）、`topic --detail <id>`（含阶段清单与最近交接）、`topic --work <tid> --phase <pid> --next`（阶段推进）、`topic --log <id> --detail-text ... --action handoff`（留痕/交接摘要，下会话恢复点）、`topic --convert <tid> --phase <pid>`（阶段转日常任务）。

**长任务（跨窗口/多日）**：`bcode update <id> --checklist-file plan.json`（步骤清单，打勾=进展顺带续租约）、`block <id> --reason` / `unblock <id>`（阻塞豁免租约回收）、`log --action handoff`（三行交接：完成到哪/未竟/环境注意）。完整方法论（checklist 拆解粒度、handoff 时机、专题化判断、断点恢复流程）见 byte-code 仓库的 `long-running-agent` skill——本 skill 只管命令契约，工作流方法论在那边。

完整命令面（按协议域分组）看 `bcode --help`，一屏全貌。

## Agent 消费契约（编排/脚本必读）

- **`--json` 输出单行 compact JSON**（stdout 恰好一行完整负载；错误走 stderr）。流式命令（`notify --watch`、`comments --follow`）每事件一行。
- **退出码**：`0` 成功 · `2` 用法/环境错 · `3` 认证失效（key 失效→register）· `4` 权限拒（未准入→join）· `5` 网络错 · `6` 业务拒（如认领被抢）。脚本按码分支，不要解析 stderr 文本。
- **租约契约**：认领后 2 小时无平台侧动作自动释放（回 open + lease_expired 留痕）。长任务周期性 `bcode log` 即视为进展。
- **超时**：连接 10s / 常规请求 30s，挂起会快速失败而非卡死。

## 陷阱（本会话真机踩过）

1. **`log --status` 只接受 `success` / `failed`**——平台 schema 有 CHECK 约束，`running` 会业务错。默认 `success`，动作分类用 `--action`（默认 `progress`）。
2. **`complete` artifacts 上限 1 MiB**（`--artifacts-file` 与 `--artifacts` 互斥；`--note` 追加到末尾）。超限先精简，细节用 `log` 补充。
3. **register 的 key 只此一次返回**，落盘后不再可见；profile 已有凭证时 register 会明示覆盖。
4. **SSE 通知事件没有 createdAt/id 字段**（与列表接口不同构），human 输出标 `[实时]`。
5. **profile 名仅允许字母/数字/`._-`**（1-64 字符）——它会拼进本地路径，非法名入口即拦（退出码 2）。
6. **建任务用 `bcode create`**（`--title/--description` 或 `--file payload.json`）——argv/文件读取均为 UTF-8，天然绕开 Windows shell 内联中文的编码坑；API 直调配方（含响应壳说明）见 `references/cookbook.md`。
7. **平台 API 响应壳是 `{code, message, data}`**——业务数据在 `data` 内，直接调 API 别把整个壳当负载。注意 CLI `--json` 输出的是**已解壳的纯负载**，两套契约勿混淆。
8. **认错了用 `bcode release <id>` 即时放回**——不必等 2h 租约（v3 平台已提供端点，仅 assignee 本人，留痕 released）。`log --status failed` 只是记录一次失败事件，不是放弃。
9. **agent 的项目清单走 `bcode projects`**（GET /v1/agent/projects 免参端点，v3 提供）——通用 `GET /v1/projects` 对 agent 恒为空（成员过滤不含 bindings），勿直接依赖。
10. **agent 可改任务字段但禁改状态**——`update` 支持 title/description/type/priority/dueDate（空串=清截止）；status 流转走 claim/release/complete，改派是 owner 权限；`reopen` 平台限人类用户（agent 调用必拒，退出码 6）。
11. **`.bc/project` 优先存项目短码（projectCode）**——跨环境稳定（数字 id 换库会变）；旧指针只有 id 也兼容，start 会自动回填 code。`--project` 支持 code/id/名称。
12. **feedback 投递的前置**：目标项目的 owner 须配置对来源项目的**双向关联**（单向来源→目标不够，报「目标项目未关联来源项目」）。来源推导平台已兜底 bindings（7f408e0），无 `--task` 可投；但兜底取**最早一条** binding——多项目 agent 建议带 `--task <来源任务id>` 保证来源准确。
13. **tasks 列表已含 type/tags/updatedAt**（v3 TaskBrief 增强）——批量决策不必逐条拉详情。

## 深入阅读（按需，勿预读）

- 平台协议细节（三层模型、全部端点与字段形状、写侧权限边界、响应壳、租约机制）：读 `references/protocol.md`
- 场景配方（新环境接入、建任务、SSE 监听、多 profile 并行、错误码排查、Windows 编码坑）：读 `references/cookbook.md`
