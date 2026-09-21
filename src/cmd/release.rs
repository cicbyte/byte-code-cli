//! 项目发布域命令：本地打包 → 一条命令推平台（团队内分发安装包）。
//! 建删收 maintainer 档；读全成员+绑定 agent；文件走平台独立发布文件通道。

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;

use super::project_ctx;
use crate::cli::ReleaseArgs;
use crate::config::Config;
use crate::model::discuss::{ReleaseFileList, ReleaseItem, ReleaseList};
use crate::output::{Out, pad_display};

/// `bcode release`：缺省列表（--channel 过滤）。
/// `--create <ver> [--title] [--notes-file] [--channel]` 创建；
/// `--upload <ver> --file <p>...` 上传文件（可多个）；
/// `--download <ver> [--file <名>] [-o <目录>]` 下载；
/// `--delete <ver>` 删除（级联清文件）。
pub async fn release(cfg: &Config, profile: &str, a: &ReleaseArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    if let Some(ver) = &a.create {
        let notes = match a.notes_file.as_deref() {
            Some(f) => std::fs::read_to_string(f).with_context(|| format!("读取 {f} 失败"))?,
            None => String::new(),
        };
        let mut body = json!({ "version": ver, "notes": notes });
        if let Some(t) = a.title.as_deref() {
            body["title"] = json!(t);
        }
        if let Some(c) = a.channel.as_deref() {
            body["channel"] = json!(c);
        }
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(&format!("/v1/projects/{pid}/releases"), body)
            .await?;
        out.kv(
            "已创建",
            &format!("发布 {ver}（#{}, 可 --upload 传文件）", created.id),
        );
        out.emit_value(&json!({ "created": true, "version": ver, "release_id": created.id }));
        return Ok(());
    }

    if let Some(ver) = &a.upload {
        let files = a
            .file
            .as_deref()
            .ok_or_else(|| anyhow!("--upload 需要 --file <路径>（可多次）"))?;
        let rel = find_by_version(&ctx, pid, ver).await?;
        for f in files {
            let name = std::path::Path::new(f)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            ctx.client
                .upload_file(&format!("/v1/releases/{}/files", rel.id), &name, f)
                .await?;
            out.kv("已上传", &format!("{name} → {ver}"));
        }
        out.emit_value(&json!({ "uploaded": files.len(), "version": ver, "release_id": rel.id }));
        return Ok(());
    }

    if let Some(ver) = &a.download {
        let rel = find_by_version(&ctx, pid, ver).await?;
        let files: ReleaseFileList = ctx
            .client
            .get_as(&format!("/v1/releases/{}/files", rel.id))
            .await?;
        if files.list.is_empty() {
            out.line(&format!("（{ver} 无文件）"));
            return Ok(());
        }
        let target = a.output.clone().unwrap_or_else(|| ".".into());
        let picks: Vec<_> = match a.file.as_deref() {
            Some(names) => files
                .list
                .iter()
                .filter(|f| names.contains(&f.file_name))
                .collect(),
            None => files.list.iter().collect(),
        };
        if picks.is_empty() {
            bail!("指定文件名不在发布文件列表中");
        }
        let n_downloaded = picks.len();
        std::fs::create_dir_all(&target).with_context(|| format!("创建 {target} 失败"))?;
        for f in picks {
            let dest = std::path::Path::new(&target).join(&f.file_name);
            // 认证下载两跳：share 端点生成/获取 token → public 直链取文件流
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ShareRes {
                #[serde(default)]
                url: String,
            }
            let share: ShareRes = ctx
                .client
                .post_as(&format!("/v1/release-files/{}/share", f.id), json!({}))
                .await
                .map_err(|e| {
                    // 平台 share 是 maintainer 档；成员 agent 无鉴权下载通道
                    // （文件列表只回 shared 布尔不回 token）——平台侧已知缺口
                    anyhow!("下载失败：{e}（分享端点限 maintainer；成员侧请让 maintainer 执行 bcode release --download，或索取分享直链）")
                })?;
            // share 返回相对路径（/api/v1/release-files/public/<token>），
            // 兼容未来绝对 URL 形态：剥 origin 留 /release-files/ 起始段
            let raw = share.url.trim();
            let api_path = if raw.starts_with('/') {
                raw.to_string()
            } else {
                let stripped = raw
                    .trim_start_matches("http://")
                    .trim_start_matches("https://");
                match stripped.find("/release-files/") {
                    Some(i) => stripped[i..].to_string(),
                    None => bail!("分享直链形态异常：{raw}"),
                }
            };
            ctx.client.download_file_pub(&api_path, &dest).await?;
            out.kv(
                "已下载",
                &format!(
                    "{}（{:.1} MB）",
                    dest.display(),
                    f.file_size as f64 / 1048576.0
                ),
            );
        }
        out.emit_value(&json!({ "downloaded": n_downloaded, "version": ver }));
        return Ok(());
    }

    if let Some(ver) = &a.delete {
        let rel = find_by_version(&ctx, pid, ver).await?;
        ctx.client
            .delete(&format!("/v1/releases/{}", rel.id))
            .await?;
        out.kv("已删除", &format!("发布 {ver}（文件级联清理）"));
        out.emit_value(&json!({ "deleted": true, "version": ver }));
        return Ok(());
    }

    // 缺省：列表
    let mut path = format!("/v1/projects/{pid}/releases");
    if let Some(c) = a.channel.as_deref() {
        path.push_str(&format!("?channel={c}"));
    }
    let list: ReleaseList = ctx.sessioned_get(&path).await?;
    if list.list.is_empty() {
        out.line("（无发布）");
    }
    for r in &list.list {
        out.line(&format!(
            "  {} {} [{}] {}（{} 文件，{:.1} MB，by {}）",
            pad_display(&r.version, 12),
            r.channel,
            r.id,
            r.title,
            r.file_count,
            r.total_size_bytes as f64 / 1048576.0,
            r.created_by_name,
        ));
    }
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}

/// 版本号 → 发布 id/条目（本地唯一性由平台保证；找不到时报可操作错误）
async fn find_by_version(ctx: &super::Ctx, pid: i64, ver: &str) -> Result<ReleaseItem> {
    let list: ReleaseList = ctx
        .client
        .get_as(&format!("/v1/projects/{pid}/releases"))
        .await?;
    list.list
        .into_iter()
        .find(|r| r.version == ver)
        .ok_or_else(|| anyhow!("发布 {ver} 不存在（bcode release 看列表）"))
}
