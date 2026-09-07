//! clap 定义：全局参数（--profile / --json / --insecure）与子命令枚举。
//! 与 main 解耦——参数结构即命令面的单一事实来源。
//! 顶层 help 用自定义模板渲染「按协议域分组」的命令目录（agent 一屏看全貌），
//! GROUPED_CATALOG 与实际子命令由单测强制同步（缺一个即测试失败）。

use clap::{Parser, Subcommand};

/// 顶层帮助的分组命令目录 + 示例/提示（渲染进 --help 的 after-help 段）
const GROUPED_CATALOG: &str = "\
命令总览（按协议域分组；详细参数看 `bcode <命令> --help`）

身份与接入
  register <name>    注册身份（key 只此一次返回并落盘，不回显）
  whoami             本地概览：profile/凭证/项目指向/会话缓存（不打网络）
  status             在线校验：身份/准入/会话有效性 + 可见任务采样
  profiles           本地 profile 清单
  join <code>        凭接入码加入项目（写 .bc/project）
  projects           已加入项目清单（标注当前目录指向）

工作会话
  start [--project]  建立会话 + 展示开工包（约定/我的任务/待审）
  context            开工包约定（conventions）原样输出——AI 建立项目认知的入口

任务工作流
  tasks              免参列表（未完成三态；优先级升序；--status/--keyword/--priority/--sort）
  task <id>          单任务详情：描述/artifacts/执行日志/评论
  create             建任务（--title/--description/--file JSON 绕开编码坑；实验性）
  update <id>        改字段（--title/--type/--priority/--due；状态流转走 claim/complete）
  claim <id>         原子认领（租约 2h 无主动释放，周期 log 保活）
  complete <id>      完成 → review（--artifacts-file/--note；上限 1 MiB）
  log <id> <msg>     过程留痕（执行日志；刷新租约；failed=记录失败，非放弃）

通信
  comment <id> <text>  发表评论（支持 @提及）
  comments <id>        评论流（--follow 增量轮询，一行一事件）
  notify               通知（--unread / --read <id> / --read-all / --watch SSE 实时流）

上下文消费
  docs [path]        文档中枢（读正文 / --list；--write-file 写入，设计沉淀）
  memory <key>       项目记忆（读；--set/--file 写；--delete 删）
  memories           记忆列表（--prefix 前缀过滤）
  search <kw>        全局搜索（已获读权的项目范围）

配置与工具
  init [url]         引导写配置（TTY 交互；纯 host 自动补 /api）
  open <task|board>  生成 Web 深链并尝试打开浏览器
  completion <shell> shell 补全脚本（stdout）
  man                man 手册（roff，stdout）

示例
  bcode register my-agent                          # 一次性拿 bc_ key
  bcode join <接入码>                               # owner 在 Web 项目设置页生成
  bcode start                                      # 会话 + 开工包
  bcode tasks && bcode claim 42 && bcode complete 42 --artifacts-file out.md
  bcode notify --watch                             # SSE 实时通知

提示
  --json               全部输出单行 JSON（agent/脚本消费的稳定契约）
  --profile / BC_AGENT 切换身份（协议 4A：身份与项目指向正交）
  退出码                0 成功 · 2 用法/环境错 · 3 认证失效 · 4 权限拒 · 5 网络错 · 6 业务拒
  BC_HOME              覆盖数据根目录 ~/.cicbyte/apps/byte-code-cli";

#[derive(Parser)]
#[command(
    name = "bcode",
    // 构建期注入（build.rs）：BCODE_VERSION > git describe > Cargo.toml，
    // 保证发布二进制与 tag 版本一致
    version = env!("BCODE_BUILD_VERSION"),
    about = "ByteCode CLI — agent 与平台之间的本地桥",
    help_template = "{about}\n\n{usage-heading} {usage}\n\n{options}{after-help}",
    after_help = GROUPED_CATALOG
)]
pub struct Cli {
    /// 身份 profile（默认取 BC_AGENT 或 default；协议 4A：身份与项目指向正交）
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// 全部输出改为单行 JSON（供 AI/脚本消费的稳定契约）
    #[arg(long, global = true)]
    pub json: bool,

    /// 跳过 TLS 证书校验（自签/调试专用；优先于 config.toml 的 insecure）
    #[arg(long, global = true)]
    pub insecure: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 注册身份（key 只此一次返回并落盘，不回显）
    Register(RegArgs),
    /// 本地概览：当前 profile / 凭证 / 项目指向 / 会话缓存（不打网络）
    Whoami,
    /// 在线校验：身份/准入/会话有效性 + 可见任务采样
    Status,
    /// 本地 profile 清单
    Profiles,
    /// 凭接入码加入项目，成功写 <repo>/.bc/project
    Join(JoinArgs),
    /// 列出已加入的项目（标注当前目录指向）
    Projects,
    /// 建立工作会话并展示开工包（约定 / 我的任务 / 待审）
    Start(StartArgs),
    /// 输出开工包约定（conventions，AI 建立项目认知的入口）
    Context,
    /// 任务列表（会话免参；缺省=未完成三态；默认优先级升序）
    Tasks(TasksArgs),
    /// 单任务详情：描述 / artifacts / 执行日志 / 评论
    Task(TaskIdArg),
    /// 建任务（实验性：owner/编排侧补齐；--file 读 UTF-8 JSON 绕开 shell 编码坑）
    Create(CreateArgs),
    /// 更新任务字段（agent 可改：标题/描述/类型/优先级/截止；状态流转走 claim/complete）
    Update(UpdateArgs),
    /// 认领任务（原子；被抢则失败）。租约 2h，周期 log 保活；无主动释放（平台侧待补）
    Claim(TaskIdArg),
    /// 完成任务进 review（artifacts 为 markdown 产出，上限 1 MiB）
    Complete(CompleteArgs),
    /// 过程留痕（执行日志；同时刷新认领租约）。--status failed=记录一次失败事件，非放弃
    Log(LogArgs),
    /// 发表任务评论（支持 @提及）
    Comment(CommentArgs),
    /// 查看任务评论流；--follow 增量轮询（一行一事件）
    Comments(CommentsArgs),
    /// 通知列表；--read/--read-all 标已读；--watch SSE 实时流（一行一事件）
    Notify(NotifyArgs),
    /// 文档中枢：缺省目录树；给 path 读正文；--write-file 写入（设计沉淀进平台）
    Docs(DocsArgs),
    /// 项目记忆：缺省读取；--set/--file 写入；--delete 删除
    Memory(MemoryArgs),
    /// 项目记忆列表
    Memories(MemoriesArgs),
    /// 全局搜索（已获读权的项目范围）
    Search(SearchArgs),
    /// 引导式写 config.toml（server_url；缺省交互询问）
    Init(InitArgs),
    /// 生成 Web 深链并尝试打开浏览器
    Open(OpenArgs),
    /// 生成 shell 补全脚本到 stdout（source 或装入补全目录）
    Completion(CompletionArgs),
    /// 生成 man 手册（roff）到 stdout
    Man,
}

// 子命令参数体（main 以 Command::X(Args { .. }) 模式匹配）

#[derive(clap::Args)]
pub struct RegArgs {
    /// 全局唯一名称，如 codex-cli；重名被拒时可加后缀重试
    pub name: String,
    /// 逗号分隔能力声明（可选）
    #[arg(long)]
    pub capabilities: Option<String>,
}

#[derive(clap::Args)]
pub struct JoinArgs {
    /// owner 在 Web 项目设置页生成的一次性接入码（bcg_ 前缀，展示即焚）
    pub code: String,
}

#[derive(clap::Args)]
pub struct StartArgs {
    /// 项目 id 或名称；缺省取 .bc/project 指向
    #[arg(long)]
    pub project: Option<String>,
}

#[derive(clap::Args)]
pub struct TasksArgs {
    /// open / in_progress / review / all
    #[arg(long)]
    pub status: Option<String>,
    /// 标题关键词过滤
    #[arg(long)]
    pub keyword: Option<String>,
    /// 按优先级过滤（1-5，1 最高）
    #[arg(long)]
    pub priority: Option<i64>,
    /// 排序：priority（默认，升序 P1 在前）/ id（平台原始顺序）
    #[arg(long, default_value = "priority")]
    pub sort: String,
}

#[derive(clap::Args)]
pub struct CreateArgs {
    /// 任务标题（--file 已提供时可用此项覆盖）
    #[arg(long)]
    pub title: Option<String>,
    /// 任务描述
    #[arg(long)]
    pub description: Option<String>,
    /// 完整请求体 JSON 文件（UTF-8；字段即平台 TaskCreateReq，Windows 下推荐）
    #[arg(long)]
    pub file: Option<String>,
    /// 类型（feature / bug / chore …）
    #[arg(long)]
    pub r#type: Option<String>,
    /// 优先级（1-5，1 最高；缺省 3）
    #[arg(long)]
    pub priority: Option<i64>,
    /// 截止日期（Y-m-d）
    #[arg(long)]
    pub due: Option<String>,
}

#[derive(clap::Args)]
pub struct TaskIdArg {
    pub id: i64,
}

#[derive(clap::Args)]
pub struct UpdateArgs {
    pub id: i64,
    /// 新标题
    #[arg(long)]
    pub title: Option<String>,
    /// 新描述
    #[arg(long)]
    pub description: Option<String>,
    /// 类型（feature / bug / chore …）
    #[arg(long)]
    pub r#type: Option<String>,
    /// 优先级（1-5，1 最高）
    #[arg(long)]
    pub priority: Option<i64>,
    /// 截止日期（Y-m-d）；空串=清除
    #[arg(long)]
    pub due: Option<String>,
}

#[derive(clap::Args)]
pub struct CompleteArgs {
    pub id: i64,
    /// artifacts 文本
    #[arg(long, conflicts_with = "artifacts_file")]
    pub artifacts: Option<String>,
    /// artifacts 文件路径（读取文件内容作为产出；上限 1 MiB）
    #[arg(long)]
    pub artifacts_file: Option<String>,
    /// 备注（追加到 artifacts 末尾）
    #[arg(long)]
    pub note: Option<String>,
}

#[derive(clap::Args)]
pub struct LogArgs {
    pub id: i64,
    pub message: String,
    /// success / failed（平台 schema 约束）
    #[arg(long, default_value = "success")]
    pub status: String,
    /// 动作分类（如 build / test / progress）
    #[arg(long, default_value = "progress")]
    pub action: String,
}

#[derive(clap::Args)]
pub struct CommentArgs {
    pub task_id: i64,
    pub text: String,
}

#[derive(clap::Args)]
pub struct CommentsArgs {
    pub task_id: i64,
    #[arg(long)]
    pub follow: bool,
    /// 轮询间隔（秒）
    #[arg(long, default_value_t = 3)]
    pub interval: u64,
}

#[derive(clap::Args)]
pub struct NotifyArgs {
    /// 只看未读
    #[arg(long)]
    pub unread: bool,
    /// 标记指定通知已读（与 --watch 互斥）
    #[arg(long)]
    pub read: Option<i64>,
    /// 全部标记已读
    #[arg(long)]
    pub read_all: bool,
    /// SSE 实时流（断线指数退避重连，Ctrl-C 退出）
    #[arg(long)]
    pub watch: bool,
}

#[derive(clap::Args)]
pub struct DocsArgs {
    /// 文件路径（vault 内相对路径；--write-file 时为写入目标）
    pub path: Option<String>,
    /// 只列文件路径清单
    #[arg(long)]
    pub list: bool,
    /// 写入：本地文件 → vault 目标路径（整文件覆盖，平台自动 .history 快照）
    #[arg(long)]
    pub write_file: Option<String>,
}

#[derive(clap::Args)]
pub struct MemoryArgs {
    pub key: String,
    /// 写入值（与 --file 互斥；均缺省为读取）
    #[arg(long, conflicts_with = "file")]
    pub set: Option<String>,
    /// 写入值从本地 UTF-8 文件读取（长值/多行推荐）
    #[arg(long)]
    pub file: Option<String>,
    /// 有效期：30m / 12h / 7d（缺省永不过期）
    #[arg(long)]
    pub ttl: Option<String>,
    /// 删除该记忆
    #[arg(long)]
    pub delete: bool,
}

#[derive(clap::Args)]
pub struct MemoriesArgs {
    /// key 前缀过滤（如 conventions.）
    #[arg(long)]
    pub prefix: Option<String>,
}

#[derive(clap::Args)]
pub struct SearchArgs {
    pub query: String,
    /// 按模块过滤（如 task / doc）
    #[arg(long)]
    pub module: Option<String>,
}

#[derive(clap::Args)]
pub struct InitArgs {
    /// 平台地址（含 /api 前缀）；非交互环境必须提供
    pub url: Option<String>,
}

#[derive(clap::Args)]
pub struct OpenArgs {
    pub target: OpenTarget,
    /// 任务 id（target=task 时必填）
    pub id: Option<i64>,
}

#[derive(clap::Args)]
pub struct CompletionArgs {
    pub shell: clap_complete::Shell,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OpenTarget {
    /// 任务（前端暂无直达路由，落到项目看板）
    Task,
    /// 项目看板
    Board,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// 分组目录与实际子命令强制同步：新增命令没进目录，此测试即失败
    #[test]
    fn grouped_catalog_covers_every_subcommand() {
        for sc in Cli::command().get_subcommands() {
            let name = sc.get_name();
            if name == "help" {
                continue;
            }
            assert!(
                GROUPED_CATALOG.contains(name),
                "帮助分组目录缺少子命令「{name}」——请同步 GROUPED_CATALOG"
            );
        }
    }
}
