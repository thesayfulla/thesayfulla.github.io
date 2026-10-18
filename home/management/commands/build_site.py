"""Export the public site as static files for GitHub Pages.

    .venv/bin/python manage.py build_site          # writes ./docs

Every public page is rendered as a logged-out visitor sees it and saved as
<path>/index.html. CSS/JS get content-hashed file names so browsers never
show a stale stylesheet after an update. Uploaded images and the hero video
are copied from media/. The editor, login and settings are not exported.
"""

import math
import shutil
from pathlib import Path

from django.conf import settings
from django.contrib.auth.models import AnonymousUser
from django.core.management import call_command
from django.core.management.base import BaseCommand, CommandError
from django.template.loader import render_to_string
from django.test import Client, RequestFactory, override_settings
from django.urls import reverse

from blog.models import Post
from blog.views import PostListView

MARKER = ".built-by-build_site"


class Command(BaseCommand):
    help = "Export the public site as static files (default: ./docs) for GitHub Pages."

    def add_arguments(self, parser):
        parser.add_argument("--out", default=str(settings.BASE_DIR / "docs"))

    def handle(self, *args, **options):
        out = Path(options["out"]).resolve()
        self.prepare(out)

        static_settings = {
            "DEBUG": False,
            "STATIC_ROOT": out / "static",
            "STORAGES": {
                **settings.STORAGES,
                "staticfiles": {"BACKEND": "django.contrib.staticfiles.storage.ManifestStaticFilesStorage"},
            },
        }
        with override_settings(**static_settings):
            call_command("collectstatic", interactive=False, verbosity=0)
            pages = self.export_pages(out)
            self.export_404(out)

        media = Path(settings.MEDIA_ROOT)
        if media.exists():
            shutil.copytree(media, out / "media")

        (out / ".nojekyll").touch()  # serve files as-is, no Jekyll processing
        self.stdout.write(self.style.SUCCESS(f"Built {pages} pages into {out}"))

    def prepare(self, out):
        """Start from an empty folder, but only ever delete a folder this command made."""
        if out.exists():
            if not (out / MARKER).exists() and any(out.iterdir()):
                raise CommandError(f"{out} has files this command didn't create; not touching it.")
            shutil.rmtree(out)
        out.mkdir(parents=True)
        (out / MARKER).touch()

    def urls(self):
        yield reverse("home")
        yield reverse("cv")
        yield reverse("blog:list")
        pages = math.ceil(Post.objects.count() / PostListView.paginate_by)
        for n in range(2, pages + 1):
            yield reverse("blog:list-page", args=[n])
        for post in Post.objects.all():
            yield post.get_absolute_url()

    def export_pages(self, out):
        client = Client()  # logged out: no write/settings links
        count = 0
        for url in self.urls():
            response = client.get(url)
            if response.status_code != 200:
                raise CommandError(f"{url} returned {response.status_code}")
            target = out / url.strip("/") / "index.html"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(response.content)
            self.stdout.write(f"  {url}")
            count += 1
        return count

    def export_404(self, out):
        request = RequestFactory().get("/404/")
        request.user = AnonymousUser()
        (out / "404.html").write_text(render_to_string("404.html", request=request), encoding="utf-8")
