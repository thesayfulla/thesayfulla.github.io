//! The web server: public pages, login, settings and the editor.

mod editor;
mod handlers;

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Request};
use axum::http::header::{CONTENT_TYPE, COOKIE, HOST, ORIGIN, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use subtle::ConstantTimeEq;
use tower_http::services::ServeDir;

use crate::config::{Config, LOGIN_URL};
use crate::db::User;
use crate::pages::{Site, Viewer};
use crate::templates;
use crate::text::random_string;

pub type AppState = Arc<Site>;

const SESSION_COOKIE: &str = "sessionid";
const SESSION_AGE_SECS: i64 = 14 * 24 * 60 * 60;
const CSRF_COOKIE: &str = "csrftoken";
const CSRF_COOKIE_AGE_SECS: i64 = 52 * 7 * 24 * 60 * 60;

pub async fn serve(config: Config, host: &str, port: u16) -> Result<()> {
    let env = templates::environment(&config.templates_dir, None);
    let site = Arc::new(Site::new(config, env)?);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Serving on http://{addr}/  (log in at http://{addr}/login/)");
    axum::serve(listener, router(site))
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.ok(); })
        .await?;
    Ok(())
}

pub fn router(site: AppState) -> Router {
    let static_files = ServeDir::new(&site.config.static_dir);
    let media_files = ServeDir::new(&site.config.media_root);
    Router::new()
        .route("/", get(handlers::home))
        .route("/cv/", get(handlers::cv))
        .route(
            "/settings/",
            get(handlers::settings_page)
                .post(handlers::settings_save)
                // The hero video comes in through this form.
                .layer(DefaultBodyLimit::max(500 * 1024 * 1024)),
        )
        .route("/login/", get(handlers::login_page).post(handlers::login))
        .route("/logout/", post(handlers::logout))
        .route("/blog/", get(handlers::post_list))
        .route("/blog/page/{page}/", get(handlers::post_list_page))
        .route("/blog/{slug}/", get(handlers::post_detail))
        .route("/write/", get(editor::new_post))
        .route("/write/{post_id}/", get(editor::edit_post))
        .route("/write/save/", post(editor::save).layer(DefaultBodyLimit::max(60 * 1024 * 1024)))
        .route("/write/upload/", post(editor::upload).layer(DefaultBodyLimit::max(64 * 1024 * 1024)))
        .route("/write/{post_id}/delete/", post(editor::delete))
        .nest_service("/static", static_files)
        .nest_service("/media", media_files)
        .fallback(handlers::not_found)
        .layer(middleware::from_fn(base_layer))
        .with_state(site)
}

// ---------- Cookies, CSRF, security headers ----------

pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

pub fn set_cookie(res: &mut Response, name: &str, value: &str, max_age: i64, http_only: bool) {
    let http_only = if http_only { "; HttpOnly" } else { "" };
    let header = format!("{name}={value}; Max-Age={max_age}; Path=/; SameSite=Lax{http_only}");
    if let Ok(v) = HeaderValue::from_str(&header) {
        res.headers_mut().append(SET_COOKIE, v);
    }
}

#[derive(Clone)]
struct Csrf {
    token: String,
    from_cookie: bool,
}

/// Hands every request a CSRF token (from its cookie, or a new one that is then
/// set), and adds the usual security headers.
async fn base_layer(mut req: Request, next: Next) -> Response {
    let existing = cookie(req.headers(), CSRF_COOKIE)
        .filter(|t| (16..=64).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric()));
    let csrf = match existing {
        Some(token) => Csrf { token, from_cookie: true },
        None => Csrf { token: random_string(32), from_cookie: false },
    };
    req.extensions_mut().insert(csrf.clone());
    let mut res = next.run(req).await;

    let is_html = res.headers().get(CONTENT_TYPE).is_some_and(|v| v.as_bytes().starts_with(b"text/html"));
    if !csrf.from_cookie && is_html {
        set_cookie(&mut res, CSRF_COOKIE, &csrf.token, CSRF_COOKIE_AGE_SECS, false);
    }
    let headers = res.headers_mut();
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    headers.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    res
}

/// A POST is genuine if it carries the token from the browser's own cookie, either as
/// the `csrfmiddlewaretoken` form field or the `X-CSRFToken` header, and (when the
/// browser says where it came from) it came from this site.
pub fn csrf_ok(viewer: &Viewer, headers: &HeaderMap, form_token: Option<&str>) -> bool {
    if !viewer.csrf_from_cookie {
        return false;
    }
    if let Some(origin) = headers.get(ORIGIN).and_then(|v| v.to_str().ok()) {
        let origin_host = origin.split_once("://").map_or(origin, |(_, rest)| rest);
        let host = headers.get(HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
        if origin_host != host {
            return false;
        }
    }
    let given = form_token.or_else(|| headers.get("x-csrftoken").and_then(|v| v.to_str().ok()));
    given.is_some_and(|g| bool::from(g.as_bytes().ct_eq(viewer.csrf_token.as_bytes())))
}

pub fn csrf_failed() -> Response {
    let body = "<!DOCTYPE html><title>403 Forbidden</title><h1>Forbidden (403)</h1>\
                <p>CSRF verification failed. Reload the page and try again.</p>";
    (StatusCode::FORBIDDEN, Html(body)).into_response()
}

// ---------- Who is asking ----------

impl FromRequestParts<AppState> for Viewer {
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, site: &AppState) -> Result<Self, WebError> {
        let csrf = parts.extensions.get::<Csrf>().cloned().unwrap_or(Csrf { token: String::new(), from_cookie: false });
        let session_key = cookie(&parts.headers, SESSION_COOKIE).filter(|k| !k.is_empty());
        let user = match &session_key {
            Some(key) => User::by_session(&mut site.db.conn(), key)?,
            None => None,
        };
        Ok(Viewer { user, session_key, csrf_token: csrf.token, csrf_from_cookie: csrf.from_cookie })
    }
}

/// Only you (a staff user) may see the page; everyone else goes to the login page.
pub struct Staff(pub Viewer);

impl FromRequestParts<AppState> for Staff {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, site: &AppState) -> Result<Self, Response> {
        let viewer = Viewer::from_request_parts(parts, site).await.map_err(IntoResponse::into_response)?;
        if viewer.is_staff() {
            return Ok(Staff(viewer));
        }
        if viewer.user.is_some() {
            return Err((StatusCode::FORBIDDEN, Html("<h1>403 Forbidden</h1>")).into_response());
        }
        let full_path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
        Err(Redirect::to(&format!("{LOGIN_URL}?next={}", quote(full_path))).into_response())
    }
}

/// Same check for the editor's background requests, answered with JSON instead of a redirect.
pub struct StaffApi(pub Viewer);

impl FromRequestParts<AppState> for StaffApi {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, site: &AppState) -> Result<Self, Response> {
        let viewer = Viewer::from_request_parts(parts, site).await.map_err(IntoResponse::into_response)?;
        if viewer.is_staff() {
            return Ok(StaffApi(viewer));
        }
        Err(json_error(StatusCode::FORBIDDEN, "You were logged out. Log in again in another tab, then retry."))
    }
}

pub fn json_error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": message }))).into_response()
}

/// Percent-encode for a query value, keeping `/` readable.
fn quote(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A redirect target from the request is only followed if it stays on this site.
pub fn safe_next(next: Option<&str>) -> Option<String> {
    let next = next?.trim();
    let ok = next.starts_with('/')
        && !next.starts_with("//")
        && !next.starts_with("/\\")
        && !next.chars().any(char::is_control);
    ok.then(|| next.to_string())
}

// ---------- Errors ----------

/// Any unexpected failure: logged, and answered with the 500 page.
pub struct WebError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for WebError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        eprintln!("Internal server error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, Html(include_str!("../../templates/500.html"))).into_response()
    }
}

pub type WebResult<T = Response> = Result<T, WebError>;

pub async fn not_found_page(site: &Site, viewer: &Viewer) -> WebResult {
    Ok((StatusCode::NOT_FOUND, Html(site.not_found(viewer)?)).into_response())
}

pub fn is_get(method: &Method) -> bool {
    method == Method::GET || method == Method::HEAD
}
