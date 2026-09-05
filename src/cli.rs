//! clap 定义：全局参数（--profile / --json）与子命令枚举。
//! 与 main 解耦——参数结构即命令面的单一事实来源，M1 在此扩充子命令。

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

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 本地概览：当前 profile / 凭证 / 项目指向 / 会话缓存（不打网络）
    Whoami,
    /// 在线校验：身份/准入/会话有效性 + 可见任务采样
    Status,
    /// 本地 profile 清单
    Profiles,
}
