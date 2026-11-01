//! SQLite storage. The tables are the ones the Django version created, so an
//! existing db.sqlite3 keeps working as-is (posts, profile, users).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;

use crate::config::MEDIA_URL;
use crate::rendering;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS "auth_user" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "password" varchar(128) NOT NULL, "last_login" datetime NULL, "is_superuser" bool NOT NULL, "username" varchar(150) NOT NULL UNIQUE, "last_name" varchar(150) NOT NULL, "email" varchar(254) NOT NULL, "is_staff" bool NOT NULL, "is_active" bool NOT NULL, "date_joined" datetime NOT NULL, "first_name" varchar(150) NOT NULL);
CREATE TABLE IF NOT EXISTS "blog_post" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "title" varchar(255) NOT NULL, "slug" varchar(80) NOT NULL UNIQUE, "body_delta" text NOT NULL CHECK ((JSON_VALID("body_delta") OR "body_delta" IS NULL)), "body_html" text NOT NULL, "created_at" datetime NOT NULL, "updated_at" datetime NOT NULL);
CREATE TABLE IF NOT EXISTS "blog_postimage" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "file" varchar(100) NOT NULL, "created_at" datetime NOT NULL);
CREATE TABLE IF NOT EXISTS "home_profile" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "handle" varchar(60) NOT NULL, "name" varchar(100) NOT NULL, "intro" text NOT NULL, "hero_video" varchar(100) NOT NULL, "x_url" varchar(200) NOT NULL, "linkedin_url" varchar(200) NOT NULL, "github_url" varchar(200) NOT NULL, "email" varchar(254) NOT NULL, "latest_posts" smallint unsigned NOT NULL CHECK ("latest_posts" >= 0));
CREATE TABLE IF NOT EXISTS "web_session" ("key" text NOT NULL PRIMARY KEY, "user_id" integer NOT NULL REFERENCES "auth_user" ("id") ON DELETE CASCADE, "flash" text NULL, "expires_at" datetime NOT NULL);
CREATE INDEX IF NOT EXISTS "blog_post_created_at" ON "blog_post" ("created_at");
"#;

/// One connection behind a lock. Queries are tiny, so this is plenty for a personal site.
/// Never hold the guard across an `.await` (the compiler enforces this in handlers).
pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("can't open the database at {}", path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        conn.execute_batch(SCHEMA).context("can't create the database tables")?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

// ---------- Timestamps ----------
// Stored as UTC "YYYY-MM-DD HH:MM:SS.ffffff", like Django does.

pub fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

pub fn format_ts(dt: &NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

fn parse_ts(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
        .unwrap_or_default()
}

// ---------- Profile ----------

pub const DEFAULT_INTRO: &str = "<p>I’m a software engineer currently building <a href=\"#\">Your Project</a>. \
Previously, I studied computer science at <a href=\"#\">Your University</a> and worked as an \
engineer at <a href=\"#\">Company One</a> and <a href=\"#\">Company Two</a>.</p>\
<p>My first taste of software success was when I built <a href=\"#\">something people loved</a> \
— and I’ve been shipping things ever since.</p>";

/// Everything about you shown on the site. There is only ever one row; edit it at /settings/.
#[derive(Clone, Debug, Serialize)]
pub struct Profile {
    /// Shown at the top of every page.
    pub handle: String,
    /// Used in browser tab titles.
    pub name: String,
    /// Sanitized HTML.
    pub intro: String,
    /// Path under media/, e.g. "hero/jellyfish.mp4". Empty for none.
    pub hero_video: String,
    pub x_url: String,
    pub linkedin_url: String,
    pub github_url: String,
    pub email: String,
    pub latest_posts: i64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            handle: "sayfulla".into(),
            name: "Sayfulla Mirxoliqov".into(),
            intro: DEFAULT_INTRO.into(),
            hero_video: String::new(),
            x_url: String::new(),
            linkedin_url: String::new(),
            github_url: String::new(),
            email: "mirxoliqov.sayfulla@gmail.com".into(),
            latest_posts: 5,
        }
    }
}

impl Profile {
    pub fn hero_video_url(&self) -> Option<String> {
        (!self.hero_video.is_empty()).then(|| media_url(&self.hero_video))
    }

    /// The profile row, created with the defaults the first time.
    pub fn load(conn: &Connection) -> Result<Self> {
        let found = conn
            .query_row(
                "SELECT handle, name, intro, hero_video, x_url, linkedin_url, github_url, email, latest_posts
                 FROM home_profile WHERE id = 1",
                [],
                |r| {
                    Ok(Self {
                        handle: r.get(0)?,
                        name: r.get(1)?,
                        intro: r.get(2)?,
                        hero_video: r.get(3)?,
                        x_url: r.get(4)?,
                        linkedin_url: r.get(5)?,
                        github_url: r.get(6)?,
                        email: r.get(7)?,
                        latest_posts: r.get(8)?,
                    })
                },
            )
            .optional()?;
        match found {
            Some(profile) => Ok(profile),
            None => {
                let profile = Self::default();
                profile.save(conn)?;
                Ok(profile)
            }
        }
    }

    pub fn save(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT INTO home_profile (id, handle, name, intro, hero_video, x_url, linkedin_url, github_url, email, latest_posts)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (id) DO UPDATE SET handle = ?1, name = ?2, intro = ?3, hero_video = ?4, x_url = ?5,
                 linkedin_url = ?6, github_url = ?7, email = ?8, latest_posts = ?9",
            params![
                self.handle, self.name, self.intro, self.hero_video, self.x_url,
                self.linkedin_url, self.github_url, self.email, self.latest_posts
            ],
        )?;
        Ok(())
    }
}

// ---------- Posts ----------

#[derive(Clone, Debug)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    /// The editor's own document (used to re-open the post for editing), as JSON.
    pub body_delta: String,
    /// The rendered article HTML (what visitors see).
    pub body_html: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// What templates see of a post.
#[derive(Serialize)]
pub struct PostView {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub body_html: String,
    pub created_at: String,
    pub excerpt: String,
    pub reading_minutes: u64,
}

const POST_COLUMNS: &str = "id, title, slug, body_delta, body_html, created_at, updated_at";

impl Post {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            title: r.get(1)?,
            slug: r.get(2)?,
            body_delta: r.get(3)?,
            body_html: r.get(4)?,
            created_at: parse_ts(&r.get::<_, String>(5)?),
            updated_at: parse_ts(&r.get::<_, String>(6)?),
        })
    }

    pub fn url(&self) -> String {
        format!("/blog/{}/", self.slug)
    }

    pub fn reading_minutes(&self) -> u64 {
        let (prose, code) = rendering::split_text(&self.body_html);
        // Code is read more slowly than prose, but not word by word.
        let words = prose.split_whitespace().count() as f64 + code.split_whitespace().count() as f64 / 2.0;
        ((words / 220.0).ceil() as u64).max(1)
    }

    pub fn excerpt(&self) -> String {
        let text = rendering::prose_text(&self.body_html);
        if text.chars().count() <= 160 {
            return text;
        }
        let cut = crate::text::truncate_chars(&text, 157);
        let cut = cut.rsplit_once(' ').map_or(cut, |(head, _)| head);
        format!("{cut}…")
    }

    pub fn view(&self) -> PostView {
        PostView {
            id: self.id,
            title: self.title.clone(),
            url: self.url(),
            body_html: self.body_html.clone(),
            created_at: self.created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            excerpt: self.excerpt(),
            reading_minutes: self.reading_minutes(),
        }
    }

    /// Newest first.
    pub fn list(conn: &Connection, limit: i64, offset: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {POST_COLUMNS} FROM blog_post ORDER BY created_at DESC, id DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit, offset], Self::from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn all(conn: &Connection) -> Result<Vec<Self>> {
        Self::list(conn, -1, 0)
    }

    pub fn count(conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COUNT(*) FROM blog_post", [], |r| r.get(0))?)
    }

    pub fn by_id(conn: &Connection, id: i64) -> Result<Option<Self>> {
        let sql = format!("SELECT {POST_COLUMNS} FROM blog_post WHERE id = ?1");
        Ok(conn.query_row(&sql, [id], Self::from_row).optional()?)
    }

    pub fn by_slug(conn: &Connection, slug: &str) -> Result<Option<Self>> {
        let sql = format!("SELECT {POST_COLUMNS} FROM blog_post WHERE slug = ?1");
        Ok(conn.query_row(&sql, [slug], Self::from_row).optional()?)
    }

    pub fn slug_taken(conn: &Connection, slug: &str, exclude_id: Option<i64>) -> Result<bool> {
        Ok(conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM blog_post WHERE slug = ?1 AND id IS NOT ?2)",
            params![slug, exclude_id],
            |r| r.get(0),
        )?)
    }

    /// Insert (id 0) or update. Sets the timestamps.
    pub fn save(&mut self, conn: &Connection) -> Result<()> {
        self.updated_at = now();
        if self.id == 0 {
            self.created_at = self.updated_at;
            conn.execute(
                "INSERT INTO blog_post (title, slug, body_delta, body_html, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    self.title, self.slug, self.body_delta, self.body_html,
                    format_ts(&self.created_at), format_ts(&self.updated_at)
                ],
            )?;
            self.id = conn.last_insert_rowid();
        } else {
            conn.execute(
                "UPDATE blog_post SET title = ?1, slug = ?2, body_delta = ?3, body_html = ?4, updated_at = ?5
                 WHERE id = ?6",
                params![self.title, self.slug, self.body_delta, self.body_html, format_ts(&self.updated_at), self.id],
            )?;
        }
        Ok(())
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<bool> {
        Ok(conn.execute("DELETE FROM blog_post WHERE id = ?1", [id])? > 0)
    }
}

/// An image uploaded from the editor. Kept even if removed from a post, so old links don't break.
/// Returns the new row's id.
pub fn insert_post_image(conn: &Connection, file: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO blog_postimage (file, created_at) VALUES (?1, ?2)",
        params![file, format_ts(&now())],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn media_url(name: &str) -> String {
    format!("{MEDIA_URL}{name}")
}

// ---------- Users and sessions ----------

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub password: String,
    pub is_staff: bool,
    pub is_active: bool,
}

impl User {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            password: r.get(1)?,
            is_staff: r.get(2)?,
            is_active: r.get(3)?,
        })
    }

    pub fn by_username(conn: &Connection, username: &str) -> Result<Option<Self>> {
        Ok(conn
            .query_row(
                "SELECT id, password, is_staff, is_active FROM auth_user WHERE username = ?1",
                [username],
                Self::from_row,
            )
            .optional()?)
    }

    /// The active user behind a live session, if any.
    pub fn by_session(conn: &Connection, key: &str) -> Result<Option<Self>> {
        Ok(conn
            .query_row(
                "SELECT u.id, u.password, u.is_staff, u.is_active
                 FROM web_session s JOIN auth_user u ON u.id = s.user_id
                 WHERE s.key = ?1 AND s.expires_at > ?2 AND u.is_active",
                params![key, format_ts(&now())],
                Self::from_row,
            )
            .optional()?)
    }

    pub fn create(conn: &Connection, username: &str, email: &str, password_hash: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO auth_user (password, is_superuser, username, first_name, last_name, email, is_staff, is_active, date_joined)
             VALUES (?1, 1, ?2, '', '', ?3, 1, 1, ?4)",
            params![password_hash, username, email, format_ts(&now())],
        )?;
        Ok(())
    }

    pub fn set_password(conn: &Connection, id: i64, password_hash: &str) -> Result<()> {
        conn.execute("UPDATE auth_user SET password = ?1 WHERE id = ?2", params![password_hash, id])?;
        // Log out everywhere.
        conn.execute("DELETE FROM web_session WHERE user_id = ?1", [id])?;
        Ok(())
    }

    pub fn touch_last_login(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("UPDATE auth_user SET last_login = ?1 WHERE id = ?2", params![format_ts(&now()), id])?;
        Ok(())
    }
}

pub fn create_session(conn: &Connection, key: &str, user_id: i64, expires_at: &NaiveDateTime) -> Result<()> {
    conn.execute("DELETE FROM web_session WHERE expires_at <= ?1", [format_ts(&now())])?;
    conn.execute(
        "INSERT INTO web_session (key, user_id, expires_at) VALUES (?1, ?2, ?3)",
        params![key, user_id, format_ts(expires_at)],
    )?;
    Ok(())
}

pub fn delete_session(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM web_session WHERE key = ?1", [key])?;
    Ok(())
}

/// A one-time message shown on the next page ("Saved.").
pub fn set_flash(conn: &Connection, key: &str, message: &str) -> Result<()> {
    conn.execute("UPDATE web_session SET flash = ?1 WHERE key = ?2", params![message, key])?;
    Ok(())
}

pub fn take_flash(conn: &Connection, key: &str) -> Result<Option<String>> {
    let flash: Option<Option<String>> = conn
        .query_row("SELECT flash FROM web_session WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    let flash = flash.flatten();
    if flash.is_some() {
        conn.execute("UPDATE web_session SET flash = NULL WHERE key = ?1", [key])?;
    }
    Ok(flash)
}
