//! Back up everything that isn't in git: the database (your posts, bio, settings)
//! and media/ (uploaded images, hero video).
//!
//!     cargo run --release -- backup
//!
//! Writes portfolio-YYYYMMDD-HHMMSS.zip to ~/Documents/Portfolio Backups
//! (or $PORTFOLIO_BACKUP_DIR) and keeps the newest 30. publish.sh runs this
//! before every publish.
//!
//! To restore: stop the server, then unzip a backup into the project folder
//! (it contains db.sqlite3 and media/).

use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::config::Config;

const KEEP: usize = 30;

fn default_dest() -> PathBuf {
    if let Some(dir) = std::env::var_os("PORTFOLIO_BACKUP_DIR") {
        return PathBuf::from(dir);
    }
    home_dir().join("Documents").join("Portfolio Backups")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn expand_user(path: PathBuf) -> PathBuf {
    match path.strip_prefix("~") {
        Ok(rest) => home_dir().join(rest),
        Err(_) => path,
    }
}

pub fn run(config: &Config, dest: Option<PathBuf>) -> Result<()> {
    let dest = expand_user(dest.unwrap_or_else(default_dest));
    std::fs::create_dir_all(&dest).with_context(|| format!("can't create {}", dest.display()))?;
    if !config.db_path.exists() {
        bail!("No database at {}", config.db_path.display());
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let target = dest.join(format!("portfolio-{stamp}.zip"));

    // SQLite's backup API gives a consistent copy even while the server is running.
    let snapshot = std::env::temp_dir().join(format!("portfolio-backup-{}.sqlite3", std::process::id()));
    Connection::open(&config.db_path)?.backup(rusqlite::MAIN_DB, &snapshot, None)?;
    let written = write_zip(&target, &snapshot, &config.media_root);
    let _ = std::fs::remove_file(&snapshot);
    let files = written?;

    prune(&dest)?;

    let size = std::fs::metadata(&target)?.len() as f64 / 1024.0 / 1024.0;
    let message = format!("Backed up {files} files ({size:.1} MB) to {}", target.display());
    if std::io::stdout().is_terminal() {
        println!("\x1b[32;1m{message}\x1b[0m");
    } else {
        println!("{message}");
    }
    Ok(())
}

fn write_zip(target: &Path, snapshot: &Path, media: &Path) -> Result<usize> {
    let mut zip = ZipWriter::new(File::create(target).with_context(|| format!("can't write {}", target.display()))?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("db.sqlite3", options)?;
    zip.write_all(&std::fs::read(snapshot)?)?;
    let mut files = 1;

    if media.exists() {
        for entry in WalkDir::new(media).sort_by_file_name() {
            let entry = entry?;
            if !entry.file_type().is_file() || entry.file_name() == ".DS_Store" {
                continue;
            }
            let relative = entry.path().strip_prefix(media)?;
            let name = Path::new("media").join(relative);
            zip.start_file(name.to_string_lossy().replace('\\', "/"), options)?;
            std::io::copy(&mut File::open(entry.path())?, &mut zip)?;
            files += 1;
        }
    }
    zip.finish()?;
    Ok(files)
}

/// Keep only the newest backups.
fn prune(dest: &Path) -> Result<()> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dest)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("portfolio-") && n.ends_with(".zip"))
        })
        .collect();
    backups.sort();
    let excess = backups.len().saturating_sub(KEEP);
    for old in &backups[..excess] {
        std::fs::remove_file(old)?;
    }
    Ok(())
}
