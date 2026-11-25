export type LandingsEvents = {
  cast_connection_lost: {
    room_id?: string;
  };
  cast_devices_selected: {
    camera: string;
    mic: string;
    speaker: string;
  };
  cast_ended: {
    stub: boolean;
  };
  cast_ending: Record<string, never>;
  cast_invalid_token: {
    reason: string;
  };
  cast_join_failed: {
    reason?: string;
  };
  cast_join_requested: Record<string, never>;
  cast_not_found_go_home: {
    from: "streamer" | "unknown" | "watcher";
    reason: "ended" | "expired" | "malformed" | "missing";
  };
  cast_not_found_shown: {
    from: "streamer" | "unknown" | "watcher";
    reason: "ended" | "expired" | "malformed" | "missing";
  };
  cast_not_found_view_docs: {
    from: "streamer" | "unknown" | "watcher";
    reason: "ended" | "expired" | "malformed" | "missing";
  };
  cast_permissions_denied: Record<string, never>;
  cast_permissions_granted: Record<string, never>;
  cast_preview_ready: Record<string, never>;
  cast_screenshare_failed: {
    room_id?: string;
    stub: boolean;
  };
  cast_screenshare_started: {
    room_id?: string;
    stub: boolean;
  };
  cast_token_checked: Record<string, never>;
  cast_token_valid: {
    is_world?: boolean;
    place_name?: string;
  };
  cast_watch_chat_opened: Record<string, never>;
  cast_watch_joined: {
    place_name: string;
    room_id: string;
    stub: boolean;
  };
  cast_watch_left: Record<string, never>;
  cast_watch_lost: {
    reason: string;
  };
  cast_watch_muted: Record<string, never>;
  cast_watch_refused: {
    kind: "error" | "waiting";
    reason: string;
  };
  cast_watch_unmuted: Record<string, never>;
  cast_went_live: {
    room_id?: string;
    stub: boolean;
  };
  jump_in_completed: {
    launch_url: string;
    place_id: string;
  };
  jump_in_failed: {
    error: string;
    place_id: string;
  };
  jump_in_started: {
    place_id: string;
  };
  lp_blog_post_clicked: {
    slug: string;
  };
  lp_blog_post_viewed: {
    slug: string;
  };
  lp_blog_viewed: {
    post_count: number;
  };
  lp_community_created: {
    community_id?: string;
    stub?: true;
  };
  lp_community_submit_attempted: {
    privacy: "private" | "public";
    visibility: "all" | "unlisted";
  };
  lp_community_submit_failed: {
    error?: string;
  };
  lp_creatorhub_docs_clicked: Record<string, never>;
  lp_creatorhub_download_clicked: {
    arch: "amd64" | "arm64";
    file_name: null | string;
    kind: "primary" | "secondary";
    os: "macos" | "windows";
  };
  lp_creatorhub_download_success: {
    os: "macos" | "windows";
  };
  lp_creatorhub_download_viewed: {
    arch: string;
    os: "macos" | "windows";
    overridden: boolean;
    version: string;
  };
  lp_creatorhub_link_shared: {
    method: "copy" | "share";
  };
  lp_discover_viewed: Record<string, never>;
  lp_event_card_clicked: {
    event_id: string;
  };
  lp_event_jump_in: {
    event_id: string;
    position: null | string;
  };
  lp_event_viewed: {
    event_id: string;
    live: boolean;
  };
  lp_hangout_preview_opened: Record<string, never>;
  lp_hangout_signin_gate_viewed: Record<string, never>;
  lp_hangout_started: Record<string, never>;
  lp_hangout_step_completed: {
    from: "cover" | "details" | "location" | "preview" | "review" | "schedule" | "signinGate" | "submitted" | "submitting";
    to: "cover" | "details" | "location" | "preview" | "review" | "schedule" | "signinGate" | "submitted" | "submitting";
  };
  lp_hangout_submit_attempted: {
    location: "land" | "world";
    recurrent: boolean;
  };
  lp_hangout_submit_failed: {
    error?: string;
  };
  lp_hangout_submitted: {
    approved?: boolean;
    event_id?: string;
  };
  lp_help_creator_fund_clicked: {
    button: string;
  };
  lp_help_viewed: Record<string, never>;
  lp_invite_referrer_resolved: {
    referrer_handle: string;
    resolved: boolean;
    source: string;
  };
  lp_invite_rewards_viewed: {
    accepted: number;
    current_tier: number;
  };
  lp_invite_viewed: {
    has_referrer: boolean;
    referrer_handle: null | string;
    resolved: boolean;
  };
  lp_legal_section_clicked: {
    doc: "brand" | "content" | "creatorFund" | "credits" | "ethics" | "privacy" | "referral" | "rewards" | "security" | "terms";
    section: string;
  };
  lp_legal_viewed: {
    doc: "brand" | "content" | "creatorFund" | "credits" | "ethics" | "privacy" | "referral" | "rewards" | "security" | "terms";
  };
  lp_whatson_viewed: {
    filter: null | string;
    live_count: number;
    search: null;
    upcoming_count: number;
  };
  report_category_set: {
    reason: "cheating" | "harassment" | "illegal_content" | "impersonation" | "scam_phishing";
  };
  report_completed: {
    evidence_count: number;
    reason: "" | "cheating" | "harassment" | "illegal_content" | "impersonation" | "scam_phishing";
    report_id: string;
  };
  report_details_set: {
    description_len: number;
  };
  report_evidence_added: {
    file_count: number;
  };
  report_failed: {
    reason: string;
  };
  report_review_reached: Record<string, never>;
  report_started: Record<string, never>;
  report_submit_started: {
    evidence_count: number;
    reason: "" | "cheating" | "harassment" | "illegal_content" | "impersonation" | "scam_phishing";
  };
  report_target_set: {
    has_reporter: boolean;
  };
  report_validation_failed: {
    fields: string[];
    step: string;
  };
};
