export type MiscEvents = {
  Click: {
    action?: string;
    card?: string;
    click_id?: string;
    clicked_at?: number;
    download_target?:
      "app_store" | "desktop_installer" | "epic" | "google_play";
    event?: string;
    href?: string;
    link?: string;
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    os?: string;
    place: string;
    platform?: string;
    section?: string;
    track_called_at?: number;
    track_deferred?: boolean;
    track_delivered_at?: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_failed: {
    anon_user_id?: string;
    arch: "amd64" | "arm64";
    auth_state: "anonymous" | "authenticated";
    click_id?: string;
    download_target: "desktop_installer";
    duration_ms: number;
    failed_at: number;
    href: string;
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    ms_since_click?: number;
    os: "Windows" | "macOS";
    place?:
      | "come-hang-out"
      | "come-hang-out-platform-switch"
      | "creator-hub-download-page"
      | "creator-hub-success-page"
      | "creators-hero"
      | "download-modal"
      | "download-page"
      | "download-success-footer"
      | "jump-in-already-user"
      | "jump-in-landing"
      | "landing-hero"
      | "landing-hero-epic"
      | "landing-hero-platform-switch"
      | "lets-play-landing"
      | "unknown";
    reason: string;
    revisit: number;
    started_at: number;
    track_called_at: number;
    track_deferred: true;
    track_delivered_at: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_funnel_exit: {
    anon_user_id?: string;
    arch: "amd64" | "arm64";
    auth_state: "anonymous" | "authenticated";
    click_id?: string;
    download_failed_fired: boolean;
    download_started_fired: boolean;
    download_success_fired: boolean;
    ms_on_page: number;
    os: "Windows" | "macOS";
    place:
      | "come-hang-out"
      | "come-hang-out-platform-switch"
      | "creator-hub-download-page"
      | "creator-hub-success-page"
      | "creators-hero"
      | "download-modal"
      | "download-page"
      | "download-success-footer"
      | "jump-in-already-user"
      | "jump-in-landing"
      | "landing-hero"
      | "landing-hero-epic"
      | "landing-hero-platform-switch"
      | "lets-play-landing"
      | "unknown";
    revisit: number;
  };
  download_page_exit: {
    cta_clicked: boolean;
    ms_on_page: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_redirect_failed: {
    arch?: "amd64" | "arm64";
    download_target: string;
    os: string;
    place: string;
    reason: string;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_started: {
    anon_user_id?: string;
    arch: "amd64" | "arm64";
    auth_state: "anonymous" | "authenticated";
    click_id?: string;
    download_target: "desktop_installer";
    href: string;
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    ms_since_click?: number;
    os: "Windows" | "macOS";
    place?:
      | "come-hang-out"
      | "come-hang-out-platform-switch"
      | "creator-hub-download-page"
      | "creator-hub-success-page"
      | "creators-hero"
      | "download-modal"
      | "download-page"
      | "download-success-footer"
      | "jump-in-already-user"
      | "jump-in-landing"
      | "landing-hero"
      | "landing-hero-epic"
      | "landing-hero-platform-switch"
      | "lets-play-landing"
      | "unknown";
    revisit: number;
    started_at: number;
    track_called_at: number;
    track_deferred: true;
    track_delivered_at: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_success: {
    anon_user_id?: string;
    arch: "amd64" | "arm64";
    auth_state: "anonymous" | "authenticated";
    bytes_transferred?: number;
    click_id?: string;
    delivery_mode: "anchor_fallback" | "anchor_native" | "streamed";
    download_target: "desktop_installer";
    duration_ms: number;
    filename: string;
    gateway_request_id?: string;
    href: string;
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    ms_since_click?: number;
    os: "Windows" | "macOS";
    place?:
      | "come-hang-out"
      | "come-hang-out-platform-switch"
      | "creator-hub-download-page"
      | "creator-hub-success-page"
      | "creators-hero"
      | "download-modal"
      | "download-page"
      | "download-success-footer"
      | "jump-in-already-user"
      | "jump-in-landing"
      | "landing-hero"
      | "landing-hero-epic"
      | "landing-hero-platform-switch"
      | "lets-play-landing"
      | "unknown";
    revisit: number;
    started_at: number;
    succeeded_at: number;
    track_called_at: number;
    track_deferred: true;
    track_delivered_at: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  download_success_arrived: {
    arch: "amd64" | "arm64";
    auth_state: "anonymous" | "authenticated";
    click_id?: string;
    download_target: string;
    ms_since_click?: number;
    os: "Windows" | "macOS";
    place:
      | "come-hang-out"
      | "come-hang-out-platform-switch"
      | "creator-hub-download-page"
      | "creator-hub-success-page"
      | "creators-hero"
      | "download-modal"
      | "download-page"
      | "download-success-footer"
      | "jump-in-already-user"
      | "jump-in-landing"
      | "landing-hero"
      | "landing-hero-epic"
      | "landing-hero-platform-switch"
      | "lets-play-landing"
      | "unknown";
    revisit: number;
    track_called_at: number;
    track_deferred: true;
    track_delivered_at: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  catalyst_schema_drift: {
    issues?: string[];
    issues_count?: number;
    kind?: string;
    module?: string;
    path?: string;
  };
  client_error: {
    client_ts: string;
    correlation_id: string;
    engine_build: string;
    error_message: string;
    error_name: string;
    error_source: string;
    error_stack: string;
    ua: string;
    url: string;
  };
  ffw_shop_landing: {
    item_id: string;
    nid: string;
    on_sale: boolean;
  };
  page_not_found: {
    path: string;
    referrer: string;
    spa?: true;
    ua: string;
  };
  place_card_clicked: {
    place_id: string;
  };
  place_list_viewed: {
    category: null | string;
    count: number;
    search: null | string;
  };
  profile_card_clicked: {
    item_id: string;
    tab:
      | "assets"
      | "communities"
      | "creations"
      | "overview"
      | "photos"
      | "places"
      | "referral-rewards";
  };
  profile_communities_load_failed: {
    address: string;
  };
  profile_tab_changed: {
    tab:
      | "assets"
      | "communities"
      | "creations"
      | "overview"
      | "photos"
      | "places"
      | "referral-rewards";
  };
  profile_viewed: {
    address: string;
    has_claimed_name: boolean;
    own: boolean;
    source: "fallback" | "live";
  };
  scene_analytics_exported: {
    scene_id: string;
    scene_type: "genesis" | "world";
  };
  scene_analytics_viewed: {
    scene_id?: string;
    scene_type?: "genesis" | "world";
    source: string;
  };
};
