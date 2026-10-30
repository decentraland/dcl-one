{
  deploy = [
    "/entities"
    "/content/entities"
  ];
  internal = [
    "/internal/pulse"
    "/metrics"
  ];
  node = [
    {
      path = "/admin/api";
      probe = "/admin/api/jobs";
    }
    {
      path = "/admin/auth";
      probe = "/admin/auth/nonce";
    }
    {
      path = "/admin/social";
      probe = "/admin/social/presence";
    }
    {
      path = "/admin/federation";
      probe = "/admin/federation/worlds/refresh";
    }
    {
      path = "/admin/worlds";
      probe = "/admin/worlds";
    }
    {
      path = "/admin/blocked";
      probe = "/admin/blocked";
    }
    {
      path = "/admin/access-log";
      probe = "/admin/access-log";
    }
    {
      path = "/admin/forwarding";
      probe = "/admin/forwarding";
    }
    {
      path = "/auth/challenge";
      probe = "/auth/challenge";
    }
    {
      path = "/auth/livekit-token";
      probe = "/auth/livekit-token";
    }
    {
      path = "/places/api";
      probe = "/places/api/status";
    }
    {
      path = "/places/v1";
      probe = "/places/v1/categories";
    }
    {
      path = "/places/place";
      probe = "/places/place";
    }
    {
      path = "/places/world";
      probe = "/places/world";
    }
    {
      path = "/events/api";
      probe = "/events/api/status";
    }
    {
      path = "/events/v1";
      probe = "/events/v1/events";
    }
    {
      path = "/events/sitemap.xml";
      probe = "/events/sitemap.xml";
    }
    {
      path = "/events/sitemap.static.xml";
      probe = "/events/sitemap.static.xml";
    }
    {
      path = "/events/sitemap.events.xml";
      probe = "/events/sitemap.events.xml";
    }
    {
      path = "/events/sitemap.schedules.xml";
      probe = "/events/sitemap.schedules.xml";
    }
  ];
}
