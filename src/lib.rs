//! bcode 库层：cli（clap 定义）/ client（HTTP）/ model（线缆类型）/
//! config·cred（本地布局）/ error（退出码体系）/ output（双模式输出）/ cmd（命令实现）。
//! bin 只做解析分发；路线图的 MCP server 将直接复用本 crate。

pub mod cli;
pub mod client;
pub mod cmd;
pub mod config;
pub mod cred;
pub mod error;
pub mod model;
pub mod output;
