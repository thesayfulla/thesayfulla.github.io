from django.contrib import messages
from django.contrib.auth.decorators import user_passes_test
from django.shortcuts import redirect, render

from blog.models import Post

from .forms import ProfileForm
from .models import Profile

staff_required = user_passes_test(lambda u: u.is_active and u.is_staff)


def home(request):
    profile = Profile.load()
    posts = Post.objects.all()[: profile.latest_posts] if profile.latest_posts else []
    return render(request, "home/home.html", {"latest_posts": posts})


@staff_required
def settings_view(request):
    profile = Profile.load()
    form = ProfileForm(request.POST or None, request.FILES or None, instance=profile)
    if request.method == "POST" and form.is_valid():
        form.save()
        messages.success(request, "Saved.")
        return redirect("settings")
    return render(request, "home/settings.html", {"form": form})
