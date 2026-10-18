from django.urls import path

from . import views

app_name = "blog"

urlpatterns = [
    path("blog/", views.PostListView.as_view(), name="list"),
    path("blog/page/<int:page>/", views.PostListView.as_view(), name="list-page"),
    path("blog/<slug:slug>/", views.PostDetailView.as_view(), name="post"),
    path("write/", views.EditorView.as_view(), name="write"),
    path("write/<int:post_id>/", views.EditorView.as_view(), name="edit"),
    path("write/save/", views.PostSaveView.as_view(), name="save"),
    path("write/upload/", views.ImageUploadView.as_view(), name="upload"),
    path("write/<int:post_id>/delete/", views.PostDeleteView.as_view(), name="delete"),
]
