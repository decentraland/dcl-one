export type MiscEvents = {
  Click: {
    click_id?: string;
    clicked_at?: number;
    download_target: "app_store" | "creator_hub" | "desktop_installer" | "epic" | "google_play";
    event?: "Download";
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    os?: "Android" | "Windows" | "iOS" | "macOS";
    place: "Creator Hub Success Footer" | "Creators Hero" | "Download" | "Landing Come Hang Out" | "Landing Hero" | "come-hang-out" | "come-hang-out-platform-switch" | "download-page" | "landing-hero-epic" | "landing-hero-platform-switch";
    track_called_at: number;
    track_deferred: boolean;
    track_delivered_at: number;
    utm_campaign?: string;
    utm_content?: string;
    utm_medium?: string;
    utm_org?: string;
    utm_source?: string;
    utm_term?: string;
  };
  catalyst_schema_drift: {
    issues_count: number;
    kind: string;
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
    category: "art" | "business" | "education" | "fashion" | "featured" | "game" | "music" | "shop" | "social" | "sports" | null;
    count: number;
    search: null | string;
  };
  profile_communities_load_failed: {
    address: string;
  };
  profile_tab_changed: {
    tab: "assets" | "communities" | "creations" | "overview" | "photos" | "places" | "referral-rewards";
  };
  profile_viewed: {
    address: string;
    has_claimed_name: boolean;
    own: boolean;
    source: "fallback" | "live";
  };
};
