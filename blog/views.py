import io
import json
from pathlib import Path

from django.core.files.base import ContentFile
from django.http import JsonResponse
from django.shortcuts import get_object_or_404
from django.utils.text import slugify
from django.views import View
from django.views.generic import DetailView, ListView, TemplateView
from PIL import Image, ImageOps, UnidentifiedImageError

from portfolio.mixins import StaffRequiredJSONMixin, StaffRequiredMixin

from .models import Post, PostImage
from .rendering import render_article

MAX_IMAGE_SIDE = 1600
MAX_UPLOAD_BYTES = 15 * 1024 * 1024


# ---------- Public pages ----------

class PostListView(ListView):
    model = Post
    template_name = "blog/post_list.html"
    context_object_name = "posts"
    paginate_by = 20


class PostDetailView(DetailView):
    model = Post
    template_name = "blog/post_detail.html"


# ---------- Editor ----------

def unique_slug(title, exclude_id=None):
    base = slugify(title)[:70] or "post"
    slug, n = base, 2
    others = Post.objects.exclude(id=exclude_id)
    while others.filter(slug=slug).exists():
        slug, n = f"{base}-{n}", n + 1
    return slug


class EditorView(StaffRequiredMixin, TemplateView):
    """The writing page: /write/ for a new post, /write/<id>/ to edit one."""

    template_name = "blog/write.html"

    def get_context_data(self, **kwargs):
        context = super().get_context_data(**kwargs)
        post_id = self.kwargs.get("post_id")
        post = get_object_or_404(Post, id=post_id) if post_id else None
        context["post"] = post
        context["initial"] = {
            "id": post.id if post else None,
            "title": post.title if post else "",
            "delta": post.body_delta if post else None,
            "url": post.get_absolute_url() if post else None,
        }
        return context


class EditorAPIView(StaffRequiredJSONMixin, View):
    """Base for the editor's background requests: staff only, POST only, JSON replies."""

    http_method_names = ["post"]


class PostSaveView(EditorAPIView):
    def post(self, request):
        try:
            data = json.loads(request.body)
        except ValueError:
            return JsonResponse({"error": "Bad request."}, status=400)

        title = (data.get("title") or "").strip()[:255]
        if not title:
            return JsonResponse({"error": "Add a title first."}, status=400)
        if not (data.get("html") or "").strip():
            return JsonResponse({"error": "Write something first."}, status=400)

        if data.get("id"):
            post = get_object_or_404(Post, id=data["id"])  # keep the URL stable on edits
        else:
            post = Post(slug=unique_slug(title))

        post.title = title
        post.body_delta = data.get("delta") or {}
        post.body_html = render_article(data["html"])
        post.save()
        return JsonResponse({"id": post.id, "url": post.get_absolute_url()})


class ImageUploadView(EditorAPIView):
    def post(self, request):
        upload = request.FILES.get("file")
        if not upload:
            return JsonResponse({"error": "No file."}, status=400)
        if upload.size > MAX_UPLOAD_BYTES:
            return JsonResponse({"error": "That image is over 15 MB."}, status=400)
        try:
            content, ext = self.shrink(upload)
        except (UnidentifiedImageError, OSError):
            return JsonResponse({"error": "That file isn't an image I can read."}, status=400)

        name = slugify(Path(upload.name).stem[:60]) or "image"
        image = PostImage()
        image.file.save(f"{name}.{ext}", ContentFile(content), save=True)
        return JsonResponse({"id": image.id, "url": image.file.url})

    @staticmethod
    def shrink(upload):
        """Fix phone-photo rotation and cap the size at 1600px. Returns (bytes, extension)."""
        img = Image.open(upload)
        img.verify()
        upload.seek(0)
        img = Image.open(upload)

        if img.format == "GIF":  # keep animations untouched
            upload.seek(0)
            return upload.read(), "gif"

        fmt = img.format or "PNG"
        img = ImageOps.exif_transpose(img)
        img.thumbnail((MAX_IMAGE_SIDE, MAX_IMAGE_SIDE))
        buf = io.BytesIO()
        if fmt == "PNG" or img.mode in ("RGBA", "LA", "P"):
            img.save(buf, "PNG", optimize=True)
            return buf.getvalue(), "png"
        img.convert("RGB").save(buf, "JPEG", quality=85, optimize=True, progressive=True)
        return buf.getvalue(), "jpg"


class PostDeleteView(EditorAPIView):
    def post(self, request, post_id):
        get_object_or_404(Post, id=post_id).delete()
        return JsonResponse({"url": "/blog/"})
