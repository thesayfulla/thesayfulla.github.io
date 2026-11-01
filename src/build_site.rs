//! Export the public site as static files for GitHub Pages.
//!
//!     cargo run --release -- build-site          # writes ./docs
//!
//! Every public page is rendered as a logged-out visitor sees it and saved as
//! <path>/index.html. CSS/JS get content-hashed file names so browsers never
//! show a stale stylesheet after an update. Uploaded images and the hero video
//! are copied from media/. The editor, login and settings are not exported.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use md5::{Digest, Md5};
use walkdir::WalkDir;

use crate::config::Config;
use crate::db::Post;
use crate::pages::{Site, Viewer};
use crate::templates;

const MARKER: &str = ".built-by-build_site";

pub fn run(config: &Config, out: &Path) -> Result<()> {
    let out = if out.is_absolute() { out.to_path_buf() } else { config.root.join(out) };
    prepare(&out)?;

    let manifest = collect_static(&config.static_dir, &out.join("static"))?;
    let env = templates::environment(&config.templates_dir, Some(manifest.into_iter().collect()));
    let site = Site::new(config.clone(), env)?;
    let pages = export_pages(&site, &out)?;
    std::fs::write(out.join("404.html"), site.not_found(&Viewer::anonymous())?)?;

    if config.media_root.exists() {
        copy_tree(&config.media_root, &out.join("media"))?;
    }
    std::fs::write(out.join(".nojekyll"), "")?; // serve files as-is, no Jekyll processing
    println!("Built {pages} pages into {}", out.display());
    Ok(())
}

/// Start from an empty folder, but only ever delete a folder this command made.
fn prepare(out: &Path) -> Result<()> {
    if out.exists() {
        let has_files = std::fs::read_dir(out)?.next().is_some();
        if !out.join(MARKER).exists() && has_files {
            bail!("{} has files this command didn't create; not touching it.", out.display());
        }
        std::fs::remove_dir_all(out)?;
    }
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join(MARKER), "")?;
    Ok(())
}

fn md5_12(bytes: &[u8]) -> String {
    let digest = Md5::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect::<String>()[..12].to_string()
}

/// Copy static/ into `dest`, each file under its own name and a content-hashed one
/// ("css/site.css" -> "css/site.b3954303e4cb.css"), plus staticfiles.json listing them.
/// Returns the original -> hashed name map.
fn collect_static(src: &Path, dest: &Path) -> Result<BTreeMap<String, String>> {
    let mut manifest = BTreeMap::new();
    let entries = WalkDir::new(src).sort_by_file_name().into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        e.depth() == 0 || !(name.starts_with('.') || name.ends_with('~'))
    });
    for entry in entries {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(src)?.to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(entry.path())?;
        let (stem, ext) = match relative.rfind('.').filter(|&i| i > relative.rfind('/').map_or(0, |s| s + 1)) {
            Some(i) => relative.split_at(i),
            None => (relative.as_str(), ""),
        };
        let hashed = format!("{stem}.{}{ext}", md5_12(&bytes));
        for name in [&relative, &hashed] {
            let target = dest.join(name);
            std::fs::create_dir_all(target.parent().unwrap())?;
            std::fs::write(&target, &bytes)?;
        }
        manifest.insert(relative, hashed);
    }

    // Same layout as Django's staticfiles.json.
    let pairs: Vec<[&String; 2]> = manifest.iter().map(|(k, v)| [k, v]).collect();
    let hash = md5_12(python_json(&serde_json::to_value(&pairs)?).as_bytes());
    let paths = python_json(&serde_json::to_value(&manifest)?);
    let json = format!(r#"{{"paths": {paths}, "version": "1.1", "hash": "{hash}"}}"#);
    std::fs::write(dest.join("staticfiles.json"), json)?;
    Ok(manifest)
}

/// JSON the way Python's json.dumps writes it (", " and ": " separators).
fn python_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => format!("[{}]", items.iter().map(python_json).collect::<Vec<_>>().join(", ")),
        serde_json::Value::Object(map) => format!(
            "{{{}}}",
            map.iter().map(|(k, v)| format!("{}: {}", serde_json::Value::from(k.as_str()), python_json(v))).collect::<Vec<_>>().join(", ")
        ),
        other => other.to_string(),
    }
}

fn urls(site: &Site) -> Result<Vec<String>> {
    let mut urls = vec!["/".to_string(), "/cv/".to_string(), "/blog/".to_string()];
    for n in 2..=site.page_count()? {
        urls.push(format!("/blog/page/{n}/"));
    }
    urls.extend(Post::all(&mut site.db.conn())?.iter().map(Post::url));
    Ok(urls)
}

fn export_pages(site: &Site, out: &Path) -> Result<usize> {
    let viewer = Viewer::anonymous(); // logged out: no write/settings links
    let urls = urls(site)?;
    for url in &urls {
        let html = match url.as_str() {
            "/" => Some(site.home(&viewer)?),
            "/cv/" => Some(site.cv(&viewer)?),
            "/blog/" => site.post_list(&viewer, 1)?,
            u => match u.strip_prefix("/blog/page/") {
                Some(n) => site.post_list(&viewer, n.trim_end_matches('/').parse()?)?,
                None => site.post_detail(&viewer, u.trim_start_matches("/blog/").trim_end_matches('/'))?,
            },
        };
        let html = html.with_context(|| format!("{url} returned 404"))?;
        let target: PathBuf = out.join(url.trim_matches('/')).join("index.html");
        std::fs::create_dir_all(target.parent().unwrap())?;
        std::fs::write(&target, html)?;
        println!("  {url}");
    }
    Ok(urls.len())
}

fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let target = dest.join(entry.path().strip_prefix(src)?);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
