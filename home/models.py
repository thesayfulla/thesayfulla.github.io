from django.core.validators import FileExtensionValidator
from django.db import models

DEFAULT_INTRO = (
    "<p>I’m a software engineer currently building <a href=\"#\">Your Project</a>. "
    "Previously, I studied computer science at <a href=\"#\">Your University</a> and worked as an "
    "engineer at <a href=\"#\">Company One</a> and <a href=\"#\">Company Two</a>.</p>"
    "<p>My first taste of software success was when I built <a href=\"#\">something people loved</a> "
    "— and I’ve been shipping things ever since.</p>"
)


class Profile(models.Model):
    """Everything about you shown on the site. There is only ever one row; edit it at /settings/."""

    handle = models.CharField(max_length=60, default="sayfulla", help_text="Shown at the top of every page.")
    name = models.CharField(max_length=100, default="Sayfulla Mirxoliqov", help_text="Used in browser tab titles.")
    intro = models.TextField(default=DEFAULT_INTRO, blank=True)
    hero_video = models.FileField(
        upload_to="hero/",
        blank=True,
        validators=[FileExtensionValidator(["mp4", "webm"])],
    )
    x_url = models.URLField("X", blank=True)
    linkedin_url = models.URLField("LinkedIn", blank=True)
    github_url = models.URLField("GitHub", blank=True)
    email = models.EmailField(blank=True, default="mirxoliqov.sayfulla@gmail.com")
    latest_posts = models.PositiveSmallIntegerField(default=5)

    def __str__(self):
        return self.name

    def save(self, *args, **kwargs):
        self.pk = 1
        super().save(*args, **kwargs)

    @classmethod
    def load(cls):
        obj, _ = cls.objects.get_or_create(pk=1)
        return obj
