//! Every page of the site, rendered to HTML. Shared by the web server and the
//! static export, so both produce the same pages.

use std::collections::BTreeMap;

use anyhow::Result;
use minijinja::value::{Serde, Value};
use minijinja::{Environment, context};
use serde::Serialize;

use crate::config::Config;
use crate::cv::CV;
use crate::db::{Db, Post, Profile, User};
use crate::text::escape;

pub const POSTS_PER_PAGE: i64 = 20;

pub struct Site {
    pub config: Config,
    pub db: Db,
    pub env: Environment<'static>,
}

/// Who is looking at the page.
pub struct Viewer {
    pub user: Option<User>,
    pub session_key: Option<String>,
    pub csrf_token: String,
    /// The CSRF token came from the browser's cookie (a new one can't vouch for a form).
    pub csrf_from_cookie: bool,
}

impl Viewer {
    /// A logged-out visitor (what the static export renders as).
    pub fn anonymous() -> Self {
        Self { user: None, session_key: None, csrf_token: String::new(), csrf_from_cookie: false }
    }

    pub fn is_staff(&self) -> bool {
        self.user.as_ref().is_some_and(|u| u.is_active && u.is_staff)
    }
}

#[derive(Serialize)]
struct ProfileContext<'a> {
    #[serde(flatten)]
    profile: &'a Profile,
    hero_video_url: Option<String>,
}

/// The settings form as typed (strings, so a bad value can be shown back), plus its errors.
#[derive(Default, Serialize)]
pub struct ProfileForm {
    pub handle: String,
    pub name: String,
    pub intro: String,
    pub x_url: String,
    pub linkedin_url: String,
    pub github_url: String,
    pub email: String,
    pub latest_posts: String,
    pub errors: BTreeMap<&'static str, String>,
}

impl ProfileForm {
    pub fn from_profile(p: &Profile) -> Self {
        Self {
            handle: p.handle.clone(),
            name: p.name.clone(),
            intro: p.intro.clone(),
            x_url: p.x_url.clone(),
            linkedin_url: p.linkedin_url.clone(),
            github_url: p.github_url.clone(),
            email: p.email.clone(),
            latest_posts: p.latest_posts.to_string(),
            errors: BTreeMap::new(),
        }
    }
}

impl Site {
    pub fn new(config: Config, env: Environment<'static>) -> Result<Self> {
        let db = Db::open(&config.db_path)?;
        Ok(Self { config, db, env })
    }

    fn render(&self, template: &str, viewer: &Viewer, extra: Value) -> Result<String> {
        let profile = Profile::load(&mut self.db.conn())?;
        let csrf_input = format!(
            r#"<input type="hidden" name="csrfmiddlewaretoken" value="{}">"#,
            escape(&viewer.csrf_token)
        );
        let ctx = context! {
            profile => Serde(ProfileContext { hero_video_url: profile.hero_video_url(), profile: &profile }),
            user => context! { is_staff => viewer.is_staff(), is_authenticated => viewer.user.is_some() },
            csrf_input => Value::from_safe_string(csrf_input),
            ..extra
        };
        Ok(self.env.get_template(template)?.render(ctx)?)
    }

    pub fn home(&self, viewer: &Viewer) -> Result<String> {
        let posts = {
            let mut conn = self.db.conn();
            let count = Profile::load(&mut conn)?.latest_posts;
            if count > 0 { Post::list(&mut conn, count, 0)? } else { vec![] }
        };
        let posts: Vec<_> = posts.iter().map(Post::view).collect();
        self.render("home/home.html", viewer, context! { latest_posts => Serde(posts) })
    }

    pub fn cv(&self, viewer: &Viewer) -> Result<String> {
        self.render("home/cv.html", viewer, context! { cv => Serde(&CV) })
    }

    pub fn page_count(&self) -> Result<i64> {
        let count = Post::count(&mut self.db.conn())?;
        Ok(((count + POSTS_PER_PAGE - 1) / POSTS_PER_PAGE).max(1))
    }

    /// None if there is no such page.
    pub fn post_list(&self, viewer: &Viewer, page: i64) -> Result<Option<String>> {
        let pages = self.page_count()?;
        if page < 1 || page > pages {
            return Ok(None);
        }
        let posts = Post::list(&mut self.db.conn(), POSTS_PER_PAGE, (page - 1) * POSTS_PER_PAGE)?;
        let posts: Vec<_> = posts.iter().map(Post::view).collect();
        let ctx = context! {
            posts => Serde(posts),
            is_paginated => pages > 1,
            page => context! {
                has_previous => page > 1,
                previous_page_number => page - 1,
                has_next => page < pages,
                next_page_number => page + 1,
            },
        };
        self.render("blog/post_list.html", viewer, ctx).map(Some)
    }

    /// None if there is no such post.
    pub fn post_detail(&self, viewer: &Viewer, slug: &str) -> Result<Option<String>> {
        let Some(post) = Post::by_slug(&mut self.db.conn(), slug)? else {
            return Ok(None);
        };
        self.render("blog/post_detail.html", viewer, context! { post => Serde(post.view()) }).map(Some)
    }

    pub fn not_found(&self, viewer: &Viewer) -> Result<String> {
        self.render("404.html", viewer, context! {})
    }

    pub fn login(&self, viewer: &Viewer, next: &str, username: &str, failed: bool) -> Result<String> {
        self.render("login.html", viewer, context! { next, username, failed })
    }

    pub fn settings(&self, viewer: &Viewer, form: &ProfileForm, messages: &[String]) -> Result<String> {
        self.render("home/settings.html", viewer, context! { form => Serde(form), messages => Serde(messages) })
    }

    /// The writing page: a new post, or an existing one to edit.
    pub fn editor(&self, viewer: &Viewer, post: Option<&Post>) -> Result<String> {
        let initial = serde_json::json!({
            "id": post.map(|p| p.id),
            "title": post.map_or("", |p| p.title.as_str()),
            "delta": post.map(|p| serde_json::from_str::<serde_json::Value>(&p.body_delta).unwrap_or_default()),
            "url": post.map(Post::url),
        });
        let ctx = context! { post => Serde(post.map(Post::view)), initial => Serde(initial) };
        self.render("blog/write.html", viewer, ctx)
    }
}
