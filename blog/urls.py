from django.urls import path

from . import views

app_name = "blog"

urlpatterns = [
    path("blog/", views.post_list, name="list"),
    path("blog/<slug:slug>/", views.post_detail, name="post"),
    path("write/", views.write, name="write"),
    path("write/<int:post_id>/", views.write, name="edit"),
    path("write/save/", views.save, name="save"),
    path("write/upload/", views.upload_image, name="upload"),
    path("write/<int:post_id>/delete/", views.delete, name="delete"),
]
