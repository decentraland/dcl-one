export type MiscEvents = {
  Click: {
    button?: string;
    card?: string;
    click_id?: string;
    clicked_at?: number;
    download_target?: "app_store" | "creator_hub" | "desktop_installer" | "epic" | "google_play";
    event?: "Download" | "Reels Click Decentraland Logo";
    mac_arch?: "apple_silicon" | "intel" | "unknown";
    os?: "Android" | "Windows" | "iOS" | "macOS";
    place?: "Creator Fund" | "Creator Hub Success Footer" | "Creators Blog" | "Creators Connect" | "Creators Create" | "Creators Faqs" | "Creators Hero" | "Creators Learn" | "Creators Live Scenes" | "Creators Why" | "Download" | "Landing Catch The Vibe" | "Landing Come Hang Out" | "Landing Explore" | "Landing Hero" | "Landing Weekly Rituals" | "come-hang-out" | "come-hang-out-platform-switch" | "download-page" | "landing-hero-epic" | "landing-hero-platform-switch";
    section?: string;
    tab?: string;
    title?: string;
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
  "Reels Click Place": {
    x: string;
    y: string;
  };
  "Reels Click Profile": {
    legacyProfileUrl?: string;
    userAddress: string;
  };
  "Reels Click Thumbnail": {
    imageId: string;
  };
  "Reels Click Wearable": {
    wearableUrn: string;
  };
  "Reels Copy Link": {
    imageId: string;
  };
  "Reels Download": {
    imageId: string;
  };
  "Reels Hide Metadata": {
    imageId: string;
  };
  "Reels Hide Wearables": {
    userAddress: string;
  };
  "Reels Jump In": {
    x: string;
    y: string;
  };
  "Reels Share": {
    imageId: string;
  };
  "Reels Show Metadata": {
    imageId: string;
  };
  "Reels Show Wearables": {
    userAddress: string;
  };
  "Section Viewed": {
    mobile: boolean;
    section_viewed: "Creators Blog" | "Creators Connect" | "Creators Create" | "Creators Faqs" | "Creators Learn" | "Creators Live Scenes" | "Creators Why";
    track_called_at: number;
    track_deferred: boolean;
    track_delivered_at: number;
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
