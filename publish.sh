#!/usr/bin/env bash
# Rebuild the static site into docs/ and push it to GitHub Pages.
#   ./publish.sh
set -euo pipefail
cd "$(dirname "$0")"

.venv/bin/python manage.py backup      # db + media -> ~/Documents/Portfolio Backups
.venv/bin/python manage.py build_site

git add -A docs
if git diff --cached --quiet -- docs; then
  echo "Nothing changed on the site."
  exit 0
fi
git commit -m "Publish site" -- docs
git push
echo "Pushed. GitHub Pages updates in about a minute: https://thesayfulla.github.io/"
