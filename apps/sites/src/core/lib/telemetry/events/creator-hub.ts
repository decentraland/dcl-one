export type CreatorHubEvents = {
  bd_activity_opened: {
    collection_id: string;
  };
  bd_approval_error_copied: {
    collection_id: string;
  };
  bd_approval_flow_completed: {
    collection_id: string;
    mode: "approve" | "deploy_missing" | "enable";
  };
  bd_approval_flow_failed: {
    collection_id: string;
    error: string;
    step: "approve" | "changed" | "deploy" | "prepare" | "rescue";
  };
  bd_approval_flow_started: {
    collection_id: string;
    mode: "approve" | "deploy_missing" | "enable";
  };
  bd_collection_approve_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_approved: {
    collection_id: string;
    tx_hash: string;
  };
  bd_collection_collaborators_set: {
    collection_id: string;
    count: number;
  };
  bd_collection_collaborators_set_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_delete_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_deleted: {
    collection_id: string;
  };
  bd_collection_enable_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_enabled: {
    collection_id: string;
    tx_hash: string;
  };
  bd_collection_opened_by_contract: {
    collection_id?: string;
    outcome: "error" | "found" | "invalid" | "not_found";
  };
  bd_collection_ownership_transfer_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_ownership_transferred: {
    collection_id: string;
    new_owner: string;
  };
  bd_collection_reject_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_rejected: {
    collection_id: string;
    tx_hash: string;
  };
  bd_collection_save_failed: {
    collection_id: string;
    error: string;
  };
  bd_collection_saved: {
    collection_id: string;
    item_count: number;
  };
  bd_collection_senders_set: {
    collection_id: string;
    count: number;
  };
  bd_collection_senders_set_failed: {
    collection_id: string;
    error: string;
  };
  bd_credits_buy_opened: {
    collection_id: string;
    shortfall: number;
  };
  bd_credits_checkout_canceled: {
    collection_id: string;
    order_id: string;
  };
  bd_credits_checkout_settled: {
    credits: null | number;
    order_id: string;
    status: "abandoned" | "credited" | "crediting" | "failed" | "initiated" | "pending" | "processing";
  };
  bd_credits_checkout_started: {
    collection_id: string;
    credits: number;
    order_id: string;
    pack_id: string;
    quantity: number;
    resume_saved: boolean;
    shortfall: number;
    usd: number;
  };
  bd_curation_appeal_failed: {
    collection_id: string;
    error: string;
  };
  bd_curation_appealed: {
    collection_id: string;
    has_note: boolean;
  };
  bd_curation_approved: {
    collection_id: string;
    has_note: boolean;
  };
  bd_curation_filter_changed: {
    filter: string;
    value: string;
  };
  bd_curation_push_failed: {
    collection_id: string;
    error: string;
  };
  bd_curation_pushed: {
    collection_id: string;
    first_review: boolean;
    has_note: boolean;
  };
  bd_curation_reject_failed: {
    collection_id: string;
    error: string;
  };
  bd_curation_rejected: {
    collection_id: string;
    first_review: boolean;
    reasons: ("thumbnail" | "emote" | "other" | "clipping" | "category_hides" | "rigging" | "triangle_count" | "file_size" | "textures_materials" | "reversed_faces" | "smart_wearable_files" | "content_policy_ip")[];
  };
  bd_curator_assign_failed: {
    collection_id: string;
    error: string;
  };
  bd_curator_assigned: {
    assignee: null | string;
    collection_id: string;
  };
  bd_curator_unassigned: {
    assignee: null | string;
    collection_id: string;
  };
  bd_discount_abandoned: {
    collection_id: string;
    last_step: string;
  };
  bd_discount_create_failed: {
    capped: boolean;
    collection_id: string;
    discount_pct: number;
    discounted_count: number;
    duration_days: number;
    error: string;
  };
  bd_discount_created: {
    capped: boolean;
    collection_id: string;
    discount_id: string;
    discount_pct: number;
    discounted_count: number;
    duration_days: number;
  };
  bd_discount_end_failed: {
    collection_id: string;
    discount_id: string;
    error: string;
  };
  bd_discount_ended: {
    collection_id: string;
    discount_id: string;
  };
  bd_discount_opened: {
    collection_id: string;
    trigger: "header" | "menu";
  };
  bd_discount_reviewed: {
    capped: boolean;
    collection_id: string;
    discount_pct: number;
    duration_days: number;
    mana_items: number;
    scheduled: boolean;
  };
  bd_emote_played: {
    emote_played_base: boolean;
    emote_played_item_id: null | string;
    emote_played_name: string;
    previewed_wearable_item_id: null | string;
    previewed_wearable_name: null | string;
  };
  bd_entities_deploy_failed: {
    collection_id: string;
    item_count: number;
  };
  bd_entities_deployed: {
    collection_id: string;
    item_count: number;
  };
  bd_error_page_shown: Record<string, never>;
  bd_forum_assignee_post_failed: {
    collection_id: string;
    error: string;
  };
  bd_forum_post_create_failed: {
    collection_id: string;
    error: string;
    trigger: "publish" | "recovery";
  };
  bd_forum_post_created: {
    collection_id: string;
    existing: boolean;
    trigger: "publish" | "recovery";
  };
  bd_forum_post_opened: {
    collection_id: string;
    surface: "curation_list" | "detail" | "detail_menu" | "review_verdict";
  };
  bd_item_changes_reset: {
    item_id: string;
  };
  bd_item_changes_reset_failed: {
    error: string;
    item_id: string;
  };
  bd_item_delete_failed: {
    error: string;
    item_id: string;
  };
  bd_item_deleted: {
    item_id: string;
  };
  bd_item_editor_save_failed: {
    error: string;
    item_id: string;
  };
  bd_item_editor_saved: {
    item_id: string;
  };
  bd_item_file_change_failed: {
    error: string;
    item_id: string;
  };
  bd_item_file_changed: {
    body_shape: "both" | "female" | "male";
    file_kind: string;
    item_id: string;
    mode: "add_representation" | "replace_all" | "replace_shape";
    thumbnail_regenerated: boolean;
  };
  bd_item_listing_remove_failed: {
    collection_id: string;
    error: string;
    item_id: string;
  };
  bd_item_listing_removed: {
    collection_id: string;
    item_id: string;
  };
  bd_item_listing_set: {
    collection_id: string;
    is_giveaway: boolean;
    is_update: boolean;
    item_id: string;
    price_kind: "credits" | "free" | "mana";
  };
  bd_item_listing_set_failed: {
    collection_id: string;
    error: string;
    is_giveaway: boolean;
    is_update: boolean;
    item_id: string;
    price_kind: "credits" | "free" | "mana";
  };
  bd_item_metrics_repaired: {
    item_id: string;
    saved: boolean;
  };
  bd_item_move_failed: {
    error: string;
    item_id: string;
  };
  bd_item_moved: {
    collection_id: string;
    item_id: string;
  };
  bd_item_preview_processed: {
    attempts: number;
    batch_size: number;
    duration_ms: number;
    flow: "add_items" | "item_file_change";
    item_type: "emote" | "wearable" | null;
    outcome: "error" | "ok" | "timeout";
    renderer: "babylon" | "unity" | null;
  };
  bd_item_previewed: {
    item_id: null | string;
    item_is_third_party: boolean;
    item_name: string;
    item_type: "emote" | "wearable";
  };
  bd_item_validation_details_opened: {
    item_id: null | string;
    status: "errors" | "idle" | "loading" | "pass" | "warnings";
    trigger: string;
  };
  bd_item_validation_rerun: {
    item_id: null | string;
    new_status: "errors" | "idle" | "loading" | "pass" | "warnings";
    previous_status: "errors" | "idle" | "loading" | "pass" | "warnings";
    trigger: string;
  };
  bd_items_add_failed: {
    collection_id: string;
    error: string;
    item_count: number;
  };
  bd_items_added: {
    collection_id: string;
    error?: "duplicate_urn" | "generic" | "locked" | "published" | "too_big" | "urn_taken";
    failed_count: number;
    item_count: number;
    origin: string;
  };
  bd_items_rescue_failed: {
    collection_id: string;
    error: string;
  };
  bd_items_rescued: {
    collection_id: string;
    item_count: number;
  };
  bd_items_send_failed: {
    collection_id: string;
    copies: number;
    error: string;
    item_count: number;
    recipient_count: number;
  };
  bd_items_sent: {
    collection_id: string;
    copies: number;
    item_count: number;
    recipient_count: number;
  };
  bd_linked_changes_push_failed: {
    collection_id: string;
    error: string;
    items: number;
  };
  bd_linked_changes_pushed: {
    collection_id: string;
    failed: number;
    items: number;
  };
  bd_linked_cheque_signed: {
    collection_id: string;
    qty: number;
    third_party_id: string;
  };
  bd_linked_collection_approve_failed: {
    collection_id: string;
    error?: string;
    items?: number;
    step: string;
  };
  bd_linked_collection_approve_started: {
    collection_id: string;
    third_party_id: string;
  };
  bd_linked_collection_approved: {
    collection_id: string;
    items: number;
    third_party_id: string;
  };
  bd_linked_collection_contract_edited: {
    collection_id: string;
    from_address: null | string;
    from_network: "amoy" | "ape_caldera" | "ape_mainnet" | "base_mainnet" | "base_sepolia" | "mainnet" | "matic" | "monad_mainnet" | "monad_testnet" | "sepolia" | null;
    items: number;
    to_address: string;
    to_network: "amoy" | "ape_caldera" | "ape_mainnet" | "base_mainnet" | "base_sepolia" | "mainnet" | "matic" | "monad_mainnet" | "monad_testnet" | "sepolia";
  };
  bd_linked_collection_created: {
    collection_id: string;
    collection_name: string;
    linked_contract_address: string;
    linked_contract_network: "amoy" | "ape_caldera" | "ape_mainnet" | "base_mainnet" | "base_sepolia" | "mainnet" | "matic" | "monad_mainnet" | "monad_testnet" | "sepolia";
    third_party_id?: string;
  };
  bd_linked_collection_reject_failed: {
    collection_id: string;
    error: string;
  };
  bd_linked_collection_rejected: {
    collection_id: string;
    items: number;
  };
  bd_linked_collection_urn_changed: {
    new_urn: string;
    old_urn: string;
  };
  bd_linked_collection_viewed: {
    collection_id: string;
    item_count: number;
    network?: string;
  };
  bd_linked_collections_filtered: {
    count: number;
  };
  bd_linked_contract_copied: {
    collection_id: string;
    network?: string;
  };
  bd_linked_item_mapping_edited: {
    item_id: string;
    mapping_type: "any" | "multiple" | "range" | "single";
  };
  bd_linked_item_urn_changed: {
    new_urn: string;
    old_urn?: string;
  };
  bd_linked_items_all_selected: {
    collection_id: string;
    count: number;
    filter: "all" | "draft" | "pending_mapping" | "pending_migration" | "published" | "ready_to_push" | "under_review";
  };
  bd_linked_items_edited: {
    collection_id: string;
    failed_count: number;
    item_count: number;
  };
  bd_linked_items_publish_failed: {
    collection_id: string;
    error: string;
    step: string;
  };
  bd_linked_items_publish_started: {
    collection_id: string;
    items: number;
    pushed: number;
    third_party_id: string;
  };
  bd_linked_items_published: {
    collection_id: string;
    items: number;
    pushed: number;
    third_party_id: string;
    tx_hash?: string;
  };
  bd_linked_merkle_root_set: {
    collection_id: string;
    third_party_id: string;
    tx_hash: string;
  };
  bd_live_preview_added_to_collection: {
    item_type: string;
    push_count: number;
    signed_in: boolean;
  };
  bd_live_preview_connect_failed: {
    error: "bridge_response" | "invalid_state" | "model_fetch" | "permission_denied" | "unreachable";
  };
  bd_live_preview_connected: {
    long_poll: boolean;
  };
  bd_live_preview_model_received: {
    category?: string;
    item_type?: "emote" | "wearable";
    since_last_ms?: number;
  };
  bd_live_preview_tuned: {
    control: "category" | "hides" | "loop" | "spring_bones";
    item_type: string;
  };
  bd_modal_closed: {
    name: string;
  };
  bd_modal_opened: {
    name: string;
  };
  bd_preview_in_world_opened: {
    collection_id: string;
    item_count: number;
    launched: boolean;
    trigger: string;
  };
  bd_preview_renderer_picked: {
    measured_mbps: null | number;
    reason: "connection-ok" | "flag-off" | "gpu-capability" | "low-device-memory" | "optimistic-default" | "save-data" | "slow-connection" | "url-override";
    renderer: "babylon" | "unity";
  };
  bd_publish_collection_failed: {
    collection_id: string;
    error: string;
    is_fiat: false;
    is_retry: boolean;
    used_credits: boolean;
  };
  bd_publish_collection_reverted: {
    collection_id: string;
    tx_hash: string;
    used_credits: boolean;
  };
  bd_publish_collection_started: {
    collection_id: string;
    email_source: string;
    name_changed: boolean;
  };
  bd_publish_collection_submitted: {
    collection_id: string;
    credits_amount: number;
    is_fiat: false;
    is_retry: boolean;
    item_count: number;
    tx_hash: string;
    used_credits: boolean;
  };
  bd_publish_validation_resolved: {
    action: "back" | "continue";
    collection_id: string;
    errors: number;
    flow: "publish" | "push_changes";
    warnings: number;
  };
  bd_publish_validation_result: {
    blocking: boolean;
    collection_id: string;
    duration_ms: number;
    errors: number;
    flow: "publish" | "push_changes";
    items_with_issues: number;
    oversized_items: number;
    warnings: number;
  };
  bd_publish_validation_started: {
    collection_id: string;
    flow: "publish" | "push_changes";
    item_count: number;
  };
  bd_recipients_pasted: {
    added: number;
    collection_id: string;
    cut: number;
    invalid: number;
    repeated: number;
  };
  bd_review_comment_post_failed: {
    collection_id: string;
    error: string;
    role: "creator" | "curator";
  };
  bd_review_comment_posted: {
    collection_id: string;
    has_item: boolean;
    length: number;
    role: "creator" | "curator";
  };
  bd_review_modal_opened: {
    collection_id: string;
    modal: "activity" | "additional" | "issues" | "limit" | "request" | "reviewing" | "sent";
    stage: "appealed" | "approved" | "automated_reviewing" | "awaiting_curator" | "needs_curator" | "rejected_by_curator" | "rejected_by_validator" | null;
  };
  bd_review_preview_in_world_opened: {
    collection_id: string;
    item_count: number;
    launched: boolean;
  };
  bd_review_validation_opened: {
    collection_id: string;
    status: "errors" | "idle" | "loading" | "pass" | "warnings";
  };
  bd_review_validation_result: {
    collection_id: string;
    duration_ms: number;
    errors: number;
    item_count: number;
    items_with_issues: number;
    warnings: number;
  };
  bd_sales_enable_failed: {
    collection_id: string;
    error: string;
  };
  bd_sales_enabled: {
    collection_id: string;
  };
  bd_scene_boundaries_toggled: {
    trigger: "editor" | "live_preview";
    visible: boolean;
  };
  bd_sign_in_opened: Record<string, never>;
  bd_third_party_disable_failed: {
    error: string;
    third_party_id: string;
  };
  bd_third_party_disabled: {
    third_party_id: string;
    tx_hash: string;
  };
  bd_third_party_enable_failed: {
    error: string;
    third_party_id: string;
  };
  bd_third_party_enabled: {
    third_party_id: string;
    tx_hash: string;
  };
  bd_validation_request_failed: {
    collection_id: string;
    error: string;
  };
  bd_validation_requested: {
    attempts_left: number;
    collection_id: string;
  };
  ch_mp_paired: Record<string, never>;
  ch_mp_replay_completed: {
    run: null | string;
    tier: "a" | "b";
  };
  ch_mp_replay_failed: {
    error: string;
    run: null | string;
  };
  ch_mp_replay_requested: {
    run: null | string;
    tier?: "a" | "b";
  };
  ch_mp_run_completed: {
    run: null | string;
  };
  ch_mp_run_failed: {
    detail?: string;
    run: null | string;
  };
  ch_mp_run_launched: Record<string, never>;
  ch_mp_run_rejected: {
    error: string;
  };
  creator_builder_redirect: {
    from: string;
    fromPath?: string;
    query?: string;
    to: string;
  };
  creator_builder_redirect_dashboard_viewed: {
    total: number;
    window_days: number;
  };
  creator_claim_name_redirect: {
    from: string;
    to: string;
  };
  creator_collection_published: {
    funnel_step: number;
    source_event?: string;
  };
  creator_collection_viewed: {
    funnel_step: number;
    source_event?: string;
  };
  creator_curation_submitted: {
    funnel_step: number;
    source_event?: string;
  };
  creator_item_edited: {
    funnel_step: number;
    source_event?: string;
  };
  creator_item_listed: {
    funnel_step: number;
    source_event?: string;
  };
  creator_publish_started: {
    funnel_step: number;
    source_event?: string;
  };
  creator_sale_completed: {
    funnel_step: number;
    source_event?: string;
  };
  creator_store_viewed: {
    funnel_step: number;
    source_event?: string;
  };
};
