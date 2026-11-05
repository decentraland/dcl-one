//! Exercise posture expansion through typed configuration and the real analytics
//! constructor. No database connection or provider request is made.
use catalyst::social::analytics::{Analytics, Settings};
use catalyst_config::{
    config::Config,
    posture::{self, Posture},
};

#[test]
fn every_posture_can_start_without_social_provider_configuration() {
    let http = catalyst::http::Outbound::new(catalyst::http::HttpClientCfg::serve(16)).unwrap();
    for posture in [Posture::Journey, Posture::Island, Posture::Mirror] {
        let expanded = posture::expand(posture, &|key| {
            (key == "DATABASE_URL").then(|| "postgres:///fixture".into())
        });
        let config = Config::parse_env_for_tests(
            std::iter::once((
                "DATABASE_URL".to_string(),
                "postgres:///fixture".to_string(),
            ))
            .chain(
                expanded
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value)),
            ),
        )
        .unwrap();
        config.social.validate_provider_bootstrap().unwrap();
        assert_eq!(config.social.social_analytics_api_url.as_deref(), Some(""));
        let analytics = Analytics::new(
            &http,
            Settings {
                context: config.social.social_analytics_context,
                api_url: config.social.social_analytics_api_url,
                api_token: config.social.social_analytics_api_token,
                environment: config.social.social_environment,
                request_timeout_ms: config.social.social_analytics_request_timeout_ms,
            },
        )
        .unwrap();
        assert!(
            analytics.is_none(),
            "posture-only configuration does not activate source providers"
        );
    }
}
