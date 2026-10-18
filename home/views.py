from django.contrib.messages.views import SuccessMessageMixin
from django.urls import reverse_lazy
from django.views.generic import TemplateView, UpdateView

from blog.models import Post
from portfolio.mixins import StaffRequiredMixin

from . import cv
from .forms import ProfileForm
from .models import Profile


class HomeView(TemplateView):
    template_name = "home/home.html"

    def get_context_data(self, **kwargs):
        context = super().get_context_data(**kwargs)
        count = Profile.load().latest_posts
        context["latest_posts"] = Post.objects.all()[:count] if count else []
        return context


class CVView(TemplateView):
    template_name = "home/cv.html"
    extra_context = {"cv": cv}


class SettingsView(StaffRequiredMixin, SuccessMessageMixin, UpdateView):
    form_class = ProfileForm
    template_name = "home/settings.html"
    success_url = reverse_lazy("settings")
    success_message = "Saved."

    def get_object(self, queryset=None):
        return Profile.load()

    def get_context_object_name(self, obj):
        # Don't shadow the site-wide `profile` (from the context processor) with
        # the half-edited copy the form holds when a save fails validation.
        return None
