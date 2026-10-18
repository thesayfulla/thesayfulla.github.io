from django.contrib.auth.mixins import UserPassesTestMixin
from django.http import JsonResponse


class StaffRequiredMixin(UserPassesTestMixin):
    """Only you (a staff user) may see the page; everyone else goes to the login page."""

    def test_func(self):
        user = self.request.user
        return user.is_active and user.is_staff


class StaffRequiredJSONMixin(StaffRequiredMixin):
    """Same check for the editor's background requests, answered with JSON instead of a redirect."""

    def handle_no_permission(self):
        return JsonResponse({"error": "You were logged out. Log in again in another tab, then retry."}, status=403)
