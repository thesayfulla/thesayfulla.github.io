# sayfulla

Personal site + blog. Rust (axum + minijinja + Diesel on SQLite) — no CMS.

## Run locally

```bash
cargo run -- create-user sayfulla      # first time only; the old db.sqlite3 users still work
cargo run -- serve
```

Site: http://localhost:8000 · Log in: http://localhost:8000/login/

With [just](https://github.com/casey/just): `just run`, `just create-user`, `just build-site`, `just publish`.

When logged in you get a **write** link in the header and **settings · log out** in the footer.
Visitors see none of that. The database tables are created on first start.

## Writing posts

Go to **/write/** (or click **write** in the header).

- Type a title, press Enter, write your story, click **Publish**.
- **Select text** for the toolbar: bold, italic, link, inline code, heading, subheading, quote.
- **On an empty line**, buttons appear on the left: image, embed, code block.
- **Images**: use the camera button, or just paste / drag an image in. Click under it to add a caption.
- **Embeds**: paste a YouTube, Vimeo, X or GitHub gist link on its own line and press Enter.
- **Shortcuts**: `## ` heading, `### ` subheading, `> ` quote, `- ` or `1. ` list,
  ` ```python ` + Enter for a code block, ⌘B / ⌘I / ⌘K, ⌘Enter to publish.
- To change or delete a post, open it and click **edit**.
- Drafts of a new post are kept in your browser until you publish.

## Settings

**/settings/** — header text, full name, bio, social links, optional hero video,
and how many recent posts the home page lists.

Change your password with:

```bash
cargo run -- change-password sayfulla
```

## Publishing

```bash
./publish.sh
```

Backs up the database and media, exports the public site into `docs/`
(`cargo run -- build-site`), commits and pushes it to GitHub Pages.

## Layout

- `src/main.rs` — the commands: `serve`, `build-site`, `backup`, `create-user`, `change-password`
- `src/pages.rs` — every page, shared by the server and the static export
- `src/web/` — routes, login/settings (`handlers.rs`), the `/write/` editor and its uploads (`editor.rs`)
- `src/rendering.rs` — editor HTML → article HTML; `src/highlight.rs` — code highlighting
- `src/db.rs` — posts, profile, users, sessions (Diesel ORM on SQLite, same tables as the old Django version)
- `src/cv.rs` — résumé content for `/cv/`; edit it to update the page
- `templates/` — Jinja templates; `base.html` has the header/footer on every page
- `static/` — CSS, hero animation + theme toggle (`js/site.js`), editor (`js/write.js`, `css/write.css`)
- `media/` — uploaded images and video (not in git; back it up together with `db.sqlite3`)

`cargo test` checks the renderer against output captured from the old Python version
(`tests/fixtures/python_reference.json`).
