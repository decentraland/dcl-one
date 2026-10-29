export type MiscEvents = {
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
    tab: "assets" | "communities" | "creations" | "overview" | "photos" | "places" | "referral-rewards";
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
