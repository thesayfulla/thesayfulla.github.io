from django.conf import settings
from django.contrib.auth import views as auth_views
from django.urls import include, path

from home import views as home_views

urlpatterns = [
    path("", home_views.home, name="home"),
    path("settings/", home_views.settings_view, name="settings"),
    path("login/", auth_views.LoginView.as_view(template_name="login.html", redirect_authenticated_user=True), name="login"),
    path("logout/", auth_views.LogoutView.as_view(next_page="/"), name="logout"),
    path("", include("blog.urls")),
]

if settings.DEBUG:
    from django.conf.urls.static import static

    # Serve uploaded files from the development server
    urlpatterns += static(settings.MEDIA_URL, document_root=settings.MEDIA_ROOT)
