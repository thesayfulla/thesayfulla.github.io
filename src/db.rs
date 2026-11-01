//! SQLite storage. The tables are the ones the Django version created, so an
//! existing db.sqlite3 keeps working as-is (posts, profile, users).

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use diesel::connection::SimpleConnection;
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::dsl::exists;
use diesel::expression::AsExpression;
use diesel::prelude::*;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};
use serde::Serialize;

use crate::config::MEDIA_URL;
use crate::rendering;

pub type Conn = SqliteConnection;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS "auth_user" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "password" varchar(128) NOT NULL, "last_login" datetime NULL, "is_superuser" bool NOT NULL, "username" varchar(150) NOT NULL UNIQUE, "last_name" varchar(150) NOT NULL, "email" varchar(254) NOT NULL, "is_staff" bool NOT NULL, "is_active" bool NOT NULL, "date_joined" datetime NOT NULL, "first_name" varchar(150) NOT NULL);
CREATE TABLE IF NOT EXISTS "blog_post" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "title" varchar(255) NOT NULL, "slug" varchar(80) NOT NULL UNIQUE, "body_delta" text NOT NULL CHECK ((JSON_VALID("body_delta") OR "body_delta" IS NULL)), "body_html" text NOT NULL, "created_at" datetime NOT NULL, "updated_at" datetime NOT NULL);
CREATE TABLE IF NOT EXISTS "blog_postimage" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "file" varchar(100) NOT NULL, "created_at" datetime NOT NULL);
CREATE TABLE IF NOT EXISTS "home_profile" ("id" integer NOT NULL PRIMARY KEY AUTOINCREMENT, "handle" varchar(60) NOT NULL, "name" varchar(100) NOT NULL, "intro" text NOT NULL, "hero_video" varchar(100) NOT NULL, "x_url" varchar(200) NOT NULL, "linkedin_url" varchar(200) NOT NULL, "github_url" varchar(200) NOT NULL, "email" varchar(254) NOT NULL, "latest_posts" smallint unsigned NOT NULL CHECK ("latest_posts" >= 0));
CREATE TABLE IF NOT EXISTS "web_session" ("key" text NOT NULL PRIMARY KEY, "user_id" integer NOT NULL REFERENCES "auth_user" ("id") ON DELETE CASCADE, "flash" text NULL, "expires_at" datetime NOT NULL);
CREATE INDEX IF NOT EXISTS "blog_post_created_at" ON "blog_post" ("created_at");
"#;

// Datetimes are Text: they hold Django's timestamp strings (see Ts below).
diesel::table! {
    auth_user (id) {
        id -> BigInt,
        password -> Text,
        last_login -> Nullable<Text>,
        is_superuser -> Bool,
        username -> Text,
        last_name -> Text,
        email -> Text,
        is_staff -> Bool,
        is_active -> Bool,
        date_joined -> Text,
        first_name -> Text,
    }
}

diesel::table! {
    blog_post (id) {
        id -> BigInt,
        title -> Text,
        slug -> Text,
        body_delta -> Text,
        body_html -> Text,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    blog_postimage (id) {
        id -> BigInt,
        file -> Text,
        created_at -> Text,
    }
}

diesel::table! {
    home_profile (id) {
        id -> BigInt,
        handle -> Text,
        name -> Text,
        intro -> Text,
        hero_video -> Text,
        x_url -> Text,
        linkedin_url -> Text,
        github_url -> Text,
        email -> Text,
        latest_posts -> BigInt,
    }
}

diesel::table! {
    web_session (key) {
        key -> Text,
        user_id -> BigInt,
        flash -> Nullable<Text>,
        expires_at -> Text,
    }
}

diesel::joinable!(web_session -> auth_user (user_id));
diesel::allow_tables_to_appear_in_same_query!(auth_user, web_session);

/// One connection behind a lock. Queries are tiny, so this is plenty for a personal site.
/// Never hold the guard across an `.await` (the compiler enforces this in handlers).
pub struct Db(Mutex<Conn>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Conn::establish(&path.to_string_lossy())
            .with_context(|| format!("can't open the database at {}", path.display()))?;
        conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        conn.batch_execute(SCHEMA).context("can't create the database tables")?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn conn(&self) -> MutexGuard<'_, Conn> {
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

#[derive(Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct Ts(NaiveDateTime);

impl FromSql<Text, Sqlite> for Ts {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        let s: String = FromSql::<Text, Sqlite>::from_sql(value)?;
        Ok(Ts(parse_ts(&s)))
    }
}

impl ToSql<Text, Sqlite> for Ts {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(format_ts(&self.0));
        Ok(IsNull::No)
    }
}

impl From<Ts> for NaiveDateTime {
    fn from(ts: Ts) -> Self {
        ts.0
    }
}

// ---------- Profile ----------

pub const DEFAULT_INTRO: &str = "<p>I’m a software engineer currently building <a href=\"#\">Your Project</a>. \
Previously, I studied computer science at <a href=\"#\">Your University</a> and worked as an \
engineer at <a href=\"#\">Company One</a> and <a href=\"#\">Company Two</a>.</p>\
<p>My first taste of software success was when I built <a href=\"#\">something people loved</a> \
— and I’ve been shipping things ever since.</p>";

/// Everything about you shown on the site. There is only ever one row; edit it at /settings/.
#[derive(Clone, Debug, Serialize, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = home_profile)]
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
    pub fn load(conn: &mut Conn) -> Result<Self> {
        let found = home_profile::table.find(1).select(Self::as_select()).first(conn).optional()?;
        match found {
            Some(profile) => Ok(profile),
            None => {
                let profile = Self::default();
                profile.save(conn)?;
                Ok(profile)
            }
        }
    }

    pub fn save(&self, conn: &mut Conn) -> Result<()> {
        diesel::insert_into(home_profile::table)
            .values((home_profile::id.eq(1), self))
            .on_conflict(home_profile::id)
            .do_update()
            .set(self)
            .execute(conn)?;
        Ok(())
    }
}

// ---------- Posts ----------

#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = blog_post)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub slug: String,
    /// The editor's own document (used to re-open the post for editing), as JSON.
    pub body_delta: String,
    /// The rendered article HTML (what visitors see).
    pub body_html: String,
    #[diesel(deserialize_as = Ts)]
    pub created_at: NaiveDateTime,
    #[diesel(deserialize_as = Ts)]
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

impl Post {
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

    /// Newest first. A limit of -1 means all of them.
    pub fn list(conn: &mut Conn, limit: i64, offset: i64) -> Result<Vec<Self>> {
        use self::blog_post::dsl::*;
        Ok(blog_post
            .order((created_at.desc(), id.desc()))
            .limit(limit)
            .offset(offset)
            .select(Self::as_select())
            .load(conn)?)
    }

    pub fn all(conn: &mut Conn) -> Result<Vec<Self>> {
        Self::list(conn, -1, 0)
    }

    pub fn count(conn: &mut Conn) -> Result<i64> {
        Ok(blog_post::table.count().get_result(conn)?)
    }

    pub fn by_id(conn: &mut Conn, id: i64) -> Result<Option<Self>> {
        Ok(blog_post::table.find(id).select(Self::as_select()).first(conn).optional()?)
    }

    pub fn by_slug(conn: &mut Conn, slug: &str) -> Result<Option<Self>> {
        Ok(blog_post::table
            .filter(blog_post::slug.eq(slug))
            .select(Self::as_select())
            .first(conn)
            .optional()?)
    }

    pub fn slug_taken(conn: &mut Conn, slug: &str, exclude_id: Option<i64>) -> Result<bool> {
        // ids start at 1, so 0 excludes nothing.
        let others = blog_post::table
            .filter(blog_post::slug.eq(slug))
            .filter(blog_post::id.ne(exclude_id.unwrap_or(0)));
        Ok(diesel::select(exists(others)).get_result(conn)?)
    }

    /// Insert (id 0) or update. Sets the timestamps.
    pub fn save(&mut self, conn: &mut Conn) -> Result<()> {
        use self::blog_post::dsl::*;
        self.updated_at = now();
        let fields = (
            title.eq(&self.title),
            slug.eq(&self.slug),
            body_delta.eq(&self.body_delta),
            body_html.eq(&self.body_html),
            updated_at.eq(Ts(self.updated_at)),
        );
        if self.id == 0 {
            self.created_at = self.updated_at;
            self.id = diesel::insert_into(blog_post)
                .values((fields, created_at.eq(Ts(self.created_at))))
                .returning(id)
                .get_result(conn)?;
        } else {
            diesel::update(blog_post.find(self.id)).set(fields).execute(conn)?;
        }
        Ok(())
    }

    pub fn delete(conn: &mut Conn, id: i64) -> Result<bool> {
        Ok(diesel::delete(blog_post::table.find(id)).execute(conn)? > 0)
    }
}

/// An image uploaded from the editor. Kept even if removed from a post, so old links don't break.
/// Returns the new row's id.
pub fn insert_post_image(conn: &mut Conn, file: &str) -> Result<i64> {
    use blog_postimage::dsl;
    Ok(diesel::insert_into(dsl::blog_postimage)
        .values((dsl::file.eq(file), dsl::created_at.eq(Ts(now()))))
        .returning(dsl::id)
        .get_result(conn)?)
}

pub fn media_url(name: &str) -> String {
    format!("{MEDIA_URL}{name}")
}

// ---------- Users and sessions ----------

#[derive(Clone, Debug, Queryable, Selectable)]
#[diesel(table_name = auth_user)]
pub struct User {
    pub id: i64,
    pub password: String,
    pub is_staff: bool,
    pub is_active: bool,
}

impl User {
    pub fn by_username(conn: &mut Conn, username: &str) -> Result<Option<Self>> {
        Ok(auth_user::table
            .filter(auth_user::username.eq(username))
            .select(Self::as_select())
            .first(conn)
            .optional()?)
    }

    /// The active user behind a live session, if any.
    pub fn by_session(conn: &mut Conn, key: &str) -> Result<Option<Self>> {
        Ok(web_session::table
            .inner_join(auth_user::table)
            .filter(web_session::key.eq(key))
            .filter(web_session::expires_at.gt(Ts(now())))
            .filter(auth_user::is_active.eq(true))
            .select(Self::as_select())
            .first(conn)
            .optional()?)
    }

    pub fn create(conn: &mut Conn, name: &str, mail: &str, password_hash: &str) -> Result<()> {
        use self::auth_user::dsl::*;
        diesel::insert_into(auth_user)
            .values((
                password.eq(password_hash),
                is_superuser.eq(true),
                username.eq(name),
                first_name.eq(""),
                last_name.eq(""),
                email.eq(mail),
                is_staff.eq(true),
                is_active.eq(true),
                date_joined.eq(Ts(now())),
            ))
            .execute(conn)?;
        Ok(())
    }

    pub fn set_password(conn: &mut Conn, id: i64, password_hash: &str) -> Result<()> {
        diesel::update(auth_user::table.find(id)).set(auth_user::password.eq(password_hash)).execute(conn)?;
        // Log out everywhere.
        diesel::delete(web_session::table.filter(web_session::user_id.eq(id))).execute(conn)?;
        Ok(())
    }

    pub fn touch_last_login(conn: &mut Conn, id: i64) -> Result<()> {
        diesel::update(auth_user::table.find(id)).set(auth_user::last_login.eq(Some(Ts(now())))).execute(conn)?;
        Ok(())
    }
}

pub fn create_session(conn: &mut Conn, key: &str, user_id: i64, expires_at: &NaiveDateTime) -> Result<()> {
    diesel::delete(web_session::table.filter(web_session::expires_at.le(Ts(now())))).execute(conn)?;
    diesel::insert_into(web_session::table)
        .values((
            web_session::key.eq(key),
            web_session::user_id.eq(user_id),
            web_session::expires_at.eq(Ts(*expires_at)),
        ))
        .execute(conn)?;
    Ok(())
}

pub fn delete_session(conn: &mut Conn, key: &str) -> Result<()> {
    diesel::delete(web_session::table.find(key)).execute(conn)?;
    Ok(())
}

/// A one-time message shown on the next page ("Saved.").
pub fn set_flash(conn: &mut Conn, key: &str, message: &str) -> Result<()> {
    diesel::update(web_session::table.find(key)).set(web_session::flash.eq(message)).execute(conn)?;
    Ok(())
}

pub fn take_flash(conn: &mut Conn, key: &str) -> Result<Option<String>> {
    let flash: Option<String> = web_session::table
        .find(key)
        .select(web_session::flash)
        .first(conn)
        .optional()?
        .flatten();
    if flash.is_some() {
        diesel::update(web_session::table.find(key))
            .set(web_session::flash.eq(None::<String>))
            .execute(conn)?;
    }
    Ok(flash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_save_and_load() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        let conn = &mut db.conn();
        let mut post = Post {
            id: 0,
            title: "Hello".into(),
            slug: "hello".into(),
            body_delta: "{}".into(),
            body_html: "<p>hi</p>".into(),
            created_at: now(),
            updated_at: now(),
        };
        post.save(conn).unwrap();
        post.title = "Hello again".into();
        post.save(conn).unwrap();

        let saved = Post::by_slug(conn, "hello").unwrap().unwrap();
        assert_eq!(saved.title, "Hello again");
        assert_eq!(format_ts(&saved.created_at), format_ts(&post.created_at));
        assert!(Post::slug_taken(conn, "hello", None).unwrap());
        assert!(!Post::slug_taken(conn, "hello", Some(post.id)).unwrap());
    }

    #[test]
    fn sessions() {
        let db = Db::open(Path::new(":memory:")).unwrap();
        let conn = &mut db.conn();
        User::create(conn, "me", "me@example.com", "hash").unwrap();
        let me = User::by_username(conn, "me").unwrap().unwrap();
        create_session(conn, "abc", me.id, &(now() + chrono::Duration::hours(1))).unwrap();
        assert!(User::by_session(conn, "abc").unwrap().is_some());

        set_flash(conn, "abc", "Saved.").unwrap();
        assert_eq!(take_flash(conn, "abc").unwrap().as_deref(), Some("Saved."));
        assert_eq!(take_flash(conn, "abc").unwrap(), None);

        User::set_password(conn, me.id, "new").unwrap();
        assert!(User::by_session(conn, "abc").unwrap().is_none());
    }
}
