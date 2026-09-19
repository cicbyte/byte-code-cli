//! `bcode skill`：内嵌 bcode skill 的安装/升级——skill 与二进制同版本锁死，
//! 升级 CLI 后一条命令刷新本地发现目录（此前靠手动 cp，版本漂移是常态）。
//! 文件以 include_str! 构建期嵌入（事实来源仍是仓库 skills/bcode/）。
//!
//! 单一副本模型（用户设计）：正本唯一落 CLI 数据根 `<BC_HOME>/skill/`；
//! 各 agent 宿主的发现目录（--path 指定）通过**目录链接**指向正本——
//! 系统只保留一份实体文件，升级 = 刷新正本，所有链接即时生效。

use anyhow::{Context, Result, anyhow};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::output::Out;

/// 嵌入的 skill 文件清单（目标相对路径 → 内容）。
/// 事实来源：仓库 skills/bcode/（改动后随下次构建/发版生效）
const EMBEDDED: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../../skills/bcode/SKILL.md")),
    (
        "references/protocol.md",
        include_str!("../../skills/bcode/references/protocol.md"),
    ),
    (
        "references/cookbook.md",
        include_str!("../../skills/bcode/references/cookbook.md"),
    ),
];

/// 嵌入 skill 的版本标记（写在安装清单里，用于过期检测）。
/// 与二进制同源（BCODE_BUILD_VERSION 含 git describe，dev 构建也区分）。
const SKILL_VERSION: &str = env!("BCODE_BUILD_VERSION");

/// `bcode skill [--path <dir>] [--copy] [--dry-run] [--list]`
/// - 缺省：写正本到 CLI 数据根 `<BC_HOME>/skill/`（单一实体副本）
/// - --path：宿主发现目录——默认建**目录链接**指向正本（系统只留一份）；
///   链接失败（跨盘/权限/FAT 等）自动回退拷贝并提示，`--copy` 强制拷贝
/// - --dry-run：列出将执行的写入/链接动作，不落盘
/// - --list：嵌入/正本/各链接的版本总览
///
/// 路径支持绝对 / 相对（cwd）/ `~` 展开
pub fn skill(a: &crate::cli::SkillArgs, out: &Out) -> Result<()> {
    let target = resolve_target_dir(a.path.as_deref())?;

    if a.list {
        return list(&target, out);
    }

    // --path：链接模式（正本 + 宿主目录链接）
    if a.path.is_some() {
        return link_mode(&target, a, out);
    }

    // 缺省：正本安装到数据根
    install_files(&default_dir()?, a.dry_run, out, "正本")
}

/// 缺省安装：写正本（幂等覆盖 + 版本标记）
fn install_files(dir: &Path, dry_run: bool, out: &Out, label: &str) -> Result<()> {
    let mut plan = vec![];
    for (rel, content) in EMBEDDED {
        let dest = dir.join(rel);
        let action = if dest.exists() { "覆盖" } else { "创建" };
        plan.push((dest, action, content.len()));
    }
    out.kv("目标", &format!("{}（{label}）", dir.display()));
    out.kv(
        "文件",
        &format!("{} 个（SKILL.md + references/）", plan.len()),
    );
    for (dest, action, bytes) in &plan {
        out.line(&format!("  {action} {}（{bytes} 字节）", dest.display()));
    }
    if dry_run {
        out.line("（dry-run：未写入任何文件）");
        out.emit_value(&json!({
            "dry_run": true, "mode": label,
            "path": dir.display().to_string(),
            "files": plan.iter().map(|(d, a, _)| json!({"file": d.display().to_string(), "action": a})).collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    for (dest, _, _) in &plan {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建 {} 失败", parent.display()))?;
        }
        let (_, content) = EMBEDDED
            .iter()
            .find(|(rel, _)| dest.ends_with(rel))
            .expect("plan 源自 EMBEDDED");
        std::fs::write(dest, content).with_context(|| format!("写入 {} 失败", dest.display()))?;
    }
    std::fs::write(dir.join(".installed-version"), SKILL_VERSION)
        .with_context(|| format!("写版本标记失败（{}）", dir.display()))?;
    out.kv("完成", &format!("{label}已更新（版本 {SKILL_VERSION}）"));
    out.emit_value(&json!({
        "installed": true, "mode": label,
        "path": dir.display().to_string(), "version": SKILL_VERSION, "files": plan.len(),
    }));
    Ok(())
}

/// 链接模式：确保正本存在 → 宿主目录链到正本（失败回退拷贝）
fn link_mode(host_path: &Path, a: &crate::cli::SkillArgs, out: &Out) -> Result<()> {
    let canonical = default_dir()?;

    // 正本不存在或缺版本标记：先装正本（dry-run 下只报告）
    let need_master = installed_version(&canonical) != Some(SKILL_VERSION.to_string());
    if need_master {
        out.line("── 正本（数据根）──");
        install_files(&canonical, a.dry_run, out, "正本")?;
    }

    out.line("── 宿主目录 ──");
    out.kv("目标", &host_path.display().to_string());

    if a.dry_run {
        let mode = if a.copy { "拷贝" } else { "链接" };
        out.line(&format!(
            "  {mode} {} → {}（正本）",
            host_path.display(),
            canonical.display()
        ));
        if !a.copy && host_path.exists() {
            out.line("  （目标已存在：将先移除旧目录再建链接）");
        }
        out.line("（dry-run：未写入任何文件）");
        out.emit_value(&json!({
            "dry_run": true, "mode": mode, "host": host_path.display().to_string(),
            "master": canonical.display().to_string(),
        }));
        return Ok(());
    }

    if a.copy {
        return copy_dir(&canonical, host_path, out);
    }

    match link_dir(&canonical, host_path) {
        Ok(()) => {
            out.kv(
                "完成",
                &format!(
                    "已链接 {} → {}（升级正本即全链接生效）",
                    host_path.display(),
                    canonical.display()
                ),
            );
            out.emit_value(&json!({
                "linked": true, "host": host_path.display().to_string(),
                "master": canonical.display().to_string(), "version": SKILL_VERSION,
            }));
            Ok(())
        }
        Err(e) => {
            out.line(&format!(
                "（链接失败：{e}——回退拷贝模式；可用 --copy 直达）"
            ));
            copy_dir(&canonical, host_path, out)
        }
    }
}

/// 目录级链接：目标已存在则先移除（旧拷贝/旧链接），然后建链接。
/// Windows 用 junction（无需管理员）；Unix 用 symlink。
fn link_dir(src: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建 {} 失败", parent.display()))?;
    }
    if dest.exists() || dest.symlink_metadata().is_ok() {
        if dest.is_dir()
            && !dest
                .symlink_metadata()
                .map(|m| m.is_symlink())
                .unwrap_or(false)
        {
            std::fs::remove_dir_all(dest).with_context(|| {
                format!("移除旧目录 {} 失败（请手动清理后重试）", dest.display())
            })?;
        } else {
            std::fs::remove_file(dest).context("移除旧链接失败")?;
        }
    }
    #[cfg(windows)]
    {
        // junction：无管理员权限要求，跨会话稳定；mklink 两侧均需绝对路径
        // （相对路径含 ./ 前缀会报「无效参数」——真机踩坑）
        let src_abs = std::fs::canonicalize(src)
            .with_context(|| format!("定位正本失败（{}）", src.display()))?;
        let dest_abs = absolute(dest)?;
        std::process::Command::new("cmd")
            .args([
                "/c",
                "mklink",
                "/J",
                &dest_abs.display().to_string(),
                &src_abs.display().to_string(),
            ])
            .output()
            .map_err(|e| anyhow!("启动 mklink 失败：{e}"))
            .and_then(|o| {
                if o.status.success() {
                    Ok(())
                } else {
                    Err(anyhow!(
                        "mklink 退出 {}：{}",
                        o.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&o.stderr).trim()
                    ))
                }
            })
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dest)
            .with_context(|| format!("symlink {} → {} 失败", dest.display(), src.display()))
    }
}

/// 相对路径补绝对（目标尚不存在，不能 canonicalize——父目录 canonicalize 后拼回）
#[cfg(windows)]
fn absolute(p: &Path) -> Result<PathBuf> {
    // mklink 对路径中的 `./` 段敏感（cwd.join("./x") 会原样保留）——
    // 统一用 components 重建：剥掉 CurDir 段、根前缀保留
    let cleaned: PathBuf = p
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect();
    if cleaned.is_absolute() {
        return Ok(cleaned);
    }
    let cwd = std::env::current_dir()?;
    let joined = cwd.join(&cleaned);
    // 父目录存在则 canonicalize（消 .. 与盘符大小写），否则原样
    if let Some(parent) = joined.parent()
        && let Ok(canon) = std::fs::canonicalize(parent)
        && let Some(name) = joined.file_name()
    {
        return Ok(canon.join(name));
    }
    Ok(joined)
}

/// 拷贝回退：正本目录整体复制到宿主目录（独立副本，升级需重新执行）
fn copy_dir(src: &Path, dest: &Path, out: &Out) -> Result<()> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)
            .with_context(|| format!("移除旧目录 {} 失败", dest.display()))?;
    }
    for (rel, content) in EMBEDDED {
        let d = dest.join(rel);
        if let Some(parent) = d.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&d, content)?;
        out.line(&format!("  拷贝 {}（{} 字节）", d.display(), content.len()));
    }
    out.kv(
        "完成",
        &format!(
            "已拷贝到 {}（独立副本，升级需重跑或改用链接）",
            dest.display()
        ),
    );
    out.emit_value(&json!({
        "copied": true, "host": dest.display().to_string(),
        "master": src.display().to_string(), "version": SKILL_VERSION,
    }));
    Ok(())
}

/// 版本总览：嵌入 vs 正本；--path 时再看该目录（链接解析到正本自然一致）
fn list(target: &Path, out: &Out) -> Result<()> {
    out.kv("嵌入版本", SKILL_VERSION);
    let master = default_dir()?;
    match installed_version(&master) {
        Some(v) => {
            out.kv("正本", &format!("{}（{v}）", master.display()));
            if v != SKILL_VERSION {
                out.line("（正本较旧：bcode skill 刷新）");
            }
        }
        None => out.kv("正本", "（未安装）"),
    }
    if let Some(t) = target.to_str() {
        let canon = master.display().to_string();
        if t != canon && !t.is_empty() {
            match installed_version(target) {
                Some(v) => out.kv("目标目录", &format!("{t}（{v}）")),
                None => out.kv("目标目录", &format!("{t}（未安装）")),
            }
        }
    }
    out.emit_value(&json!({
        "embedded": SKILL_VERSION,
        "master": master.display().to_string(),
        "master_installed": installed_version(&master),
    }));
    Ok(())
}

/// 目标目录解析：绝对直用；`~`/`~/...` 展开主目录；其余相对 cwd
fn resolve_target_dir(path: Option<&str>) -> Result<PathBuf> {
    let raw = match path {
        Some(p) => p.to_string(),
        None => return default_dir(),
    };
    let expanded = if raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("无法定位主目录展开 ~"))?;
        home.join(raw.trim_start_matches("~/").trim_start_matches('~'))
            .display()
            .to_string()
    } else {
        raw
    };
    let p = Path::new(&expanded);
    Ok(if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()?.join(p)
    })
}

/// 默认装 CLI 自己的数据根（中立：不绑定任何 agent 宿主；BC_HOME 覆盖同生效）
fn default_dir() -> Result<PathBuf> {
    Ok(crate::config::bc_root()?.join("skill"))
}

fn installed_version(target: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(target.join(".installed-version")).ok()?;
    let v = raw.trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_dir_absolute_relative_and_tilde() {
        let cwd = std::env::current_dir().unwrap();
        // Unix 绝对路径在 Windows 上不 is_absolute（会被当相对 join cwd）——按平台断言
        if cfg!(unix) {
            assert_eq!(
                resolve_target_dir(Some("/abs/skill-dir")).unwrap(),
                PathBuf::from("/abs/skill-dir")
            );
        }
        assert_eq!(
            resolve_target_dir(Some("rel/skill")).unwrap(),
            cwd.join("rel/skill")
        );
        assert_eq!(resolve_target_dir(Some(".")).unwrap(), cwd);
        let home = dirs::home_dir().unwrap();
        assert_eq!(resolve_target_dir(Some("~")).unwrap(), home);
        assert_eq!(
            resolve_target_dir(Some("~/x/y")).unwrap(),
            home.join("x").join("y")
        );
    }

    #[test]
    fn default_is_cli_data_root_not_host_bound() {
        let d = default_dir()
            .unwrap()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        assert!(d.ends_with("skill"), "默认应落 CLI 数据根（中立）：{d}");
        assert!(
            !d.contains(".zcode") && !d.contains(".claude"),
            "不得绑定任何宿主目录：{d}"
        );
    }

    #[test]
    fn embedded_files_nonempty() {
        assert!(!EMBEDDED.is_empty());
        for (rel, content) in EMBEDDED {
            assert!(!content.is_empty(), "{rel} 嵌入为空");
            assert!(content.contains("bcode"), "{rel} 内容异常");
        }
    }
}
