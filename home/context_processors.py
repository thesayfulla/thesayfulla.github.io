from .models import Profile


def profile(request):
    return {"profile": Profile.load()}
