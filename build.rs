//! 构建期版本注入（借鉴 byte-code 的 ldflags 方案，Rust 等价实现）：
//! 优先级 BCODE_VERSION 环境变量（CI 显式指定）> git describe（tag 距离，
//! 如 0.1.0-3-g1a2b3c）> Cargo.toml 版本（无 git/无 tag 的兜底）。
//! 经 rustc-env 传给 `env!("BCODE_BUILD_VERSION")`，消除「tag 与二进制版本漂移」。

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=BCODE_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let version = std::env::var("BCODE_VERSION")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
        .or_else(git_describe_version)
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=BCODE_BUILD_VERSION={version}");
}

/// `git describe --tags --match v*` → 去掉 v 前缀；无 tag/非 git 目录返回 None
fn git_describe_version() -> Option<String> {
    let out = Command::new("git")
        .args(["describe", "--tags", "--match", "v*"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().strip_prefix('v')?;
    Some(s.to_string())
}
