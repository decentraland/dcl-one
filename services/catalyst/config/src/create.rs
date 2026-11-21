//! The creator plane's settings (builder and camera_reel).

use clap::Args;

/// The creator plane: builder and camera_reel.
#[derive(Args, Clone, Debug)]
pub struct CreateConfig {
    /// Opens the creator plane's writes; off, each builder and camera reel
    /// write answers 503 with `Retry-After`.
    #[arg(long, env = "CREATE_WRITES_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub create_writes_enabled: bool,

    /// Polygon JSON-RPC (https to a public host) that verifies publication
    /// receipts and reads the rarities' MANA prices; unset, publication
    /// begin/claim/verify and the rarities routes answer 503.
    #[arg(long, env = "BUILDER_POLYGON_RPC_URL", hide_env_values = true)]
    pub builder_polygon_rpc_url: Option<String>,

    /// Comma-separated wallets that may use the builder's curation console
    /// (`/v1/collections/curation`); they hold no role on any other builder route.
    #[arg(long, env = "BUILDER_ADMIN_ADDRESSES", default_value = "")]
    pub builder_admin_addresses: String,

    /// Bearer of the builder's curation console; unset fails closed.
    #[arg(long, env = "CATALYRST_BUILDER_ADMIN_TOKEN", hide_env_values = true)]
    pub catalyrst_builder_admin_token: Option<String>,

    /// The newsletter service's API base (public http(s)) a subscription is
    /// forwarded to; the forward needs all three newsletter settings, and
    /// without them a subscription is only stored.
    #[arg(long, env = "NEWSLETTER_SERVICE_URL")]
    pub newsletter_service_url: Option<String>,

    /// The newsletter publication a subscription joins.
    #[arg(long, env = "NEWSLETTER_PUBLICATION_ID")]
    pub newsletter_publication_id: Option<String>,

    /// The newsletter service's bearer key.
    #[arg(long, env = "NEWSLETTER_SERVICE_API_KEY", hide_env_values = true)]
    pub newsletter_service_api_key: Option<String>,

    /// The origins that may read the builder routes' answers: `;`-separated,
    /// `regex:` entries matched as patterns, `*` any origin, others exactly;
    /// unset, the Decentraland front-ends and `http://localhost:<port>`.
    #[arg(long, env = "CORS_ORIGIN")]
    pub cors_origin: Option<String>,

    /// The methods a builder route's preflight allows, `,`-separated; unset
    /// or `*`, every method the routes answer.
    #[arg(long, env = "CORS_METHOD")]
    pub cors_method: Option<String>,

    /// The bytes every wallet's builder uploads together may hold while no
    /// published item names them; past it uploads answer 507. Unset, 1 TB.
    #[arg(long, env = "BUILDER_UNPUBLISHED_MAX_BYTES")]
    pub builder_unpublished_max_bytes: Option<u64>,

    /// The files every wallet's builder uploads together may hold while no
    /// published item names them; past it uploads answer 507. Unset, 10
    /// million.
    #[arg(long, env = "BUILDER_UNPUBLISHED_MAX_FILES")]
    pub builder_unpublished_max_files: Option<u64>,

    /// The item videos the builder reads at once, each spooled to disk up to
    /// 250 MB; 1 to 256. Unset, 32.
    #[arg(long, env = "BUILDER_VIDEO_UPLOADS_MAX")]
    pub builder_video_uploads_max: Option<usize>,

    /// The warehouse a terms-of-service acceptance is forwarded to; the
    /// forward is configured only when the token is set too. Without it the
    /// acceptance is stored and kept for a later forward.
    #[arg(long, env = "WAREHOUSE_URL", hide_env_values = true)]
    pub warehouse_url: Option<String>,

    /// Sent as the warehouse's `x-token` header.
    #[arg(long, env = "WAREHOUSE_TOKEN", hide_env_values = true)]
    pub warehouse_token: Option<String>,

    /// Prefixes the forward's `builder` context as `<prefix>-builder`.
    #[arg(long, env = "WAREHOUSE_CONTEXT_PREFIX")]
    pub warehouse_context_prefix: Option<String>,

    /// The curators' Slack channel (name or id) the `SLACK_BOT_TOKEN` bot
    /// posts curation notices to; no notice is sent without both.
    #[arg(long, env = "SLACK_CURATION_CHANNEL")]
    pub slack_curation_channel: Option<String>,

    /// The creators site (an http(s) URL, such as `https://example.org/create`)
    /// a curation notice links a collection's review page to; its origin also
    /// links the people it names.
    #[arg(long, env = "CREATORS_URL")]
    pub creators_url: Option<String>,

    /// The public origin of the curators' Discourse forum.
    #[arg(long, env = "FORUM_URL")]
    pub forum_url: Option<String>,

    /// The forum API key; all four forum settings are required to post.
    #[arg(long, env = "FORUM_API_KEY", hide_env_values = true)]
    pub forum_api_key: Option<String>,

    /// The forum account the node posts and replies as.
    #[arg(long, env = "FORUM_API_USERNAME")]
    pub forum_api_username: Option<String>,

    /// The numeric category id for collection topics.
    #[arg(long, env = "FORUM_CATEGORY")]
    pub forum_category: Option<String>,

    /// The public builder API base for forum item thumbnails.
    #[arg(long, env = "BUILDER_SERVER_URL")]
    pub builder_server_url: Option<String>,

    /// The secret the wearable validator signs its claims, lease extensions
    /// and verdicts with (at least 32 bytes); unset, no automated review is
    /// requested and the validator routes refuse.
    #[arg(
        long,
        env = "WEARABLE_VALIDATOR_CALLBACK_SECRET",
        hide_env_values = true
    )]
    pub wearable_validator_callback_secret: Option<String>,

    /// Bearer of the moderator routes; unset refuses them.
    #[arg(
        long,
        env = "CATALYRST_CAMERA_REEL_ADMIN_TOKEN",
        hide_env_values = true
    )]
    pub catalyrst_camera_reel_admin_token: Option<String>,

    /// Bearer for this service's metrics endpoint; unset fails closed.
    #[arg(long, env = "CAMERA_REEL_METRICS_BEARER_TOKEN", hide_env_values = true)]
    pub camera_reel_metrics_bearer_token: Option<String>,

    #[arg(long, env = "MAX_IMAGES_PER_USER", default_value_t = 500)]
    pub camera_reel_max_images_per_user: u64,

    /// Cache lifetime for successful world-name lookups; zero disables retention.
    #[arg(
        long,
        visible_alias = "places-cache-ttl-seconds",
        env = "PLACES_CACHE_TTL_SECONDS",
        default_value_t = 300
    )]
    pub camera_reel_places_cache_ttl_seconds: u64,

    /// Maximum world-name lookup entries; zero disables retention.
    #[arg(
        long,
        visible_alias = "places-cache-max-size",
        env = "PLACES_CACHE_MAX_SIZE",
        default_value_t = 1000
    )]
    pub camera_reel_places_cache_max_size: u64,

    /// Base of the rendered `url`/`thumbnailUrl` (old API_URL); empty =
    /// `<PUBLIC_URL>/camera-reel-service`.
    #[arg(long, env = "CAMERA_REEL_API_URL", default_value = "")]
    pub camera_reel_api_url: String,

    /// Public image origin; redirects retain the requested filename.
    /// Use `<CAMERA_REEL_API_URL>/api/files` for the local virtual origin.
    /// An origin serving only blob hashes needs ingress adjustment before cutover.
    /// Without this setting, redirects use the local protected byte route.
    #[arg(long, env = "CAMERA_REEL_BUCKET_URL")]
    pub camera_reel_bucket_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    fn parse(args: &[&str]) -> Result<Config, clap::Error> {
        Config::parse_for_tests(args)
    }

    #[test]
    fn forum_settings_are_optional_and_accept_the_deployed_names() {
        let defaults = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        assert!(defaults.create.forum_url.is_none());
        let configured = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--forum-url",
            "https://forum.example.org",
            "--forum-api-key",
            "sk-example123",
            "--forum-api-username",
            "builder-example",
            "--forum-category",
            "12",
            "--builder-server-url",
            "https://builder-api.example.org",
        ])
        .unwrap();
        assert_eq!(
            configured.create.forum_url.as_deref(),
            Some("https://forum.example.org")
        );
        assert_eq!(
            configured.create.forum_api_key.as_deref(),
            Some("sk-example123")
        );
        assert_eq!(
            configured.create.forum_api_username.as_deref(),
            Some("builder-example")
        );
        assert_eq!(configured.create.forum_category.as_deref(), Some("12"));
        assert_eq!(
            configured.create.builder_server_url.as_deref(),
            Some("https://builder-api.example.org")
        );
    }

    #[test]
    fn create_defaults_are_the_old_bundles_values_and_writes_are_off() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        let c = cli;
        assert!(!c.create.create_writes_enabled, "creator writes are opt-in");
        assert!(c.create.builder_polygon_rpc_url.is_none());
        assert_eq!(c.create.builder_admin_addresses, "");
        assert!(c.create.catalyrst_builder_admin_token.is_none());
        assert!(
            c.create.newsletter_service_url.is_none()
                && c.create.newsletter_publication_id.is_none()
                && c.create.newsletter_service_api_key.is_none()
        );
        assert!(
            c.create.warehouse_url.is_none()
                && c.create.warehouse_token.is_none()
                && c.create.warehouse_context_prefix.is_none()
        );
        assert!(c.create.slack_curation_channel.is_none() && c.create.creators_url.is_none());
        assert!(
            c.create.catalyrst_camera_reel_admin_token.is_none()
                && c.create.camera_reel_metrics_bearer_token.is_none()
                && c.create.camera_reel_bucket_url.is_none()
        );
        assert_eq!(
            (
                c.create.camera_reel_max_images_per_user,
                c.create.camera_reel_api_url.as_str()
            ),
            (500, "")
        );

        let cli = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--create-writes-enabled",
            "true",
            "--builder-admin-addresses",
            "0xABC",
            "--camera-reel-max-images-per-user",
            "7",
        ])
        .unwrap();
        assert!(cli.create.create_writes_enabled);
        assert_eq!(
            (
                cli.create.builder_admin_addresses.as_str(),
                cli.create.camera_reel_max_images_per_user
            ),
            ("0xABC", 7)
        );
    }

    #[test]
    fn camera_places_cache_defaults_and_overrides_match_source() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        assert_eq!(cli.create.camera_reel_places_cache_ttl_seconds, 300);
        assert_eq!(cli.create.camera_reel_places_cache_max_size, 1000);
        let upstream_cli = parse(&[
            "x",
            "--database-url",
            "postgres:///catalyst",
            "--places-cache-ttl-seconds",
            "9",
            "--places-cache-max-size",
            "3",
        ])
        .unwrap();
        assert_eq!(upstream_cli.create.camera_reel_places_cache_ttl_seconds, 9);
        assert_eq!(upstream_cli.create.camera_reel_places_cache_max_size, 3);
        use clap::Args;
        let command = super::CreateConfig::augment_args(clap::Command::new("create"));
        for (id, env) in [
            (
                "camera_reel_places_cache_ttl_seconds",
                "PLACES_CACHE_TTL_SECONDS",
            ),
            ("camera_reel_places_cache_max_size", "PLACES_CACHE_MAX_SIZE"),
        ] {
            let arg = command
                .get_arguments()
                .find(|arg| arg.get_id() == id)
                .unwrap();
            assert_eq!(arg.get_env().unwrap(), env);
        }
        for (ttl, size) in [("0", "0"), ("45", "7")] {
            let cli = parse(&[
                "x",
                "--database-url",
                "postgres:///catalyst",
                "--camera-reel-places-cache-ttl-seconds",
                ttl,
                "--camera-reel-places-cache-max-size",
                size,
            ])
            .unwrap();
            assert_eq!(
                cli.create.camera_reel_places_cache_ttl_seconds,
                ttl.parse::<u64>().unwrap()
            );
            assert_eq!(
                cli.create.camera_reel_places_cache_max_size,
                size.parse::<u64>().unwrap()
            );
        }
    }
}
