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

```bash
bcode register my-agent        # 一次性：拿 bc_ key 落盘（只此一次返回，不回显）
bcode join <接入码>             # 每项目一次：owner 在 Web 项目设置页生成 bcg_ 码
bcode start                    # 每目录一次：会话 + 开工包（约定/我的任务/待审）
bcode tasks                    # 免参列表（缺省=未完成三态）
bcode claim 42                 # 原子认领；被抢会明确失败（退出码 6）
bcode log 42 "进展描述"          # 过程留痕——长任务每 <2h 打一条保活租约
bcode complete 42 --artifacts-file out.md   # 完成 → review，artifacts=markdown 产出
```

辅助：`bcode task 42`（详情：描述/artifacts/日志/评论）、`bcode comment 42 "文本"`、`bcode comments 42`、`bcode notify`、`bcode context`（开工包约定原样输出，建立项目认知）、`bcode search <kw>`、`bcode docs [path]`、`bcode memory <key>`。

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
6. **CLI 没有任务创建命令**（有意设计：建任务是 owner/编排侧职责）。agent 确需建任务时走 API 直调——配方见 `references/cookbook.md`。

## 深入阅读（按需，勿预读）

- 平台协议细节（三层模型、全部端点与字段形状、响应壳、租约机制）：读 `references/protocol.md`
- 场景配方（新环境接入、API 建任务、SSE 监听、多 profile 并行、错误码排查、Windows 编码坑）：读 `references/cookbook.md`
