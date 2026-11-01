//! Personal site + blog.
//!
//!     cargo run -- serve                 # http://localhost:8000
//!     cargo run -- build-site            # static export into ./docs
//!     cargo run -- backup                # db + media -> ~/Documents/Portfolio Backups
//!     cargo run -- create-user sayfulla
//!     cargo run -- change-password sayfulla

mod auth;
mod backup;
mod build_site;
mod config;
mod cv;
mod db;
mod highlight;
mod pages;
#[cfg(test)]
mod parity_tests;
mod rendering;
mod templates;
mod text;
mod web;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use config::Config;

#[derive(Parser)]
#[command(version, about = "Personal site + blog.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the site (with the editor, login and settings).
    Serve {
        #[arg(long, default_value = "127.0.0.1", env = "HOST")]
        host: String,
        #[arg(long, default_value_t = 8000, env = "PORT")]
        port: u16,
    },
    /// Export the public site as static files for GitHub Pages.
    BuildSite {
        #[arg(long, default_value = "docs")]
        out: PathBuf,
    },
    /// Zip the database and media/ into a dated backup.
    Backup {
        /// Defaults to $PORTFOLIO_BACKUP_DIR, else ~/Documents/Portfolio Backups.
        #[arg(long)]
        dest: Option<PathBuf>,
    },
    /// Create a staff user who can log in, write and change settings.
    /// Asks for anything not given on the command line.
    CreateUser {
        username: Option<String>,
        #[arg(long)]
        email: Option<String>,
    },
    /// Set a new password for a user.
    ChangePassword { username: String },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run(Cli::parse()).await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = Config::from_env()?;
    match cli.command {
        Command::Serve { host, port } => web::serve(config, &host, port).await,
        Command::BuildSite { out } => build_site::run(&config, &out),
        Command::Backup { dest } => backup::run(&config, dest),
        Command::CreateUser { username, email } => auth::create_user_cli(&config, username, email),
        Command::ChangePassword { username } => auth::change_password_cli(&config, &username),
    }
}
