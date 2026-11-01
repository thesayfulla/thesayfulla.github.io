#!/usr/bin/env bash
# Rebuild the static site into docs/ and push it to GitHub Pages.
#   ./publish.sh
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --quiet
./target/release/portfolio backup      # db + media -> ~/Documents/Portfolio Backups
./target/release/portfolio build-site

git add -A docs
if git diff --cached --quiet -- docs; then
  echo "Nothing changed on the site."
  exit 0
fi
git commit -m "Publish site" -- docs
git push
echo "Pushed. GitHub Pages updates in about a minute: https://thesayfulla.github.io/"
