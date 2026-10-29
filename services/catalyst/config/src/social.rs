//! The social plane's settings and the Pulse room authority.

use clap::Args;

/// Social: communities, the social RPC, comms, notifications, badges, media and profile
/// images.
#[derive(Args, Clone, Debug)]
pub struct SocialConfig {
    /// Bearer of the community admin routes (old API_ADMIN_TOKEN); unset refuses them.
    #[arg(long, env = "COMMUNITIES_ADMIN_TOKEN", hide_env_values = true)]
    pub communities_admin_token: Option<String>,

    /// Comma-separated wallets allowed on GET /v1/moderation/communities.
    #[arg(long, env = "COMMUNITIES_GLOBAL_MODERATORS", default_value = "")]
    pub communities_global_moderators: String,

    /// Comma-separated community names refused on create/rename (folded comparison).
    #[arg(long, env = "RESTRICTED_NAMES", default_value = "")]
    pub restricted_names: String,

    /// Base of community thumbnail URLs (old CDN_URL); default PUBLIC_URL.
    #[arg(long, env = "COMMUNITIES_CDN_URL")]
    pub communities_cdn_url: Option<String>,

    /// Archipelago stats base URL for the onlyOnline member filter; unset renders everyone offline.
    #[arg(long, env = "ARCHIPELAGO_STATS_URL")]
    pub archipelago_stats_url: Option<String>,

    /// Require places added to a community to be owned by the caller per the mirrored places catalogue.
    #[arg(long, env = "COMMUNITIES_PLACES_OWNERSHIP_CHECK", default_value_t = false, action = clap::ArgAction::Set)]
    pub communities_places_ownership_check: bool,

    /// Mirror the public community listing from `COMMUNITIES_UPSTREAM_URL`
    /// (`social::communities::mirror`): read-only copies, no local roles.
    #[arg(long, env = "COMMUNITIES_MIRROR_UPSTREAM", default_value_t = false, action = clap::ArgAction::Set)]
    pub communities_mirror_upstream: bool,

    /// The social API the listing mirrors from (the mirror posture's).
    #[arg(long, env = "COMMUNITIES_UPSTREAM_URL", default_value = "")]
    pub communities_upstream_url: String,

    /// The base the upstream serves community thumbnails under (the mirror
    /// posture's).
    #[arg(long, env = "COMMUNITIES_MIRROR_THUMBNAIL_URL", default_value = "")]
    pub communities_mirror_thumbnail_url: String,

    #[arg(long, env = "COMMUNITIES_MIRROR_INTERVAL_SECS", default_value_t = 3600)]
    pub communities_mirror_interval_secs: u64,

    /// Handshake frame deadline and timestamp window, seconds.
    #[arg(long, env = "AUTH_WINDOW_SECS", default_value_t = 300)]
    pub social_rpc_auth_window_secs: i64,

    /// A private call not accepted within this many ms expires.
    #[arg(
        long,
        env = "PRIVATE_VOICE_CHAT_EXPIRATION_TIME",
        default_value_t = 60_000
    )]
    pub private_voice_chat_expiration_ms: i64,

    /// Tick of the private-call expiry sweep, ms.
    #[arg(long, env = "PRIVATE_VOICE_CHAT_JOB_INTERVAL", default_value_t = 1_000)]
    pub private_voice_chat_job_interval_ms: u64,

    /// Expired private calls reclaimed per sweep statement.
    #[arg(
        long,
        env = "PRIVATE_VOICE_CHAT_EXPIRATION_BATCH_SIZE",
        default_value_t = 20
    )]
    pub private_voice_chat_expiration_batch_size: i64,

    /// Concurrent social sockets; unset = unlimited (1013 past it).
    #[arg(long, env = "WS_MAX_CONCURRENT_CONNECTIONS")]
    pub ws_max_concurrent_connections: Option<usize>,

    /// Largest inbound WebSocket frame, bytes.
    #[arg(long, env = "WS_MAX_PAYLOAD_LENGTH", default_value_t = 1_048_576)]
    pub ws_max_payload_length: usize,

    /// Bearer of the /admin/social routes; unset refuses them.
    #[arg(long, env = "CATALYRST_SOCIAL_RPC_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_social_rpc_admin_token: Option<String>,

    // comms::gatekeeper: the comms gatekeeper (LiveKit token minting, scene and
    // platform moderation, voice presence). The LiveKit media server is a
    // separate process reached over the network.
    /// LiveKit host; signaling and API URLs derive from it.
    #[arg(long, env = "LIVEKIT_HOST", default_value = "livekit.local")]
    pub livekit_host: String,

    /// LiveKit API base when it differs from the host.
    #[arg(long, env = "LIVEKIT_API_HOST")]
    pub livekit_api_host: Option<String>,

    /// Client-facing LiveKit signaling URL when it differs from the host.
    #[arg(long, env = "LIVEKIT_WS_URL")]
    pub livekit_ws_url: Option<String>,

    /// LiveKit API key (or LIVEKIT_API_KEY_FILE); required unless LIVEKIT_ALLOW_DEV_CREDS.
    #[arg(long, env = "LIVEKIT_API_KEY", hide_env_values = true)]
    pub livekit_api_key: Option<String>,

    /// LiveKit API secret (or LIVEKIT_API_SECRET_FILE).
    #[arg(long, env = "LIVEKIT_API_SECRET", hide_env_values = true)]
    pub livekit_api_secret: Option<String>,

    /// Start on the devkey/devsecret placeholders (tokens parse locally, no real SFU accepts them).
    #[arg(long, env = "LIVEKIT_ALLOW_DEV_CREDS", default_value_t = false, action = clap::ArgAction::Set)]
    pub livekit_allow_dev_creds: bool,

    #[arg(
        long,
        env = "PRIVATE_MESSAGES_ROOM_ID",
        default_value = "private-messages"
    )]
    pub private_messages_room_id: String,

    /// The one wallet that may mint the authoritative-server host token; unset refuses /get-server-scene-adapter.
    #[arg(long, env = "AUTHORITATIVE_SERVER_ADDRESS")]
    pub authoritative_server_address: Option<String>,

    /// Bearer of the platform user-moderation routes.
    #[arg(long, env = "MODERATOR_TOKEN", hide_env_values = true)]
    pub moderator_token: Option<String>,

    /// Comma/space-separated wallets allowed on the user-moderation routes by signed fetch.
    #[arg(long, env = "PLATFORM_USER_MODERATORS", default_value = "")]
    pub platform_user_moderators: String,

    /// Service token in front of the voice routes, the DM privacy patch and
    /// the world ban status; unset answers 503 on them.
    #[arg(long, env = "COMMS_GATEKEEPER_AUTH_TOKEN", hide_env_values = true)]
    pub comms_gatekeeper_auth_token: Option<String>,

    /// Lambdas base whose `/parcels/{x}/{y}/operators` protects LAND
    /// owners/operators from scene bans (old LAMBDAS_URL); unset protects none.
    #[arg(long, env = "COMMS_LAND_OPERATORS_URL")]
    pub comms_land_operators_url: Option<String>,

    /// The linker-server lease document (`[{addresses, plots}]`) whose
    /// holders administer the scenes on their plots (the mirror posture's:
    /// Decentraland's published list); empty lists no lease holder.
    #[arg(long, env = "LEASE_AUTHORIZATIONS_URL", default_value = "")]
    pub lease_authorizations_url: String,

    /// This catalyst's federation identity, the epoch author of the MLS
    /// groups created here (normally its public domain); unset uses the id
    /// `social.fed_instance_identity` minted at schema apply.
    #[arg(long, env = "FED_PEER_ID")]
    pub fed_peer_id: Option<String>,

    #[arg(
        long,
        env = "VOICE_CHAT_CONNECTION_INTERRUPTED_TTL",
        default_value_t = 300_000
    )]
    pub voice_chat_connection_interrupted_ttl_ms: i64,

    #[arg(
        long,
        env = "VOICE_CHAT_INITIAL_CONNECTION_TTL",
        default_value_t = 300_000
    )]
    pub voice_chat_initial_connection_ttl_ms: i64,

    #[arg(
        long,
        env = "COMMUNITY_VOICE_CHAT_NO_MODERATOR_TTL",
        default_value_t = 300_000
    )]
    pub community_voice_chat_no_moderator_ttl_ms: i64,

    #[arg(long, env = "VOICE_CHAT_EXPIRED_BATCH_SIZE", default_value_t = 50)]
    pub voice_chat_expired_batch_size: i64,

    /// Bearer of POST /notifications/notifications/broadcast; unset refuses it.
    #[arg(
        long,
        env = "CATALYRST_NOTIFICATIONS_ADMIN_TOKEN",
        hide_env_values = true
    )]
    pub catalyrst_notifications_admin_token: Option<String>,

    #[arg(long, env = "SENDGRID_API_KEY", hide_env_values = true)]
    pub sendgrid_api_key: Option<String>,

    #[arg(long, env = "SENDGRID_FROM_EMAIL")]
    pub sendgrid_from_email: Option<String>,

    #[arg(long, env = "SENDGRID_VALIDATE_EMAIL_TEMPLATE_ID")]
    pub sendgrid_validate_email_template_id: Option<String>,

    #[arg(long, env = "SENDGRID_VALIDATE_CREDITS_EMAIL_TEMPLATE_ID")]
    pub sendgrid_validate_credits_email_template_id: Option<String>,

    /// The email channel of the notification outbox; unset records every delivery as skipped.
    #[arg(long, env = "SENDGRID_NOTIFICATION_TEMPLATE_ID")]
    pub sendgrid_notification_template_id: Option<String>,

    #[arg(
        long,
        env = "ACCOUNT_BASE_URL",
        default_value = "https://account.decentraland.org"
    )]
    pub account_base_url: String,

    #[arg(
        long,
        env = "MARKETPLACE_BASE_URL",
        default_value = "https://decentraland.org/marketplace"
    )]
    pub marketplace_base_url: String,

    /// Cloudflare Turnstile secret for confirm-email; unset turns the check off.
    #[arg(long, env = "TURNSTILE_SECRET_KEY", hide_env_values = true)]
    pub turnstile_secret_key: Option<String>,

    /// Comma-separated email domains refused by set-email.
    #[arg(long, env = "EMAIL_DOMAIN_BLACKLIST", default_value = "")]
    pub email_domain_blacklist: String,

    /// Run the friend_first_wear producer over the synced profiles.
    #[arg(long, env = "FIRST_WEAR_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub first_wear_enabled: bool,

    #[arg(
        long,
        env = "SHOP_ITEM_BASE_URL",
        default_value = "https://decentraland.org/marketplace/"
    )]
    pub shop_item_base_url: String,

    #[arg(long, env = "FIRST_WEAR_IMAGE_BASE")]
    pub first_wear_image_base: Option<String>,

    /// Tick of the notification outbox worker.
    #[arg(
        long,
        env = "NOTIFICATION_DELIVERY_INTERVAL_SECS",
        default_value_t = 15
    )]
    pub notification_delivery_interval_secs: u64,

    // social/badges: the old catalyrst-badges settings (the art is embedded).
    /// Bearer of the badge grant/revoke routes; unset refuses them.
    #[arg(long, env = "CATALYRST_BADGES_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_badges_admin_token: Option<String>,

    #[arg(
        long,
        env = "BADGES_PUBLIC_ASSET_BASE_URL",
        default_value = "https://badges.decentraland.org"
    )]
    pub badges_public_asset_base_url: String,

    /// Files here override the embedded badge art one by one.
    #[arg(long, env = "BADGES_ASSETS_DIR")]
    pub badges_assets_dir: Option<String>,

    // social/media: the autotranslate route (an external engine over the network).
    /// disabled | mock | http | llm; empty = http when TRANSLATE_BACKEND_URL
    /// is set, llm when TRANSLATE_LLM_BASE_URL is, else disabled (503).
    #[arg(long, env = "TRANSLATE_BACKEND", default_value = "")]
    pub translate_backend: String,

    #[arg(long, env = "TRANSLATE_BACKEND_URL", default_value = "")]
    pub translate_backend_url: String,

    #[arg(long, env = "TRANSLATE_BACKEND_API_KEY", hide_env_values = true)]
    pub translate_backend_api_key: Option<String>,

    #[arg(long, env = "TRANSLATE_LLM_BASE_URL", default_value = "")]
    pub translate_llm_base_url: String,

    #[arg(long, env = "TRANSLATE_LLM_API_KEY", hide_env_values = true)]
    pub translate_llm_api_key: Option<String>,

    #[arg(long, env = "TRANSLATE_LLM_MODEL", default_value = "gpt-4o-mini")]
    pub translate_llm_model: String,

    #[arg(long, env = "TRANSLATE_CHAR_LIMIT", default_value_t = 5000)]
    pub translate_char_limit: u64,

    #[arg(long, env = "TRANSLATE_BATCH_LIMIT", default_value_t = 100)]
    pub translate_batch_limit: u64,

    #[arg(long, env = "TRANSLATE_REQUEST_TIMEOUT_SECS", default_value_t = 30)]
    pub translate_request_timeout_secs: u64,

    /// Prune translation_cache rows older than this many days; 0 keeps them.
    #[arg(long, env = "TRANSLATE_CACHE_TTL_DAYS", default_value_t = 0)]
    pub translate_cache_ttl_days: u64,

    /// `/convert` fetches any public URL a client names (the mirror
    /// posture's); off, it converts only this node's own
    /// `<PUBLIC_URL>/content/contents/<hash>` files and fetches nothing.
    #[arg(long, env = "MEDIA_CONVERT_REMOTE_SOURCES", default_value_t = false, action = clap::ArgAction::Set)]
    pub media_convert_remote_sources: bool,

    // social/profile_images: avatar face/body images from the profile
    // entity's own files, the renderer and the upstream service as gated
    // fallbacks. Rendered and fetched images are blobs in the ONE store
    // (social.profile_images); this directory only holds render scratch.
    #[arg(
        long,
        env = "PROFILE_IMAGES_CACHE_DIR",
        default_value = "/var/lib/catalyst/profile-images"
    )]
    pub profile_images_cache_dir: String,

    #[arg(
        long,
        env = "PROFILE_IMAGES_CACHE_TTL_SECONDS",
        default_value_t = 86400
    )]
    pub profile_images_cache_ttl_seconds: u64,

    /// The rendered and fetched images' byte budget in the blob store; 0 = unbounded.
    #[arg(long, env = "PROFILE_IMAGES_CACHE_MAX_BYTES", default_value_t = 0)]
    pub profile_images_cache_max_bytes: u64,

    /// Upstream profile-images service, the explicit last resort; must be a public http(s) host.
    #[arg(long, env = "PROFILE_IMAGES_ORIGIN_URL", default_value = "")]
    pub profile_images_origin_url: String,

    #[arg(long, env = "PROFILE_IMAGES_RENDER_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub profile_images_render_enabled: bool,

    /// `bevy` (the engine's headless `avatar_render`) or `godot`.
    #[arg(long, env = "PROFILE_IMAGES_RENDERER", default_value = "bevy")]
    pub profile_images_renderer: String,

    /// The engine's `avatar_render` (`packages.batch-tools`); required with the bevy renderer.
    #[arg(long, env = "PROFILE_IMAGES_AVATAR_RENDER_BIN", default_value = "")]
    pub profile_images_avatar_render_bin: String,

    #[arg(long, env = "PROFILE_IMAGES_GODOT_BIN", default_value = "")]
    pub profile_images_godot_bin: String,

    #[arg(long, env = "PROFILE_IMAGES_GODOT_PROJECT", default_value = "")]
    pub profile_images_godot_project: String,

    #[arg(
        long,
        env = "PROFILE_IMAGES_RENDERING_METHOD",
        default_value = "gl_compatibility"
    )]
    pub profile_images_rendering_method: String,

    #[arg(
        long,
        env = "PROFILE_IMAGES_RENDERING_DRIVER",
        default_value = "opengl3"
    )]
    pub profile_images_rendering_driver: String,

    #[arg(long, env = "PROFILE_IMAGES_DCLENV", default_value = "")]
    pub profile_images_dclenv: String,

    /// Refused together with the renderer (the avatar scene needs a display).
    #[arg(long, env = "PROFILE_IMAGES_GODOT_HEADLESS", default_value_t = false, action = clap::ArgAction::Set)]
    pub profile_images_godot_headless: bool,

    #[arg(long, env = "PROFILE_IMAGES_GODOT_DISPLAY", default_value = "")]
    pub profile_images_godot_display: String,

    #[arg(long, env = "PROFILE_IMAGES_GODOT_EXTRA_ARGS", default_value = "")]
    pub profile_images_godot_extra_args: String,

    #[arg(
        long,
        env = "PROFILE_IMAGES_RENDER_TIMEOUT_SECONDS",
        default_value_t = 120
    )]
    pub profile_images_render_timeout_seconds: u64,

    #[arg(
        long,
        env = "PROFILE_IMAGES_RENDER_MAX_CONCURRENT",
        default_value_t = 1
    )]
    pub profile_images_render_max_concurrent: u64,

    /// Empty = `<cache dir>/.render-tmp`.
    #[arg(long, env = "PROFILE_IMAGES_RENDER_WORKDIR", default_value = "")]
    pub profile_images_render_workdir: String,
}

/// The Pulse room authority (`comms::gatekeeper`).
#[derive(Args, Clone, Debug)]
pub struct RoomAuthorityConfig {
    // comms::gatekeeper: the Pulse room authority (the application relay's
    // admission check, POST /internal/pulse/room-authority/v1).
    /// 32 base64url bytes without padding (or _FILE), distinct from the
    /// LiveKit secret; unset, the room authority route is not mounted.
    #[arg(long, env = "PULSE_ROOM_AUTHORITY_KEY", hide_env_values = true)]
    pub pulse_room_authority_key: Option<String>,
}
