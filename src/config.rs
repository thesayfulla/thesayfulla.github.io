use std::path::PathBuf;

use anyhow::{Context, Result};

/// Where everything lives. All paths hang off the project folder: the current
/// directory, or $PORTFOLIO_ROOT when set.
#[derive(Clone, Debug)]
pub struct Config {
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub media_root: PathBuf,
    pub static_dir: PathBuf,
    pub templates_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let root = match std::env::var_os("PORTFOLIO_ROOT") {
            Some(dir) => PathBuf::from(dir),
            None => std::env::current_dir().context("can't read the current directory")?,
        };
        Ok(Self {
            db_path: root.join("db.sqlite3"),
            media_root: root.join("media"),
            static_dir: root.join("static"),
            templates_dir: root.join("templates"),
            root,
        })
    }
}

pub const STATIC_URL: &str = "/static/";
pub const MEDIA_URL: &str = "/media/";
pub const LOGIN_URL: &str = "/login/";
pub const LOGIN_REDIRECT_URL: &str = "/write/";
