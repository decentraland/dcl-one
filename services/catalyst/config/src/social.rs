//! The social plane's settings and the Pulse room authority.

use clap::Args;

// @well-known-components/env-config-provider getNumber uses parseFloat,
// including numeric prefixes and Infinity, and rejects NaN.
fn source_number(raw: &str) -> Result<f64, String> {
    let raw =
        raw.trim_start_matches(|c: char| c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace()));
    let bytes = raw.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    if raw
        .get(end..)
        .is_some_and(|suffix| suffix.starts_with("Infinity"))
    {
        return Ok(if bytes.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let mut digits = 0;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
        digits += 1;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return Err("Value is not a number".into());
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let exponent = end;
        end += 1;
        if matches!(bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }
        let start = end;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
        if end == start {
            end = exponent;
        }
    }
    raw[..end]
        .parse::<f64>()
        .map_err(|_| "Value is not a number".into())
}

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

    /// Feature service base; `dapps.json` is fetched with SERVICE_BASE_URL as Referer.
    #[arg(long, env = "FF_URL", default_value = "")]
    pub social_feature_flags_url: String,

    #[arg(
        long,
        env = "SERVICE_BASE_URL",
        default_value = "https://social-service-ea.decentraland.zone"
    )]
    pub social_service_base_url: String,

    #[arg(
        long,
        env = "FEATURE_FLAG_REFRESH_INTERVAL_IN_MS",
        default_value_t = 240_000
    )]
    pub social_feature_flags_refresh_ms: u64,

    #[arg(long, env = "FF_REQUEST_TIMEOUT", default_value_t = 10_000)]
    pub social_feature_flags_timeout_ms: u64,

    #[arg(long, env = "FF_DAPPS_COMMUNITIES_AI_COMPLIANCE")]
    pub social_ai_compliance_override: Option<String>,

    #[arg(long, env = "FF_DAPPS_DEV_COMMUNITIES_AI_COMPLIANCE")]
    pub social_dev_ai_compliance_override: Option<String>,

    #[arg(long, env = "FF_DAPPS_COMMUNITIES_GLOBAL_MODERATORS")]
    pub social_global_moderators_override: Option<String>,

    #[arg(long, env = "FF_DAPPS_REFERRAL_REGISTRATION_DISABLED")]
    pub social_referral_registration_disabled_override: Option<String>,

    /// Source environment: AI compliance runs for `prd` or its DAPPS dev flag.
    #[arg(long, env = "ENV", default_value = "")]
    pub social_environment: String,

    #[arg(long, env = "OPEN_AI_MODEL", default_value = "gpt-5-nano")]
    pub community_compliance_model: String,

    /// Provider credential, also readable from OPEN_AI_API_KEY_FILE.
    #[arg(long, env = "OPEN_AI_API_KEY", hide_env_values = true)]
    pub community_compliance_api_key: Option<String>,

    #[arg(
        long,
        env = "OPENAI_BASE_URL",
        default_value = "https://api.openai.com/v1"
    )]
    pub community_compliance_base_url: String,

    #[arg(long, env = "OPENAI_ORG_ID")]
    pub community_compliance_organization: Option<String>,

    #[arg(long, env = "OPENAI_PROJECT_ID")]
    pub community_compliance_project: Option<String>,

    /// Comma-separated community names refused on create/rename (folded comparison).
    #[arg(long, env = "RESTRICTED_NAMES", default_value = "")]
    pub restricted_names: String,

    /// Base of community thumbnail URLs (old CDN_URL); default PUBLIC_URL.
    #[arg(long, env = "COMMUNITIES_CDN_URL")]
    pub communities_cdn_url: Option<String>,

    /// Archipelago stats base URL for the onlyOnline member filter; unset renders everyone offline.
    #[arg(long, env = "ARCHIPELAGO_STATS_URL")]
    pub archipelago_stats_url: Option<String>,

    /// Require places added to a community to be owned by the caller per the mirrored places catalogue
    /// (on by default, as upstream always checks; `false` skips the check).
    #[arg(long, env = "COMMUNITIES_PLACES_OWNERSHIP_CHECK", default_value_t = true, action = clap::ArgAction::Set)]
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

    /// Friendship/block fixed-window limits, shared by every serving replica.
    #[arg(
        long,
        env = "FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS",
        default_value_t = 60.0,
        value_parser = source_number,
        allow_hyphen_values = true
    )]
    pub friendship_rate_limit_window_seconds: f64,
    #[arg(
        long,
        env = "FRIENDSHIP_RATE_LIMIT_PER_ACTOR",
        default_value_t = 30.0,
        value_parser = source_number,
        allow_hyphen_values = true
    )]
    pub friendship_rate_limit_per_actor: f64,
    #[arg(
        long,
        env = "FRIENDSHIP_RATE_LIMIT_PER_PAIR",
        default_value_t = 10.0,
        value_parser = source_number,
        allow_hyphen_values = true
    )]
    pub friendship_rate_limit_per_pair: f64,

    #[arg(long, env = "AWS_SQS_ENDPOINT")]
    pub legacy_sqs_endpoint: Option<String>,
    #[arg(long, env = "AWS_SQS_QUEUE_URL")]
    pub legacy_sqs_queue_url: Option<String>,
    #[arg(long, env = "AWS_SNS_ENDPOINT")]
    pub legacy_sns_endpoint: Option<String>,
    #[arg(long, env = "AWS_SNS_ARN")]
    pub legacy_sns_arn: Option<String>,
    #[arg(long, env = "REFERRAL_MAX_IP_MATCHES", default_value_t = 2)]
    pub referral_max_ip_matches: i64,
    #[arg(long, env = "REFERRAL_MIN_LOGIN_DAYS", default_value_t = 3)]
    pub referral_min_login_days: i64,
    #[arg(long, env = "REWARD_SERVER_URL")]
    pub referral_reward_server_url: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_5",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_5: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_10",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_10: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_20",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_20: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_25",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_25: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_30",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_30: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_50",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_50: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_60",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_60: Option<String>,
    #[arg(
        long,
        env = "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_75",
        hide_env_values = true
    )]
    pub rewards_api_key_by_referral_invited_users_75: Option<String>,

    #[arg(long, env = "PROFILE_URL", default_value = "https://decentraland.org")]
    pub referral_profile_url: String,

    /// Handshake frame deadline and timestamp window, seconds.
    #[arg(long, env = "AUTH_WINDOW_SECS", default_value_t = 300)]
    pub social_rpc_auth_window_secs: i64,

    /// First-frame authentication deadline; independent of signature age.
    #[arg(long, env = "WS_AUTH_TIMEOUT_IN_MS")]
    pub ws_auth_timeout_in_ms: Option<u64>,
    /// Maximum absolute signature age in milliseconds.
    #[arg(long, env = "WS_AUTH_SIGNATURE_EXPIRATION_IN_MS")]
    pub ws_auth_signature_expiration_in_ms: Option<i64>,
    /// Seconds without socket traffic; zero disables idle expiry.
    #[arg(long, env = "WS_IDLE_TIMEOUT_IN_SECONDS", default_value_t = 300)]
    pub ws_idle_timeout_in_seconds: u64,

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

    /// Tick of the shared community voice notification status poll, milliseconds.
    #[arg(
        long,
        env = "COMMUNITY_VOICE_CHAT_POLLING_JOB_INTERVAL",
        default_value_t = 45_000
    )]
    pub community_voice_chat_polling_job_interval_ms: u64,

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

    /// Bytes a /social-rpc/v1/stream socket may have waiting to be written
    /// when another frame is sent; past it the socket closes 1008 `Slow consumer`.
    #[arg(long, env = "WS_MAX_BACKPRESSURE", default_value_t = 131_072)]
    pub ws_max_backpressure: usize,

    /// Authenticated social sockets per address across both protocols; past
    /// it the address's oldest closes 3005.
    #[arg(long, env = "WS_MAX_CONNECTIONS_PER_ADDRESS", default_value_t = 8, value_parser = clap::value_parser!(u32).range(1..))]
    pub ws_max_connections_per_address: u32,

    /// Stream subscriptions an address may start per second, refilling its bucket.
    #[arg(long, env = "WS_SUBSCRIPTIONS_PER_SECOND", default_value_t = 2.0, value_parser = positive)]
    pub ws_subscriptions_per_second: f64,

    /// The size of an address's subscription bucket.
    #[arg(long, env = "WS_SUBSCRIPTION_BURST", default_value_t = 24, value_parser = clap::value_parser!(u32).range(1..))]
    pub ws_subscription_burst: u32,

    /// Disconnected addresses whose partly spent subscription bucket is remembered.
    #[arg(long, env = "WS_ADDRESS_BUDGET_CAPACITY", default_value_t = 10_000, value_parser = clap::value_parser!(u32).range(1..))]
    pub ws_address_budget_capacity: u32,

    /// Largest inbound /social-rpc/v1/stream frame, bytes.
    #[arg(long, env = "STREAM_WS_MAX_PAYLOAD_LENGTH", default_value_t = 16_384, value_parser = clap::value_parser!(u32).range(1..))]
    pub stream_ws_max_payload_length: u32,

    /// Frames a /social-rpc/v1/stream socket may send per second, refilling its bucket.
    #[arg(long, env = "STREAM_WS_MESSAGES_PER_SECOND", default_value_t = 20.0, value_parser = positive)]
    pub stream_ws_messages_per_second: f64,

    /// The size of a /social-rpc/v1/stream socket's frame bucket.
    #[arg(long, env = "STREAM_WS_MESSAGE_BURST", default_value_t = 40, value_parser = clap::value_parser!(u32).range(1..))]
    pub stream_ws_message_burst: u32,

    /// Bearer of the /admin/social routes; unset refuses them.
    #[arg(long, env = "CATALYRST_SOCIAL_RPC_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_social_rpc_admin_token: Option<String>,

    /// Source CURRENT_VERSION string returned verbatim by social RPC /status.
    #[arg(long, env = "CURRENT_VERSION")]
    pub social_rpc_current_version: Option<String>,

    /// Legacy uWS metrics path; unset/empty uses /metrics under /social-rpc.
    #[arg(long, env = "WKC_METRICS_PUBLIC_PATH")]
    pub wkc_metrics_public_path: Option<String>,

    /// Legacy metrics Bearer; unset/empty permits scrapes.
    #[arg(long, env = "WKC_METRICS_BEARER_TOKEN", hide_env_values = true)]
    pub wkc_metrics_bearer_token: Option<String>,

    /// Source opt-in reset flag. Shared-recorder reset parity remains a known gap.
    #[arg(long, env = "WKC_METRICS_RESET_AT_NIGHT")]
    pub wkc_metrics_reset_at_night: Option<String>,

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

/// A finite number above zero (upstream refuses to start otherwise).
fn positive(raw: &str) -> Result<f64, String> {
    match raw.parse::<f64>() {
        Ok(value) if value.is_finite() && value > 0.0 => Ok(value),
        _ => Err(format!("`{raw}` is not a positive number")),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    const BASE: [(&str, &str); 1] = [("DATABASE_URL", "postgres:///catalyst")];

    fn social(pairs: &[(&'static str, &'static str)]) -> Result<super::SocialConfig, clap::Error> {
        Config::parse_env_for_tests(BASE.iter().chain(pairs).copied()).map(|c| c.social)
    }

    #[test]
    fn legacy_uws_status_and_metrics_keep_raw_source_strings() {
        let config = social(&[
            ("CURRENT_VERSION", ""),
            ("WKC_METRICS_PUBLIC_PATH", ""),
            ("WKC_METRICS_BEARER_TOKEN", "  "),
            ("WKC_METRICS_RESET_AT_NIGHT", "TRUE"),
        ])
        .unwrap();
        assert_eq!(config.social_rpc_current_version.as_deref(), Some(""));
        assert_eq!(config.wkc_metrics_public_path.as_deref(), Some(""));
        assert_eq!(config.wkc_metrics_bearer_token.as_deref(), Some("  "));
        assert_eq!(config.wkc_metrics_reset_at_night.as_deref(), Some("TRUE"));
        let config = social(&[
            ("CURRENT_VERSION", "source-version"),
            ("WKC_METRICS_PUBLIC_PATH", "/custom"),
            ("WKC_METRICS_RESET_AT_NIGHT", "true"),
        ])
        .unwrap();
        assert_eq!(
            config.social_rpc_current_version.as_deref(),
            Some("source-version")
        );
        assert_eq!(config.wkc_metrics_public_path.as_deref(), Some("/custom"));
        assert_eq!(config.wkc_metrics_reset_at_night.as_deref(), Some("true"));
    }

    #[test]
    fn friendship_limits_read_source_variables_including_zero_and_negative_values() {
        let defaults = social(&[]).unwrap();
        assert_eq!(
            (
                defaults.friendship_rate_limit_window_seconds,
                defaults.friendship_rate_limit_per_actor,
                defaults.friendship_rate_limit_per_pair
            ),
            (60.0, 30.0, 10.0)
        );
        let configured = social(&[
            ("FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS", "-1"),
            ("FRIENDSHIP_RATE_LIMIT_PER_ACTOR", "0"),
            ("FRIENDSHIP_RATE_LIMIT_PER_PAIR", "2.5"),
        ])
        .unwrap();
        assert_eq!(
            (
                configured.friendship_rate_limit_window_seconds,
                configured.friendship_rate_limit_per_actor,
                configured.friendship_rate_limit_per_pair
            ),
            (-1.0, 0.0, 2.5)
        );
    }

    #[test]
    fn friendship_numbers_use_source_parse_float_instead_of_strict_rust_parsing() {
        for (raw, expected) in [
            ("10abc", 10.0),
            ("  +.5tail", 0.5),
            ("1e+2suffix", 100.0),
            ("1e-", 1.0),
            ("0x10", 0.0),
            ("Infinitysuffix", f64::INFINITY),
            ("-Infinity", f64::NEG_INFINITY),
            ("1e999", f64::INFINITY),
        ] {
            let configured = social(&[("FRIENDSHIP_RATE_LIMIT_PER_PAIR", raw)]).unwrap();
            assert_eq!(configured.friendship_rate_limit_per_pair, expected, "{raw}");
        }
        for raw in ["NaN", "invalid", "", "infinity"] {
            assert!(
                social(&[("FRIENDSHIP_RATE_LIMIT_PER_PAIR", raw)]).is_err(),
                "{raw}"
            );
        }
    }

    #[test]
    fn socket_deadline_signature_age_and_idle_have_independent_source_variables() {
        let defaults = social(&[]).unwrap();
        assert_eq!(defaults.ws_idle_timeout_in_seconds, 300);
        assert_eq!(defaults.ws_auth_timeout_in_ms, None);
        let config = social(&[
            ("WS_AUTH_TIMEOUT_IN_MS", "7"),
            ("WS_AUTH_SIGNATURE_EXPIRATION_IN_MS", "23"),
            ("WS_IDLE_TIMEOUT_IN_SECONDS", "0"),
        ])
        .unwrap();
        assert_eq!(config.ws_auth_timeout_in_ms, Some(7));
        assert_eq!(config.ws_auth_signature_expiration_in_ms, Some(23));
        assert_eq!(config.ws_idle_timeout_in_seconds, 0);
    }

    /// The social sockets' limits are upstream social-service-ea's variables
    /// with its defaults (`src/logic/ws-admission/component.ts:19-22`,
    /// `stream-ws-handler.ts:28`, `stream-sessions/component.ts:68-69`,
    /// `ws-handler.ts:272`), and a value upstream refuses to start with is refused.
    #[test]
    fn socket_limits_read_upstreams_variables() {
        let defaults = social(&[]).unwrap();
        assert_eq!(defaults.ws_max_backpressure, 128 * 1024);
        assert_eq!(defaults.ws_max_connections_per_address, 8);
        assert_eq!(defaults.ws_subscriptions_per_second, 2.0);
        assert_eq!(defaults.ws_subscription_burst, 24);
        assert_eq!(defaults.ws_address_budget_capacity, 10_000);
        assert_eq!(defaults.stream_ws_max_payload_length, 16 * 1024);
        assert_eq!(defaults.stream_ws_messages_per_second, 20.0);
        assert_eq!(defaults.stream_ws_message_burst, 40);

        let set = social(&[
            ("WS_MAX_BACKPRESSURE", "1024"),
            ("WS_MAX_CONNECTIONS_PER_ADDRESS", "2"),
            ("WS_SUBSCRIPTIONS_PER_SECOND", "0.5"),
            ("WS_SUBSCRIPTION_BURST", "3"),
            ("WS_ADDRESS_BUDGET_CAPACITY", "4"),
            ("STREAM_WS_MAX_PAYLOAD_LENGTH", "5"),
            ("STREAM_WS_MESSAGES_PER_SECOND", "6.5"),
            ("STREAM_WS_MESSAGE_BURST", "7"),
        ])
        .unwrap();
        assert_eq!(
            (
                set.ws_max_backpressure,
                set.ws_max_connections_per_address,
                set.ws_subscriptions_per_second,
                set.ws_subscription_burst,
                set.ws_address_budget_capacity,
                set.stream_ws_max_payload_length,
                set.stream_ws_messages_per_second,
                set.stream_ws_message_burst,
            ),
            (1024, 2, 0.5, 3, 4, 5, 6.5, 7)
        );
        for refused in [
            ("WS_MAX_CONNECTIONS_PER_ADDRESS", "0"),
            ("WS_SUBSCRIPTIONS_PER_SECOND", "0"),
            ("WS_SUBSCRIPTION_BURST", "0"),
            ("WS_ADDRESS_BUDGET_CAPACITY", "0"),
            ("STREAM_WS_MAX_PAYLOAD_LENGTH", "0"),
            ("STREAM_WS_MESSAGES_PER_SECOND", "-1"),
            ("STREAM_WS_MESSAGES_PER_SECOND", "inf"),
            ("STREAM_WS_MESSAGE_BURST", "0"),
        ] {
            assert!(social(&[refused]).is_err(), "{refused:?}");
        }
    }
}
