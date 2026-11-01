//! Public pages, login/logout and the settings page.

use std::collections::HashMap;
use std::path::Path as FsPath;
use std::sync::LazyLock;

use axum::extract::{Form, Multipart, Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Redirect};
use chrono::Duration;
use regex::Regex;
use url::Url;

use super::editor::save_media;
use super::{
    AppState, SESSION_AGE_SECS, SESSION_COOKIE, Staff, WebResult, csrf_failed, csrf_ok, is_get, not_found_page,
    safe_next, set_cookie,
};
use crate::auth;
use crate::config::LOGIN_REDIRECT_URL;
use crate::db::{self, Profile, User};
use crate::pages::{ProfileForm, Viewer};
use crate::rendering::sanitize;
use crate::text::{random_string, valid_filename};

// ---------- Public pages ----------

pub async fn home(State(site): State<AppState>, viewer: Viewer) -> WebResult<Html<String>> {
    Ok(Html(site.home(&viewer)?))
}

pub async fn cv(State(site): State<AppState>, viewer: Viewer) -> WebResult<Html<String>> {
    Ok(Html(site.cv(&viewer)?))
}

pub async fn post_list(State(site): State<AppState>, viewer: Viewer) -> WebResult {
    list_page(&site, &viewer, 1).await
}

pub async fn post_list_page(State(site): State<AppState>, viewer: Viewer, Path(page): Path<String>) -> WebResult {
    match page.parse() {
        Ok(page) => list_page(&site, &viewer, page).await,
        Err(_) => not_found_page(&site, &viewer).await,
    }
}

async fn list_page(site: &AppState, viewer: &Viewer, page: i64) -> WebResult {
    match site.post_list(viewer, page)? {
        Some(html) => Ok(Html(html).into_response()),
        None => not_found_page(site, viewer).await,
    }
}

pub async fn post_detail(State(site): State<AppState>, viewer: Viewer, Path(slug): Path<String>) -> WebResult {
    match site.post_detail(&viewer, &slug)? {
        Some(html) => Ok(Html(html).into_response()),
        None => not_found_page(&site, &viewer).await,
    }
}

/// Unknown URLs: add a missing trailing slash if that finds a page, else 404.
pub async fn not_found(State(site): State<AppState>, viewer: Viewer, method: Method, uri: Uri) -> WebResult {
    static ROUTES: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^/(cv|settings|login|logout|blog|blog/page/\d+|blog/[-\w]+|write|write/\d+)/$").unwrap()
    });
    let path = uri.path();
    if is_get(&method) && !path.ends_with('/') && ROUTES.is_match(&format!("{path}/")) {
        let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
        return Ok(Redirect::permanent(&format!("{path}/{query}")).into_response());
    }
    not_found_page(&site, &viewer).await
}

// ---------- Login / logout ----------

pub async fn login_page(
    State(site): State<AppState>,
    viewer: Viewer,
    Query(query): Query<HashMap<String, String>>,
) -> WebResult {
    let next = safe_next(query.get("next").map(String::as_str));
    if viewer.user.is_some() {
        return Ok(Redirect::to(next.as_deref().unwrap_or(LOGIN_REDIRECT_URL)).into_response());
    }
    Ok(Html(site.login(&viewer, next.as_deref().unwrap_or(""), "", false)?).into_response())
}

pub async fn login(
    State(site): State<AppState>,
    viewer: Viewer,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> WebResult {
    let field = |name: &str| form.get(name).map(String::as_str).unwrap_or("");
    if !csrf_ok(&viewer, &headers, form.get("csrfmiddlewaretoken").map(String::as_str)) {
        return Ok(csrf_failed());
    }
    let next = safe_next(Some(field("next")));
    let username = field("username").to_string();
    let password = field("password").to_string();

    // Password hashing is deliberately slow; keep it off the async workers.
    let checker = site.clone();
    let name = username.clone();
    let user = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<User>> {
        let Some(user) = User::by_username(&mut checker.db.conn(), &name)? else {
            auth::waste_time(&password);
            return Ok(None);
        };
        Ok((auth::verify_password(&password, &user.password) && user.is_active).then_some(user))
    })
    .await??;

    let Some(user) = user else {
        return Ok(Html(site.login(&viewer, next.as_deref().unwrap_or(""), &username, true)?).into_response());
    };
    let key = random_string(32);
    {
        let mut conn = site.db.conn();
        if let Some(old) = &viewer.session_key {
            db::delete_session(&mut conn, old)?;
        }
        db::create_session(&mut conn, &key, user.id, &(db::now() + Duration::seconds(SESSION_AGE_SECS)))?;
        User::touch_last_login(&mut conn, user.id)?;
    }
    let mut res = Redirect::to(next.as_deref().unwrap_or(LOGIN_REDIRECT_URL)).into_response();
    set_cookie(&mut res, SESSION_COOKIE, &key, SESSION_AGE_SECS, true);
    Ok(res)
}

pub async fn logout(
    State(site): State<AppState>,
    viewer: Viewer,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> WebResult {
    if !csrf_ok(&viewer, &headers, form.get("csrfmiddlewaretoken").map(String::as_str)) {
        return Ok(csrf_failed());
    }
    if let Some(key) = &viewer.session_key {
        db::delete_session(&mut site.db.conn(), key)?;
    }
    let mut res = Redirect::to("/").into_response();
    set_cookie(&mut res, SESSION_COOKIE, "", 0, true);
    Ok(res)
}

// ---------- Settings ----------

pub async fn settings_page(State(site): State<AppState>, Staff(viewer): Staff) -> WebResult {
    let (form, messages) = {
        let mut conn = site.db.conn();
        let form = ProfileForm::from_profile(&Profile::load(&mut conn)?);
        let flash = match &viewer.session_key {
            Some(key) => db::take_flash(&mut conn, key)?,
            None => None,
        };
        (form, flash.into_iter().collect::<Vec<_>>())
    };
    Ok(Html(site.settings(&viewer, &form, &messages)?).into_response())
}

struct Upload {
    file_name: String,
    bytes: Vec<u8>,
}

pub async fn settings_save(
    State(site): State<AppState>,
    Staff(viewer): Staff,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> WebResult {
    let mut fields = HashMap::new();
    let mut video = None;
    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name == "hero_video" {
            let file_name = field.file_name().unwrap_or("").to_string();
            let bytes = field.bytes().await?;
            if !file_name.is_empty() {
                video = Some(Upload { file_name, bytes: bytes.to_vec() });
            }
        } else {
            fields.insert(name, field.text().await?);
        }
    }
    if !csrf_ok(&viewer, &headers, fields.get("csrfmiddlewaretoken").map(String::as_str)) {
        return Ok(csrf_failed());
    }

    let (form, mut profile) = clean_profile_form(&fields, video.as_ref(), Profile::load(&mut site.db.conn())?);
    if !form.errors.is_empty() {
        let html = site.settings(&viewer, &form, &[])?;
        return Ok((StatusCode::OK, Html(html)).into_response());
    }

    let media = &site.config.media_root;
    if let Some(video) = video {
        let name = FsPath::new(&video.file_name).file_name().and_then(|n| n.to_str()).unwrap_or("video");
        profile.hero_video = save_media(media, "hero", &valid_filename(name), &video.bytes)?;
    } else if fields.contains_key("remove_video") && !profile.hero_video.is_empty() {
        let _ = std::fs::remove_file(media.join(&profile.hero_video));
        profile.hero_video.clear();
    }

    let mut conn = site.db.conn();
    profile.save(&mut conn)?;
    if let Some(key) = &viewer.session_key {
        db::set_flash(&mut conn, key, "Saved.")?;
    }
    Ok(Redirect::to("/settings/").into_response())
}

/// Check the settings form. Returns the form as typed (with any errors) and the
/// profile with the new values applied.
fn clean_profile_form(
    fields: &HashMap<String, String>,
    video: Option<&Upload>,
    mut profile: Profile,
) -> (ProfileForm, Profile) {
    let value = |name: &str| fields.get(name).map(|v| v.trim().to_string()).unwrap_or_default();
    let mut form = ProfileForm {
        handle: value("handle"),
        name: value("name"),
        intro: value("intro"),
        x_url: value("x_url"),
        linkedin_url: value("linkedin_url"),
        github_url: value("github_url"),
        email: value("email"),
        latest_posts: value("latest_posts"),
        errors: Default::default(),
    };
    let errors = &mut form.errors;

    let mut text = |key: &'static str, value: &str, max: usize| -> String {
        let len = value.chars().count();
        if value.is_empty() {
            errors.insert(key, "This field is required.".into());
        } else if len > max {
            errors.insert(key, format!("Ensure this value has at most {max} characters (it has {len})."));
        }
        value.to_string()
    };
    profile.handle = text("handle", &form.handle, 60);
    profile.name = text("name", &form.name, 100);
    profile.intro = sanitize(&form.intro).replace("&nbsp;", " ");

    for (key, value, target) in [
        ("x_url", &form.x_url, &mut profile.x_url),
        ("linkedin_url", &form.linkedin_url, &mut profile.linkedin_url),
        ("github_url", &form.github_url, &mut profile.github_url),
    ] {
        match clean_url(value) {
            Ok(url) => *target = url,
            Err(message) => {
                errors.insert(key, message);
            }
        }
    }

    static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s.]{2,}$").unwrap());
    if !form.email.is_empty() && (!EMAIL.is_match(&form.email) || form.email.len() > 254) {
        errors.insert("email", "Enter a valid email address.".into());
    } else {
        profile.email = form.email.clone();
    }

    match form.latest_posts.parse::<i64>() {
        _ if form.latest_posts.is_empty() => {
            errors.insert("latest_posts", "This field is required.".into());
        }
        Err(_) => {
            errors.insert("latest_posts", "Enter a whole number.".into());
        }
        Ok(n) if n < 0 => {
            errors.insert("latest_posts", "Ensure this value is greater than or equal to 0.".into());
        }
        Ok(n) if n > 32767 => {
            errors.insert("latest_posts", "Ensure this value is less than or equal to 32767.".into());
        }
        Ok(n) => profile.latest_posts = n,
    }

    if let Some(video) = video {
        let ext = FsPath::new(&video.file_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !matches!(ext.as_str(), "mp4" | "webm") {
            errors.insert(
                "hero_video",
                format!("File extension “{ext}” is not allowed. Allowed extensions are: mp4, webm."),
            );
        } else if video.bytes.is_empty() {
            errors.insert("hero_video", "The submitted file is empty.".into());
        }
    }
    (form, profile)
}

/// An optional link: empty is fine, "example.com" becomes "https://example.com".
fn clean_url(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let value = if value.contains("://") { value.to_string() } else { format!("https://{value}") };
    let invalid = || "Enter a valid URL.".to_string();
    let url = Url::parse(&value).map_err(|_| invalid())?;
    let host = url.host_str().unwrap_or("");
    let host_ok = host.contains('.') || host == "localhost";
    if !matches!(url.scheme(), "http" | "https" | "ftp" | "ftps") || !host_ok {
        return Err(invalid());
    }
    let len = value.chars().count();
    if len > 200 {
        return Err(format!("Ensure this value has at most 200 characters (it has {len})."));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(clean_url("github.com/me").unwrap(), "https://github.com/me");
        assert_eq!(clean_url("https://x.com/me").unwrap(), "https://x.com/me");
        assert!(clean_url("javascript:alert(1)").is_err());
        assert!(clean_url("not a url").is_err());
    }

    #[test]
    fn profile_form() {
        let fields: HashMap<String, String> = [
            ("handle", " me "), ("name", ""), ("intro", "<p>hi<script>x</script>&nbsp;</p>"),
            ("x_url", ""), ("linkedin_url", "nope"), ("github_url", "github.com/me"),
            ("email", "a@b.co"), ("latest_posts", "-1"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let (form, profile) = clean_profile_form(&fields, None, Profile::default());
        assert_eq!(profile.handle, "me");
        assert_eq!(profile.intro, "<p>hi </p>");
        assert_eq!(profile.github_url, "https://github.com/me");
        assert_eq!(form.errors.keys().copied().collect::<Vec<_>>(), ["latest_posts", "linkedin_url", "name"]);
    }
}
