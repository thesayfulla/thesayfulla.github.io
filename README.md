# Personal site

## Run locally

```bash
cargo run -- create-user sayfulla
cargo run -- serve
```

Site: http://localhost:8000 · Log in: http://localhost:8000/login/

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

## Publishing

```bash
./publish.sh
```
