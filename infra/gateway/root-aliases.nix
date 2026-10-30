[
  {
    key = "= /get-scene-adapter";
    to = "/comms-gatekeeper$uri";
    probe = [
      "POST"
      "/get-scene-adapter"
      "/comms-gatekeeper/get-scene-adapter"
    ];
  }
  {
    key = "= /get-server-scene-adapter";
    to = "/comms-gatekeeper$uri";
    probe = [
      "POST"
      "/get-server-scene-adapter"
      "/comms-gatekeeper/get-server-scene-adapter"
    ];
  }
  {
    key = "= /scene-participants";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/scene-participants"
      "/comms-gatekeeper/scene-participants"
    ];
  }
  {
    key = "= /scene-bans";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/scene-bans"
      "/comms-gatekeeper/scene-bans"
    ];
  }
  {
    key = "^~ /scene-bans/";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/scene-bans/addresses"
      "/comms-gatekeeper/scene-bans/addresses"
    ];
  }
  {
    key = "= /bans";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/bans"
      "/comms-gatekeeper/bans"
    ];
  }
  {
    key = "= /community-voice-chat";
    to = "/comms-gatekeeper$uri";
    probe = [
      "POST"
      "/community-voice-chat"
      "/comms-gatekeeper/community-voice-chat"
    ];
  }
  {
    key = "^~ /community-voice-chat/";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/community-voice-chat/active"
      "/comms-gatekeeper/community-voice-chat/active"
    ];
  }
  {
    key = "= /private-messages/token";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/private-messages/token"
      "/comms-gatekeeper/private-messages/token"
    ];
  }
  {
    key = "^~ /cast/";
    to = "/comms-gatekeeper$uri";
    probe = [
      "GET"
      "/cast/presenters"
      "/comms-gatekeeper/cast/presenters"
    ];
  }
  {
    key = "= /island-refresh";
    to = "/comms-gatekeeper$uri";
    probe = [
      "POST"
      "/island-refresh"
      "/comms-gatekeeper/island-refresh"
    ];
  }
  {
    key = "= /livekit-webhook";
    to = "/comms-gatekeeper$uri";
    probe = [
      "POST"
      "/livekit-webhook"
      "/comms-gatekeeper/livekit-webhook"
    ];
  }
  {
    key = "= /notifications";
    to = "/notifications$uri";
    probe = [
      "GET"
      "/notifications"
      "/notifications/notifications"
    ];
  }
  {
    key = "= /notifications/read";
    to = "/notifications$uri";
    probe = [
      "PUT"
      "/notifications/read"
      "/notifications/notifications/read"
    ];
  }
  {
    key = "= /notifications/broadcast";
    to = "/notifications$uri";
    probe = [
      "POST"
      "/notifications/broadcast"
      "/notifications/notifications/broadcast"
    ];
  }
  {
    key = "= /subscription";
    to = "/notifications$uri";
    probe = [
      "GET"
      "/subscription"
      "/notifications/subscription"
    ];
  }
  {
    key = "^~ /subscription/";
    to = "/notifications$uri";
    probe = [
      "POST"
      "/subscription/opt-outs"
      "/notifications/subscription/opt-outs"
    ];
  }
  {
    key = "= /set-email";
    to = "/notifications$uri";
    probe = [
      "PUT"
      "/set-email"
      "/notifications/set-email"
    ];
  }
  {
    key = "= /confirm-email";
    to = "/notifications$uri";
    probe = [
      "PUT"
      "/confirm-email"
      "/notifications/confirm-email"
    ];
  }
  {
    key = "~ \"^/users/0x[0-9a-fA-F]{40}/(badges|preview)$\"";
    to = "/badges$uri";
    probe = [
      "GET"
      "/users/0x1234567890abcdef1234567890abcdef12345678/badges"
      "/badges/users/0x1234567890abcdef1234567890abcdef12345678/badges"
    ];
  }
  {
    key = "~ ^/metamorph-api(/(translate|convert|media/convert))$";
    rewrite = "^/metamorph-api(/.*)$ $1";
    probe = [
      "POST"
      "/metamorph-api/translate"
      "/translate"
    ];
  }
  {
    key = "~ ^/assets-cdn(/social/communities/[^/]+/raw-thumbnail\\.png)$";
    rewrite = "^/assets-cdn(/.*)$ $1";
    probe = [
      "GET"
      "/assets-cdn/social/communities/00000000-0000-0000-0000-000000000000/raw-thumbnail.png"
      "/social/communities/00000000-0000-0000-0000-000000000000/raw-thumbnail.png"
    ];
  }
  {
    key = "^~ /v1/map.png";
    to = "/api$uri";
    probe = [
      "GET"
      "/v1/map.png"
      "/api/v1/map.png"
    ];
  }
  {
    key = "^~ /v1/minimap.png";
    to = "/api$uri";
    probe = [
      "GET"
      "/v1/minimap.png"
      "/api/v1/minimap.png"
    ];
  }
  {
    key = "^~ /v1/estatemap.png";
    to = "/api$uri";
    probe = [
      "GET"
      "/v1/estatemap.png"
      "/api/v1/estatemap.png"
    ];
  }
  {
    key = "^~ /v1/tiles";
    to = "/api$uri";
    probe = [
      "GET"
      "/v1/tiles"
      "/api/v1/tiles"
    ];
  }
  {
    key = "~ ^/v2/(tiles|map\\.png|parcels|estates|districts|addresses|contracts|ping|ready)(/|$)";
    to = "/api$uri";
    probe = [
      "GET"
      "/v2/tiles/info"
      "/api/v2/tiles/info"
    ];
  }
  {
    key = "^~ /categories";
    to = "/api$uri";
    probe = [
      "GET"
      "/categories"
      "/api/categories"
    ];
  }
  {
    key = "^~ /world_names";
    to = "/api$uri";
    probe = [
      "GET"
      "/world_names"
      "/api/world_names"
    ];
  }
  {
    key = "^~ /market/";
    rewrite = "^/market/(.*)$ /marketplace-api/$1";
    probe = [
      "GET"
      "/market/v1/catalog"
      "/marketplace-api/v1/catalog"
    ];
  }
  {
    key = "^~ /v1/nfts";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/nfts"
      "/marketplace-api/v1/nfts"
    ];
  }
  {
    key = "^~ /v1/orders";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/orders"
      "/marketplace-api/v1/orders"
    ];
  }
  {
    key = "^~ /v1/bids";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/bids"
      "/marketplace-api/v1/bids"
    ];
  }
  {
    key = "^~ /v1/sales";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/sales"
      "/marketplace-api/v1/sales"
    ];
  }
  {
    key = "^~ /v1/accounts";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/accounts"
      "/marketplace-api/v1/accounts"
    ];
  }
  {
    key = "= /v1/contracts";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/contracts"
      "/marketplace-api/v1/contracts"
    ];
  }
  {
    key = "^~ /v1/owners";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/owners"
      "/marketplace-api/v1/owners"
    ];
  }
  {
    key = "^~ /v1/prices";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/prices"
      "/marketplace-api/v1/prices"
    ];
  }
  {
    key = "^~ /v1/trendings";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/trendings"
      "/marketplace-api/v1/trendings"
    ];
  }
  {
    key = "^~ /v1/volume";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/volume/week"
      "/marketplace-api/v1/volume/week"
    ];
  }
  {
    key = "^~ /v1/catalog";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/catalog"
      "/marketplace-api/v1/catalog"
    ];
  }
  {
    key = "^~ /v1/activity";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/activity"
      "/marketplace-api/v1/activity"
    ];
  }
  {
    key = "^~ /v1/items";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/items"
      "/marketplace-api/v1/items"
    ];
  }
  {
    key = "~ ^/v1/trades/[^/]+/accept$";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v1/trades/0xabc/accept"
      "/marketplace-api/v1/trades/0xabc/accept"
    ];
  }
  {
    key = "^~ /v2/catalog";
    to = "/marketplace-api$uri";
    probe = [
      "GET"
      "/v2/catalog"
      "/marketplace-api/v2/catalog"
    ];
  }
  {
    key = "^~ /seasons";
    to = "/credits$uri";
    probe = [
      "GET"
      "/seasons"
      "/credits/seasons"
    ];
  }
  {
    key = "= /captcha";
    to = "/credits$uri";
    probe = [
      "GET"
      "/captcha"
      "/credits/captcha"
    ];
  }
  {
    key = "~ \"^/users/0x[0-9a-fA-F]{40}/(credits|progress)$\"";
    to = "/credits$uri";
    probe = [
      "GET"
      "/users/0x1234567890abcdef1234567890abcdef12345678/credits"
      "/credits/users/0x1234567890abcdef1234567890abcdef12345678/credits"
    ];
  }
  {
    key = "^~ /wallet/";
    to = "/credits$uri";
    probe = [
      "GET"
      "/wallet/0x1234567890abcdef1234567890abcdef12345678/balance"
      "/credits/wallet/0x1234567890abcdef1234567890abcdef12345678/balance"
    ];
  }
  {
    key = "~ ^/(mainnet|sepolia|polygon|amoy|ethereum)$";
    to = "/rpc$uri";
    probe = [
      "POST"
      "/mainnet"
      "/rpc/mainnet"
    ];
  }
  {
    key = "^~ /LOD/";
    to = "/ab-cdn$uri";
    probe = [
      "GET"
      "/LOD/0/x.glb"
      "/ab-cdn/LOD/0/x.glb"
    ];
  }
  {
    key = "^~ /manifest/";
    to = "/ab-cdn$uri";
    probe = [
      "GET"
      "/manifest/x_windows.json"
      "/ab-cdn/manifest/x_windows.json"
    ];
  }
  {
    key = "^~ /camera-reel/";
    rewrite = "^/camera-reel/(.*)$ /camera-reel-service/$1";
    probe = [
      "GET"
      "/camera-reel/api/users/0x1234567890abcdef1234567890abcdef12345678"
      "/camera-reel-service/api/users/0x1234567890abcdef1234567890abcdef12345678"
    ];
  }
  {
    key = "^~ /builder-items/";
    rewrite = "^/builder-items/(.*)$ /builder-api/contents/$1";
    probe = [
      "GET"
      "/builder-items/QmNotStoredAnywhere"
      "/builder-api/contents/QmNotStoredAnywhere"
    ];
  }
  {
    key = "= /registry";
    to = "/asset-bundle-registry$uri";
    probe = [
      "GET"
      "/registry"
      "/asset-bundle-registry/registry"
    ];
  }
  {
    key = "^~ /denylist";
    to = "/asset-bundle-registry$uri";
    probe = [
      "GET"
      "/denylist"
      "/asset-bundle-registry/denylist"
    ];
  }
  {
    key = "^~ /queues/";
    to = "/asset-bundle-registry$uri";
    probe = [
      "GET"
      "/queues/status"
      "/asset-bundle-registry/queues/status"
    ];
  }
  {
    key = "= /flush-cache";
    to = "/asset-bundle-registry$uri";
    probe = [
      "POST"
      "/flush-cache"
      "/asset-bundle-registry/flush-cache"
    ];
  }
  {
    key = "= /main";
    to = "/realm-provider-ea$uri";
    probe = [
      "GET"
      "/main"
      "/realm-provider-ea/main"
    ];
  }
  {
    key = "= /realms";
    to = "/realm-provider-ea$uri";
    probe = [
      "GET"
      "/realms"
      "/realm-provider-ea/realms"
    ];
  }
]
