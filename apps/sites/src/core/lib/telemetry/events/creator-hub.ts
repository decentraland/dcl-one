export type CreatorHubEvents = {
  bd_create_collection_completed: {
    collection_id?: string;
    contract_address?: string;
    count: number;
    stub: boolean;
    type: "linked" | "standard";
  };
  bd_create_collection_items_added: {
    count: number;
  };
  bd_create_collection_named: {
    name: string;
  };
  bd_create_collection_review_reached: {
    cost_mana: number;
    count: number;
    type: "linked" | "standard";
  };
  bd_create_collection_started: {
    type: "linked" | "standard";
  };
  bd_create_collection_submitted: {
    cost_mana: number;
    count: number;
    type: "linked" | "standard";
  };
  bd_curation_assigned: {
    id: string;
  };
  bd_curation_comment_added: {
    decision?: "approved" | "rejected";
    has_comment: true;
    id?: string;
    length: number;
    stub: true;
    topic_id: null | number;
  };
  bd_curation_decided: {
    has_comment: boolean;
    id?: string;
    status?: "approved" | "rejected";
    stub: true;
    updated?: number;
  };
  bd_curation_filtered: {
    assignee: string;
    status: "ALL_STATUS" | "approved" | "rejected" | "to_review" | "under_review";
    type: "ALL_TYPES" | "standard" | "third_party";
  };
  bd_curation_review_opened: {
    id: string;
  };
  bd_curation_viewed: {
    count: number;
  };
  bd_item_category_set: {
    category: string;
    item: string;
  };
  bd_item_editor_opened: {
    collection: string;
    item: string;
  };
  bd_item_model_set: {
    item: string;
    model: string;
  };
  bd_item_price_set: {
    free: boolean;
    item: string;
    price: string;
  };
  bd_item_rarity_set: {
    item: string;
    max_supply: number;
    rarity: string;
  };
  bd_item_reverted: {
    item: string;
  };
  bd_item_saved: {
    item: string;
    price: string;
    rarity: string;
    stub: boolean;
    urn?: string;
  };
  bd_publish_collection_cost_shown: {
    mana: number;
  };
  bd_publish_collection_started: {
    id: string;
    itemCount: number;
  };
  bd_publish_collection_terms_accepted: Record<string, never>;
  bd_publish_fee_paid: {
    mana: number;
    simulated: boolean;
    tx_hash?: string;
  };
  bd_publish_submitted: {
    id: string;
    itemCount: number;
    mana?: number;
    stub: boolean;
  };
  ch_curate_signin_clicked: Record<string, never>;
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
  creator_collection_detail_viewed: {
    id: string;
    itemCount: number;
    missing: boolean;
    source: "catalyst" | "empty";
    tab: "emotes" | "wearables";
  };
  creator_collection_item_clicked: {
    id: string;
    itemId: string;
    kind: string;
    tab: "emotes" | "wearables";
  };
  creator_collection_published: {
    funnel_step: number;
    source_event?: string;
  };
  creator_collection_tab_changed: {
    tab: "emotes" | "wearables";
  };
  creator_collection_viewed: {
    funnel_step: number;
    source_event?: string;
  };
  creator_curation_submitted: {
    funnel_step: number;
    source_event?: string;
  };
  creator_item_detail_viewed: {
    fallback: boolean;
    id: string;
    type: string;
  };
  creator_item_edit_clicked: {
    id: string;
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
  creator_wearables_card_clicked: {
    id: string;
    kind: string;
  };
  creator_wearables_home_viewed: {
    count: number;
  };
  creator_wearables_signin_clicked: Record<string, never>;
  creator_wearables_view_changed: {
    view: "grid" | "list";
  };
};
