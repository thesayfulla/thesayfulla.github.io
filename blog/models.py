import math

from django.db import models
from django.urls import reverse

from .rendering import plain_text


class Post(models.Model):
    title = models.CharField(max_length=255)
    slug = models.SlugField(max_length=80, unique=True)
    # The editor's own document (used to re-open the post for editing)
    # and the rendered article HTML (what visitors see).
    body_delta = models.JSONField(default=dict, blank=True)
    body_html = models.TextField(blank=True)
    created_at = models.DateTimeField(auto_now_add=True)
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        ordering = ["-created_at"]

    def __str__(self):
        return self.title

    def get_absolute_url(self):
        return reverse("blog:post", args=[self.slug])

    @property
    def reading_minutes(self):
        return max(1, math.ceil(len(plain_text(self.body_html).split()) / 220))

    @property
    def excerpt(self):
        text = plain_text(self.body_html)
        return text if len(text) <= 160 else text[:157].rsplit(" ", 1)[0] + "…"


class PostImage(models.Model):
    """An image uploaded from the editor. Kept even if removed from a post, so old links don't break."""

    file = models.ImageField(upload_to="posts/%Y/%m/")
    created_at = models.DateTimeField(auto_now_add=True)

    def __str__(self):
        return self.file.name
