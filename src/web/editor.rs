//! The /write/ editor: its page and its background requests (save, image upload, delete).

use std::io::Cursor;
use std::path::Path as FsPath;

use anyhow::{Context, Result};
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use chrono::Datelike;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use serde_json::{Value, json};

use super::{AppState, Staff, StaffApi, WebResult, csrf_ok, json_error, not_found_page};
use crate::db::{self, Post};
use crate::rendering::render_article;
use crate::text::{random_string, slugify, truncate_chars};

const MAX_IMAGE_SIDE: u32 = 1600;
const MAX_UPLOAD_BYTES: usize = 15 * 1024 * 1024;

// ---------- The writing page ----------

pub async fn new_post(State(site): State<AppState>, Staff(viewer): Staff) -> WebResult {
    Ok(Html(site.editor(&viewer, None)?).into_response())
}

pub async fn edit_post(State(site): State<AppState>, Staff(viewer): Staff, Path(id): Path<String>) -> WebResult {
    let post = match id.parse() {
        Ok(id) => Post::by_id(&site.db.conn(), id)?,
        Err(_) => None,
    };
    match post {
        Some(post) => Ok(Html(site.editor(&viewer, Some(&post))?).into_response()),
        None => not_found_page(&site, &viewer).await,
    }
}

// ---------- Background requests ----------

fn security_failed() -> axum::response::Response {
    json_error(StatusCode::FORBIDDEN, "Security check failed. Reload the page and try again.")
}

fn unique_slug(conn: &rusqlite::Connection, title: &str, exclude_id: Option<i64>) -> Result<String> {
    let base = slugify(title);
    let base = match truncate_chars(&base, 70) {
        "" => "post",
        b => b,
    };
    let (mut slug, mut n) = (base.to_string(), 2);
    while Post::slug_taken(conn, &slug, exclude_id)? {
        slug = format!("{base}-{n}");
        n += 1;
    }
    Ok(slug)
}

/// JSON truthiness, like Python's.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub async fn save(State(site): State<AppState>, StaffApi(viewer): StaffApi, headers: HeaderMap, body: Bytes) -> WebResult {
    if !csrf_ok(&viewer, &headers, None) {
        return Ok(security_failed());
    }
    let Ok(Value::Object(data)) = serde_json::from_slice::<Value>(&body) else {
        return Ok(json_error(StatusCode::BAD_REQUEST, "Bad request."));
    };
    let get = |key: &str| data.get(key).filter(|v| truthy(v));

    let title = get("title").and_then(Value::as_str).unwrap_or("").trim();
    let title = truncate_chars(title, 255).to_string();
    if title.is_empty() {
        return Ok(json_error(StatusCode::BAD_REQUEST, "Add a title first."));
    }
    let html = get("html").and_then(Value::as_str).unwrap_or("");
    if html.trim().is_empty() {
        return Ok(json_error(StatusCode::BAD_REQUEST, "Write something first."));
    }
    let body_html = render_article(html);
    let body_delta = get("delta").cloned().unwrap_or_else(|| json!({})).to_string();

    let conn = site.db.conn();
    let mut post = match get("id") {
        Some(id) => {
            let id = id.as_i64().or_else(|| id.as_str().and_then(|s| s.trim().parse().ok()));
            // Keep the URL stable on edits.
            match id.map(|id| Post::by_id(&conn, id)).transpose()?.flatten() {
                Some(post) => post,
                None => return Ok(json_error(StatusCode::NOT_FOUND, "That post doesn't exist any more.")),
            }
        }
        None => Post {
            id: 0,
            title: String::new(),
            slug: unique_slug(&conn, &title, None)?,
            body_delta: String::new(),
            body_html: String::new(),
            created_at: db::now(),
            updated_at: db::now(),
        },
    };
    post.title = title;
    post.body_delta = body_delta;
    post.body_html = body_html;
    post.save(&conn)?;
    Ok(Json(json!({ "id": post.id, "url": post.url() })).into_response())
}

pub async fn upload(
    State(site): State<AppState>,
    StaffApi(viewer): StaffApi,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> WebResult {
    if !csrf_ok(&viewer, &headers, None) {
        return Ok(security_failed());
    }
    let mut upload = None;
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            let name = field.file_name().unwrap_or("").to_string();
            upload = Some((name, field.bytes().await?));
        }
    }
    let Some((file_name, bytes)) = upload.filter(|(_, b)| !b.is_empty()) else {
        return Ok(json_error(StatusCode::BAD_REQUEST, "No file."));
    };
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Ok(json_error(StatusCode::BAD_REQUEST, "That image is over 15 MB."));
    }
    let Ok((content, ext)) = tokio::task::spawn_blocking(move || shrink(&bytes)).await? else {
        return Ok(json_error(StatusCode::BAD_REQUEST, "That file isn't an image I can read."));
    };

    let stem = FsPath::new(&file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let name = match slugify(truncate_chars(stem, 60)) {
        s if s.is_empty() => "image".to_string(),
        s => s,
    };
    let now = db::now();
    let dir = format!("posts/{}/{:02}", now.year(), now.month());
    let file = save_media(&site.config.media_root, &dir, &format!("{name}.{ext}"), &content)?;
    let id = db::insert_post_image(&site.db.conn(), &file)?;
    Ok(Json(json!({ "id": id, "url": db::media_url(&file) })).into_response())
}

pub async fn delete(
    State(site): State<AppState>,
    StaffApi(viewer): StaffApi,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> WebResult {
    if !csrf_ok(&viewer, &headers, None) {
        return Ok(security_failed());
    }
    let deleted = match id.parse() {
        Ok(id) => Post::delete(&site.db.conn(), id)?,
        Err(_) => false,
    };
    if !deleted {
        return Ok(json_error(StatusCode::NOT_FOUND, "That post doesn't exist any more."));
    }
    Ok(Json(json!({ "url": "/blog/" })).into_response())
}

// ---------- Files ----------

/// Fix phone-photo rotation and cap the size at 1600px. Returns (bytes, extension).
fn shrink(bytes: &[u8]) -> Result<(Vec<u8>, &'static str)> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader.format().context("unknown image format")?;
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?; // also proves the file is readable

    if format == ImageFormat::Gif {
        return Ok((bytes.to_vec(), "gif")); // keep animations untouched
    }
    img.apply_orientation(orientation);
    if img.width() > MAX_IMAGE_SIDE || img.height() > MAX_IMAGE_SIDE {
        img = img.resize(MAX_IMAGE_SIDE, MAX_IMAGE_SIDE, FilterType::Lanczos3);
    }
    let mut out = Vec::new();
    if format == ImageFormat::Png || img.color().has_alpha() {
        img.write_with_encoder(PngEncoder::new_with_quality(&mut out, CompressionType::Best, PngFilter::Adaptive))?;
        return Ok((out, "png"));
    }
    img.to_rgb8().write_with_encoder(JpegEncoder::new_with_quality(&mut out, 85))?;
    Ok((out, "jpg"))
}

/// Write a file under media/<dir>/, picking a free name ("photo_x8Kq2Lm.jpg") if
/// one is taken. Returns its path relative to media/.
pub fn save_media(media_root: &FsPath, dir: &str, file_name: &str, bytes: &[u8]) -> Result<String> {
    let folder = media_root.join(dir);
    std::fs::create_dir_all(&folder).with_context(|| format!("can't create {}", folder.display()))?;
    let path = FsPath::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    let mut name = file_name.to_string();
    while folder.join(&name).exists() {
        name = format!("{stem}_{}{ext}", random_string(7));
    }
    let target = folder.join(&name);
    std::fs::write(&target, bytes).with_context(|| format!("can't write {}", target.display()))?;
    Ok(format!("{dir}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinks_and_rotates() {
        let img = DynamicImage::new_rgb8(3200, 1000);
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).unwrap();
        let (out, ext) = shrink(&png).unwrap();
        assert_eq!(ext, "png");
        let small = image::load_from_memory(&out).unwrap();
        assert_eq!((small.width(), small.height()), (1600, 500));

        let mut jpg = Vec::new();
        img.write_to(&mut Cursor::new(&mut jpg), ImageFormat::Jpeg).unwrap();
        assert_eq!(shrink(&jpg).unwrap().1, "jpg");
        assert!(shrink(b"not an image").is_err());
    }
}
