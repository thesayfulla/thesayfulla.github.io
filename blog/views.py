import io
import json
from functools import wraps
from pathlib import Path

from django.core.files.base import ContentFile
from django.core.paginator import Paginator
from django.http import JsonResponse
from django.shortcuts import get_object_or_404, redirect, render
from django.utils.text import slugify
from django.views.decorators.http import require_POST
from PIL import Image, ImageOps, UnidentifiedImageError

from home.views import staff_required

from .models import Post, PostImage
from .rendering import render_article

MAX_IMAGE_SIDE = 1600
MAX_UPLOAD_BYTES = 15 * 1024 * 1024


def api_staff_required(view):
    """Like staff_required, but answers with JSON instead of redirecting to the login page."""

    @wraps(view)
    def wrapper(request, *args, **kwargs):
        if not (request.user.is_authenticated and request.user.is_staff):
            return JsonResponse({"error": "You were logged out. Log in again in another tab, then retry."}, status=403)
        return view(request, *args, **kwargs)

    return wrapper


# ---------- Public pages ----------

def post_list(request):
    posts = Paginator(Post.objects.all(), 20).get_page(request.GET.get("page"))
    return render(request, "blog/post_list.html", {"posts": posts})


def post_detail(request, slug):
    post = get_object_or_404(Post, slug=slug)
    return render(request, "blog/post_detail.html", {"post": post})


# ---------- Editor ----------

def unique_slug(title, exclude_id=None):
    base = slugify(title)[:70] or "post"
    slug, n = base, 2
    others = Post.objects.exclude(id=exclude_id)
    while others.filter(slug=slug).exists():
        slug, n = f"{base}-{n}", n + 1
    return slug


@staff_required
def write(request, post_id=None):
    post = get_object_or_404(Post, id=post_id) if post_id else None
    initial = {
        "id": post.id if post else None,
        "title": post.title if post else "",
        "delta": post.body_delta if post else None,
    }
    return render(request, "blog/write.html", {"post": post, "initial": initial})


@api_staff_required
@require_POST
def save(request):
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


@api_staff_required
@require_POST
def upload_image(request):
    upload = request.FILES.get("file")
    if not upload:
        return JsonResponse({"error": "No file."}, status=400)
    if upload.size > MAX_UPLOAD_BYTES:
        return JsonResponse({"error": "That image is over 15 MB."}, status=400)
    try:
        img = Image.open(upload)
        img.verify()
        upload.seek(0)
        img = Image.open(upload)
    except (UnidentifiedImageError, OSError):
        return JsonResponse({"error": "That file isn't an image I can read."}, status=400)

    name = Path(upload.name).stem[:60] or "image"
    fmt = img.format or "PNG"
    if fmt == "GIF":  # keep animations untouched
        upload.seek(0)
        content, ext = upload.read(), "gif"
    else:
        img = ImageOps.exif_transpose(img)
        img.thumbnail((MAX_IMAGE_SIDE, MAX_IMAGE_SIDE))
        buf = io.BytesIO()
        if fmt == "PNG" or img.mode in ("RGBA", "LA", "P"):
            img.save(buf, "PNG", optimize=True)
            ext = "png"
        else:
            img.convert("RGB").save(buf, "JPEG", quality=85, optimize=True, progressive=True)
            ext = "jpg"
        content = buf.getvalue()

    image = PostImage()
    image.file.save(f"{slugify(name) or 'image'}.{ext}", ContentFile(content), save=True)
    return JsonResponse({"id": image.id, "url": image.file.url})


@api_staff_required
@require_POST
def delete(request, post_id):
    get_object_or_404(Post, id=post_id).delete()
    return JsonResponse({"url": "/blog/"})
