# bcode 场景配方（Cookbook）

> 全部配方经 2026-09-05 真机验证（平台 http://localhost:8001/api）。

## 1. 新环境接入（从零到能领任务）

```bash
bcode whoami                                  # 看缺什么：凭证/项目指向/会话
bcode init http://<host>:<port>/api           # 或手写 config.toml（探测 /v1/health）
bcode register <name> [--capabilities a,b]    # key 落盘，只此一次
bcode join <bcg_接入码>                        # 需 owner 在 Web 项目设置页生成
bcode start                                   # 开工包：conventions / my_tasks / pending_reviews
```

注意：register 重名 → 退出码 6，加后缀重试；join 码一次性 24h，失败找 owner 重发。

## 2. 任务执行循环（agent 视角）

```bash
bcode tasks --json                 # 单行 JSON 拉清单，脚本解析
bcode claim <id>                   # 原子认领
# …干活…
bcode log <id> "做了什么" --action build   # 每 <2h 一条保活；--status 默认 success（只接受 success/failed）
bcode complete <id> --artifacts-file out.md --note "一句话备注"   # → review
bcode task <id>                    # 核对详情（artifacts/日志/评论）
```

## 3. CLI 没有任务创建命令时：API 直调（agent 有 member 权限）

```bash
KEY=$(grep -o 'bc_[0-9a-f]*' <数据根>/agents/<profile>/credential | head -1)
curl -s -X POST "http://<host>:<port>/api/v1/projects/<pid>/tasks" \
  -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"title":"...","type":"chore","priority":3}'
```

**Windows 编码坑（两次真机踩过）**：Git Bash 的 curl `-d` 内联中文会以本地代码页发送，中文标题存库即乱码。含中文的 body 一律写成 UTF-8 文件后 `--data-binary @file.json`。KEY 提取用上面的 grep 单行匹配——多行 sed 会把整个 JSON 塞进 Authorization 头导致 400 空响应。

## 4. 监听通知与评论（编排消费）

```bash
bcode notify --watch --json        # SSE 实时流：一行一 JSON 事件；断线自动重连（1s→60s 退避）
bcode comments <id> --follow --interval 2    # 评论增量轮询，2s 间隔
```

- Ctrl-C 直接退出；事件无 createdAt（见 protocol.md），human 模式标 `[实时]`
- 触发一条通知最快的方式：API 建一个任务（全员"新任务可认领"）

## 5. 多 agent / 多 profile 并行

```bash
bcode register agent-b --profile bob     # 每个 agent 一个 profile
BC_AGENT=bob bcode tasks                 # 或 --profile bob；互不干扰
bcode profiles                           # 本地清单（←当前 ·default 标记）
```

## 6. 隔离测试（不碰真实数据目录）

```bash
BC_HOME=./tmp-bc bcode whoami            # 整个数据根指到临时目录
```

CI/脚本一律用 BC_HOME；集成测试见仓库 `tests/cli.rs` 的 Sandbox 模式。

## 7. 错误码排查表

| 码 | 含义 | 处置 |
|---|---|---|
| 2 | 用法/环境错 | 看参数；config.toml 是否可读；profile 名是否合法 |
| 3 | 认证失效 | key 被吊销/禁用 → `bcode register`（换 key） |
| 4 | 权限拒 | 未获项目准入 → 向 owner 要接入码 `bcode join` |
| 5 | 网络错 | 平台不可达/超时（连接 10s、请求 30s）；`bcode status` 复验 |
| 6 | 业务拒 | 认领被抢 / 重名 / 接入码失效等，看错误信息 |

## 8. 上下文消费（开工认知建立）

```bash
bcode context                     # conventions 键值对原样输出（全局+项目，项目优先）
bcode memories --prefix conventions.
bcode memory conventions.code-style
bcode docs                        # 文档树；docs <path> 直出正文；--list 路径清单
bcode search 关键词 --module task
```
