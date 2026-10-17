from django import forms

from blog.rendering import sanitize

from .models import Profile


class ProfileForm(forms.ModelForm):
    remove_video = forms.BooleanField(required=False)

    class Meta:
        model = Profile
        fields = ["handle", "name", "intro", "hero_video", "x_url", "linkedin_url", "github_url", "email", "latest_posts"]
        widgets = {"intro": forms.HiddenInput}

    def clean_intro(self):
        return sanitize(self.cleaned_data["intro"]).replace("&nbsp;", " ")

    def save(self, commit=True):
        profile = super().save(commit=False)
        if self.cleaned_data.get("remove_video") and not self.files.get("hero_video"):
            profile.hero_video.delete(save=False)
            profile.hero_video = ""
        if commit:
            profile.save()
        return profile
