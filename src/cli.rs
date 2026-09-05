//! clap 定义：全局参数（--profile / --json）与子命令枚举。
//! 与 main 解耦——参数结构即命令面的单一事实来源。

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "bcode",
    version,
    about = "ByteCode CLI — agent 与平台之间的本地桥"
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
    Register {
        /// 全局唯一名称，如 codex-cli；重名被拒时可加后缀重试
        name: String,
        /// 逗号分隔能力声明（可选）
        #[arg(long)]
        capabilities: Option<String>,
    },
    /// 本地概览：当前 profile / 凭证 / 项目指向 / 会话缓存（不打网络）
    Whoami,
    /// 在线校验：身份/准入/会话有效性 + 可见任务采样
    Status,
    /// 本地 profile 清单
    Profiles,
    /// 凭接入码加入项目，成功写 <repo>/.bc/project
    Join {
        /// owner 在 Web 项目设置页生成的一次性接入码（bcg_ 前缀，展示即焚）
        code: String,
    },
    /// 建立工作会话并展示开工包（约定 / 我的任务 / 待审）
    Start {
        /// 项目 id 或名称；缺省取 .bc/project 指向
        #[arg(long)]
        project: Option<String>,
    },
    /// 输出开工包约定（conventions，AI 建立项目认知的入口）
    Context,
    /// 任务列表（会话免参；缺省=未完成三态）
    Tasks {
        /// open / in_progress / review / all
        #[arg(long)]
        status: Option<String>,
        /// 标题关键词过滤
        #[arg(long)]
        keyword: Option<String>,
    },
    /// 单任务详情：描述 / artifacts / 执行日志 / 评论
    Task { id: i64 },
    /// 认领任务（原子；被抢则失败）。租约 2h，周期 log 保活
    Claim { id: i64 },
    /// 完成任务进 review（artifacts 为 markdown 产出）
    Complete {
        id: i64,
        /// artifacts 文本
        #[arg(long, conflicts_with = "artifacts_file")]
        artifacts: Option<String>,
        /// artifacts 文件路径（读取文件内容作为产出；上限 1 MiB）
        #[arg(long)]
        artifacts_file: Option<String>,
        /// 备注（追加到 artifacts 末尾）
        #[arg(long)]
        note: Option<String>,
    },
    /// 过程留痕（执行日志；同时刷新认领租约）
    Log {
        id: i64,
        message: String,
        /// success / failed（平台 schema 约束）
        #[arg(long, default_value = "success")]
        status: String,
        /// 动作分类（如 build / test / progress）
        #[arg(long, default_value = "progress")]
        action: String,
    },
    /// 发表任务评论（支持 @提及）
    Comment { task_id: i64, text: String },
    /// 查看任务评论流；--follow 增量轮询（一行一事件）
    Comments {
        task_id: i64,
        #[arg(long)]
        follow: bool,
        /// 轮询间隔（秒）
        #[arg(long, default_value_t = 3)]
        interval: u64,
    },
    /// 通知列表；--watch SSE 实时流（一行一事件）
    Notify {
        /// 只看未读
        #[arg(long)]
        unread: bool,
        /// SSE 实时流（断线指数退避重连，Ctrl-C 退出）
        #[arg(long)]
        watch: bool,
    },
    /// 文档中枢：缺省目录树；给 path 输出文件正文
    Docs {
        /// 文件路径（vault 内相对路径）
        path: Option<String>,
        /// 只列文件路径清单
        #[arg(long)]
        list: bool,
    },
    /// 读取单条项目记忆（human 模式直接输出 value）
    Memory { key: String },
    /// 项目记忆列表
    Memories {
        /// key 前缀过滤（如 conventions.）
        #[arg(long)]
        prefix: Option<String>,
    },
    /// 全局搜索（已获读权的项目范围）
    Search {
        query: String,
        /// 按模块过滤（如 task / doc）
        #[arg(long)]
        module: Option<String>,
    },
    /// 引导式写 config.toml（server_url；缺省交互询问）
    Init {
        /// 平台地址（含 /api 前缀）；非交互环境必须提供
        url: Option<String>,
    },
    /// 生成 Web 深链并尝试打开浏览器
    Open {
        target: OpenTarget,
        /// 任务 id（target=task 时必填）
        id: Option<i64>,
    },
    /// 生成 shell 补全脚本到 stdout（source 或装入补全目录）
    Completion { shell: clap_complete::Shell },
    /// 生成 man 手册（roff）到 stdout
    Man,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OpenTarget {
    /// 任务（前端暂无直达路由，落到项目看板）
    Task,
    /// 项目看板
    Board,
}
