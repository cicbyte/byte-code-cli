//! 命令实现，按协议三层分组（agent-protocol.md v2）：
//! identity=注册层，project=准入与会话层；
//! M1 的任务工作流落 tasks.rs、M2 通信落 comms.rs。

pub mod identity;
pub mod project;
