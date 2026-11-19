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
  ch_claim_name_available: {
    name: string;
  };
  ch_claim_name_completed: {
    name: string;
    stub: true;
    token_id?: string;
    tx_hash?: string;
    world_name: string;
  };
  ch_claim_name_mint_submitted: {
    name: string;
    simulated: true;
  };
  ch_claim_name_returned_to_publish: {
    name: string;
    world_name: string;
  };
  ch_claim_name_review_reached: {
    name: string;
    price_mana: string;
  };
  ch_claim_name_started: {
    name: string;
  };
  ch_claim_name_unavailable: {
    name: string;
  };
  ch_create_project_completed: {
    files: string[];
    folder?: string;
    name: string;
    path: string;
    template: string;
    via?: "canceled" | "directory" | "download";
    written: boolean;
  };
  ch_create_project_failed: {
    code?: string;
    message: string;
  };
  ch_create_project_name_set: {
    name: string;
  };
  ch_create_project_path_invalid: {
    path: string;
  };
  ch_create_project_path_set: {
    path: string;
  };
  ch_create_project_scaffolding: {
    name: string;
    path: string;
    template: string;
  };
  ch_create_project_started: Record<string, never>;
  ch_create_project_template_selected: {
    preselected?: true;
    template: string;
  };
  ch_delete_cancelled: {
    local?: true;
    project_id: string;
  };
  ch_delete_confirmed: {
    delete_files: boolean;
    local?: true;
    local_files?: "deleted" | "kept";
    overrode_pointers?: number;
    project_id: string;
    replaced_entity?: string;
    simulated: false;
    status?: number;
    tombstone_id?: string;
  };
  ch_delete_done_viewed: {
    deleted_id: null | string;
    remaining: number;
  };
  ch_delete_failed: {
    error: string;
    project_id: string;
    simulated: false;
    status: number;
  };
  ch_delete_files_opted_in: {
    local?: true;
    project_id: string;
  };
  ch_delete_files_toggled: {
    checked: boolean;
    local?: true;
    project_id: string;
  };
  ch_delete_no_deployment: {
    base: string;
    project_id: string;
  };
  ch_delete_opened: {
    local?: true;
    project_id: string;
    published: boolean | null;
  };
  ch_delete_scenes_viewed: {
    count: number;
    source: "empty" | "live" | "unavailable";
  };
  ch_deploy_world_completed: {
    jump_url?: string;
    name?: string;
    target: "land" | "world";
  };
  ch_deploy_world_confirm_reached: {
    name?: string;
    target: "land" | "world";
  };
  ch_deploy_world_destination_selected: {
    target: "land" | "world";
  };
  ch_deploy_world_failed: {
    error?: string;
    name?: string;
  };
  ch_deploy_world_name_selected: {
    name?: string;
  };
  ch_deploy_world_names_empty: Record<string, never>;
  ch_deploy_world_quota_exceeded: {
    max_mb: number;
    phase?: string;
    total_bytes: number;
  };
  ch_deploy_world_review_reached: {
    exceeded: boolean;
    target: "land" | "world";
    total_bytes: number;
  };
  ch_deploy_world_started: {
    target: "land" | "world";
  };
  ch_home_card_opened: {
    card: string;
  };
  ch_home_signin_clicked: Record<string, never>;
  ch_home_start_building: Record<string, never>;
  ch_home_viewed: {
    scene_count: number;
  };
  ch_learn_signin_clicked: Record<string, never>;
  ch_learn_viewed: Record<string, never>;
  ch_manage_card_clicked: {
    id: string;
    role: "collaborator" | "operator" | "owner";
  };
  ch_manage_empty_viewed: {
    filter: "published" | "unpublished";
  };
  ch_manage_filter_changed: {
    filter: "published" | "unpublished";
  };
  ch_manage_searched: {
    q: null | string;
  };
  ch_manage_sorted: {
    sort: "domain" | "last_published";
  };
  ch_manage_storage_opened: Record<string, never>;
  ch_manage_viewed: {
    address: null | string;
    count: number;
    filter: "published" | "unpublished";
    search: null | string;
    sort: "domain" | "last_published";
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
  ch_scenes_clicked: {
    card?: string;
    entity?: string;
    from?: string;
    local?: true;
    ok?: boolean;
    parcel?: string;
    seeded?: boolean;
    to?: string;
  };
  ch_scenes_empty_viewed: Record<string, never>;
  ch_scenes_import_failed: {
    files?: number;
    has_scene_json?: boolean;
    reason: string;
  };
  ch_scenes_signin_clicked: Record<string, never>;
  ch_scenes_viewed: {
    count: number;
    empty: boolean;
    error: boolean;
  };
  ch_studio_opened: {
    source: string;
    template_id: string;
  };
  ch_template_previewed: {
    template_id: string;
  };
  ch_template_selected: {
    template_id: string;
    title: string;
  };
  ch_template_view_code: {
    template_id: string;
  };
  ch_templates_signin_clicked: Record<string, never>;
  ch_templates_viewed: Record<string, never>;
  ch_world_perms_access_type_set: {
    access_type: string;
  };
  ch_world_perms_collaborator_validated: {
    address?: string;
    valid: boolean;
  };
  ch_world_perms_completed: {
    access_type: "allow-list" | "shared-secret" | "unrestricted";
    addresses?: number;
    stub: boolean;
  };
  ch_world_perms_confirm_reached: {
    access_type: "allow-list" | "shared-secret" | "unrestricted";
    collaborators: number;
  };
  ch_world_perms_invalid_address: Record<string, never>;
  ch_world_perms_invite_submitted: {
    channel: "community" | "csv" | "wallet";
  };
  ch_world_perms_password_set: Record<string, never>;
  ch_world_perms_started: Record<string, never>;
  ch_world_settings_changed: {
    field: string;
    tab: "details" | "layout" | "misc";
    world: string;
  };
  ch_world_settings_discarded: {
    change_count: number;
    world: string;
  };
  ch_world_settings_opened: {
    world: string;
  };
  ch_world_settings_review_reached: {
    change_count: number;
    world: string;
  };
  ch_world_settings_saved: {
    fields: string[];
    stub: boolean;
    world: string;
  };
  ch_world_settings_saving: {
    fields: string[];
    world: string;
  };
  ch_world_settings_tab_viewed: {
    tab: "details" | "layout" | "misc";
    world: string;
  };
  ch_worlds_storage_asset_selected: {
    kind: string;
    world: string;
  };
  ch_worlds_storage_clear_failed: {
    count: number;
    error: string;
    namespace: "env" | "players" | "scene";
    world: null | string;
  };
  ch_worlds_storage_cleared: {
    count: number;
    namespace: "env" | "players" | "scene";
    reason?: string;
    stub?: true;
    world: null | string;
  };
  ch_worlds_storage_dialog_opened: {
    key: null | string;
    mode: string;
    world: null | string;
  };
  ch_worlds_storage_namespace_changed: {
    namespace: "env" | "players" | "scene";
    world: null | string;
  };
  ch_worlds_storage_quota_opened: {
    world: null | string;
  };
  ch_worlds_storage_quota_retry: {
    world: null | string;
  };
  ch_worlds_storage_value_delete_failed: {
    error: string;
    key: string;
    namespace: "env" | "players" | "scene";
    world: null | string;
  };
  ch_worlds_storage_value_deleted: {
    key: string;
    namespace: "env" | "players" | "scene";
    reason?: string;
    stub?: true;
    world: null | string;
  };
  ch_worlds_storage_value_save_failed: {
    error: string;
    key: null | string;
    mode: string;
    namespace: "env" | "players" | "scene";
    world: null | string;
  };
  ch_worlds_storage_value_saved: {
    key: null | string;
    mode: string;
    namespace: "env" | "players" | "scene";
    reason?: string;
    stub?: true;
    world: null | string;
  };
  ch_worlds_storage_viewed: {
    fallback: boolean;
    lands: number;
    namespace: "env" | "players" | "scene";
    source: "empty" | "live";
    step: "clear" | "edit" | "scene" | "select";
    world: null | string;
    worlds: number;
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
  creator_dashboard_viewed: {
    on_sale_items: null | number;
    published_collections: null | number;
    sales_7d: null | number;
    scene_visits_30d: null | number;
    window_days: number;
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
