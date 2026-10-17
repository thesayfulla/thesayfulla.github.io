# sayfulla

Personal site + blog. Plain Django — no CMS.

## Run locally

```bash
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python manage.py migrate
.venv/bin/python manage.py createsuperuser
.venv/bin/python manage.py runserver
```

Site: http://localhost:8000 · Log in: http://localhost:8000/login/

When logged in you get a **write** link in the header and **settings · log out** in the footer.
Visitors see none of that.

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
.venv/bin/python manage.py changepassword sayfulla
```

## Layout

- `home/` — `Profile` (your name, bio, links), home page and settings page
- `blog/` — `Post` model, blog pages, the `/write/` editor views (`views.py`), article rendering (`rendering.py`)
- `portfolio/templates/` — `base.html` (header/footer on every page), login page
- `portfolio/static/` — CSS, hero animation + theme toggle (`js/site.js`), editor (`js/write.js`, `css/write.css`)
- `media/` — uploaded images and video (not in git; back it up together with `db.sqlite3`)
