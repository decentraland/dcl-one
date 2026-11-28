//! The creator plane's settings (builder and camera_reel).

use clap::Args;

/// The creator plane: builder and camera_reel.
#[derive(Args, Clone, Debug)]
pub struct CreateConfig {
    /// Opens the creator plane's writes; off, each builder and camera reel
    /// write answers 503 with `Retry-After`.
    #[arg(long, env = "CREATE_WRITES_ENABLED", default_value_t = false, action = clap::ArgAction::Set)]
    pub create_writes_enabled: bool,

    /// Polygon JSON-RPC (https to a public host) the builder asks whether a
    /// collection's address holds its contract; unset, a save that moves a
    /// collection off its stored address answers 503.
    #[arg(long, env = "BUILDER_POLYGON_RPC_URL", hide_env_values = true)]
    pub builder_polygon_rpc_url: Option<String>,

    /// The origins that may read the builder routes' answers: `;`-separated,
    /// `regex:` entries matched as patterns, `*` any origin, others exactly;
    /// unset, the Decentraland front-ends and `http://localhost:<port>`.
    #[arg(long, env = "CORS_ORIGIN")]
    pub cors_origin: Option<String>,

    /// The methods a builder route's preflight allows, `,`-separated; unset
    /// or `*`, every method the routes answer.
    #[arg(long, env = "CORS_METHOD")]
    pub cors_method: Option<String>,

    /// Bearer of the moderator routes; unset refuses them.
    #[arg(
        long,
        env = "CATALYRST_CAMERA_REEL_ADMIN_TOKEN",
        hide_env_values = true
    )]
    pub catalyrst_camera_reel_admin_token: Option<String>,

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
    fn create_defaults_are_the_old_bundles_values_and_writes_are_off() {
        let cli = parse(&["x", "--database-url", "postgres:///catalyst"]).unwrap();
        let c = cli;
        assert!(!c.create.create_writes_enabled, "creator writes are opt-in");
        assert!(c.create.builder_polygon_rpc_url.is_none());
        assert!(
            c.create.catalyrst_camera_reel_admin_token.is_none()
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
            "--camera-reel-max-images-per-user",
            "7",
        ])
        .unwrap();
        assert!(cli.create.create_writes_enabled);
        assert_eq!(cli.create.camera_reel_max_images_per_user, 7);
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
