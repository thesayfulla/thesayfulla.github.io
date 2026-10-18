"""Back up everything that isn't in git: the database (your posts, bio, settings)
and media/ (uploaded images, hero video).

    .venv/bin/python manage.py backup

Writes portfolio-YYYYMMDD-HHMMSS.zip to ~/Documents/Portfolio Backups
(or $PORTFOLIO_BACKUP_DIR) and keeps the newest 30. publish.sh runs this
before every publish.

To restore: stop the server, then unzip a backup into the project folder
(it contains db.sqlite3 and media/).
"""

import os
import sqlite3
import tempfile
import zipfile
from datetime import datetime
from pathlib import Path

from django.conf import settings
from django.core.management.base import BaseCommand, CommandError

KEEP = 30


class Command(BaseCommand):
    help = "Zip the database and media/ into a dated backup."

    def add_arguments(self, parser):
        default = os.environ.get("PORTFOLIO_BACKUP_DIR", str(Path.home() / "Documents" / "Portfolio Backups"))
        parser.add_argument("--dest", default=default)

    def handle(self, *args, **options):
        dest = Path(options["dest"]).expanduser()
        dest.mkdir(parents=True, exist_ok=True)
        db_path = Path(settings.DATABASES["default"]["NAME"])
        if not db_path.exists():
            raise CommandError(f"No database at {db_path}")
        media = Path(settings.MEDIA_ROOT)

        target = dest / f"portfolio-{datetime.now():%Y%m%d-%H%M%S}.zip"
        with tempfile.TemporaryDirectory() as tmp:
            snapshot = Path(tmp) / "db.sqlite3"
            # SQLite's backup API gives a consistent copy even while the server is running.
            with sqlite3.connect(db_path) as src, sqlite3.connect(snapshot) as dst:
                src.backup(dst)

            files = 1
            with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as zf:
                zf.write(snapshot, "db.sqlite3")
                if media.exists():
                    for path in sorted(media.rglob("*")):
                        if path.is_file() and path.name != ".DS_Store":
                            zf.write(path, Path("media") / path.relative_to(media))
                            files += 1

        old = sorted(dest.glob("portfolio-*.zip"))[:-KEEP]
        for path in old:
            path.unlink()

        size = target.stat().st_size / 1024 / 1024
        self.stdout.write(self.style.SUCCESS(f"Backed up {files} files ({size:.1f} MB) to {target}"))
