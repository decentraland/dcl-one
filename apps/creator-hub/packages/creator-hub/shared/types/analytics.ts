import type { FeedbackDelivery, FeedbackSource } from './feedback';

export type Events = {
  'Open Editor': {
    version: string;
  };
  'Install Creator Hub': {
    version: string;
  };
  'Update Creator Hub': {
    version: string;
    previous_version: string;
  };
  'Scene created': {
    projectType: 'github-repo';
    url: string;
    project_id: string;
  };
  'Create Project': {
    project_id: string;
    project_name: string;
    template: string;
    rows: number;
    cols: number;
  };
  'Open Project': {
    project_id: string;
    project_name: string;
  };
  'Save Project Success': {
    project_id: string;
    project_name: string;
  };
  'Preview Scene': {
    project_id: string;
  };
  'Publish Scene': {
    project_id: string;
    target: string;
    targetContent: string;
  };
  'Execute Scene Deployment': {
    project_id: string;
    scene_size: number;
    file_count: number;
    base_parcel: string;
    parcels: string[];
    is_world: boolean;
    world_name?: string;
    existing_world_scenes: Array<{
      entity_id: string;
      parcels: string[];
      size: number;
    }>;
  };
  'Open Code': undefined;
  'Worlds Your Storage Modal Action': {
    action: string;
  };
  'Manage Worlds External Action': {
    action: string;
  };
  'Add World Permissions': {
    world_name: string;
    wallet_address: string;
    permission_name: string;
  };
  'Remove World Permissions': {
    world_name: string;
    wallet_address: string;
    permission_name: string;
  };
  'Add Collaborator Parcel Permissions': {
    world_name: string;
    wallet_address: string;
    permission_name: string;
    parcels_count: number;
  };
  'Remove Collaborator Parcel Permissions': {
    world_name: string;
    wallet_address: string;
    permission_name: string;
    parcels_count: number;
  };
  'Update World Settings': {
    world_name: string;
    world_settings: object;
    changed_fields: {
      title: boolean;
      description: boolean;
      thumbnail: boolean;
      content_rating: boolean;
      categories: boolean;
    };
    scenes_list: Array<{
      entityId: string;
      deployer: string;
      parcels_count: number;
      size: string;
      created_at: string;
    }>;
  };
  'Unpublish World Scene': {
    world_name: string;
    scene_coord: string;
  };
  'Unpublish Entire World': {
    world_name: string;
  };
  'Sign In Action': {
    method: 'deeplink';
  };
  'Sign In Blocked': {
    method: 'deeplink';
    reason: 'offline' | 'max_attempts';
  };
  'Sign In Dapp Opened': {
    method: 'deeplink';
  };
  'Sign In Copy URL Action': {
    method: 'deeplink';
  };
  'Sign In Completed': {
    method: 'deeplink';
  };
  'Sign Out Action': undefined;
  'Use Bevy Renderer': {
    project_id: string;
  };
  'AI Chat Opened': {
    project_id: string;
  };
  'AI Turn Started': {
    project_id: string;
    provider: string;
    model: string;
  };
  'AI Turn Completed': {
    project_id: string;
    provider: string;
    model: string;
    // What the CLI reported running, when it reports it ('default' above says nothing).
    actual_model: string | null;
    ok: boolean;
    cancelled: boolean;
    duration_ms: number;
    tool_count: number;
    mutations: number;
    input_tokens: number | null;
    output_tokens: number | null;
  };
  'Creator Fund Join Clicked': {
    signed_in: boolean;
  };
  'Creator Fund Eligibility Checked': {
    result: 'eligible' | 'ineligible' | 'error';
    reasons?: string[];
  };
  'Creator Fund Terms Accepted': {
    terms_version: number;
    flow: 'join' | 'reaccept';
  };
  'Creator Fund Terms Declined': {
    terms_version: number | null;
    flow: 'join' | 'reaccept';
    via: 'not_now' | 'close' | 'escape';
  };
  'Creator Fund Contact Submitted': {
    ok: boolean;
    status?: number;
  };
  'Creator Fund Joined': {
    terms_version: number;
  };
  'Creator Fund Contact Support': {
    source: 'check_failed' | 'snackbar' | 'menu';
  };
  'Creator Fund Viewed': {
    state: 'signed_out' | 'not_joined' | 'joined' | 'suspended';
  };
  'Creator Fund Read Terms': {
    source: 'landing' | 'snackbar' | 'menu';
  };
  'Creator Fund Program Opened': {
    source: 'menu';
  };
  'Creator Fund Program Rules Opened': {
    source: 'menu';
  };
  'Creator Fund Tab Viewed': {
    tab: 'home' | 'leaderboard' | 'my_places' | 'payouts';
  };
  'Creator Fund Cycle Changed': {
    tab: 'leaderboard' | 'my_places';
    cycle_id: number | 'all';
  };
  'Creator Fund Jump In': {
    rank: number;
    place_id: string;
  };
  'Creator Fund Empty State Action': {
    action: 'create_scene' | 'start_from_template' | 'open_scene';
  };
  'Creator Fund Payout Banner Clicked': {
    step: 'verify_email' | 'add_wallet';
  };
  'Creator Fund Email Verification Started': undefined;
  'Creator Fund Email Code Submitted': {
    result: 'verified' | 'incorrect' | 'expired' | 'locked';
  };
  'Creator Fund Payout Wallet Saved': {
    action: 'add' | 'edit';
    ok: boolean;
  };
  'Creator Fund Transaction Viewed': undefined;
  'Creator Fund Banner Clicked': undefined;
  'Creator Fund Banner Dismissed': undefined;
  'Report Issue Opened': {
    source: FeedbackSource;
  };
  'Report Issue Submitted': {
    source: FeedbackSource;
    result: FeedbackDelivery | 'failed';
  };
  'Report Issue Help Center Clicked': {
    source: FeedbackSource;
  };
  // `error` is the error message with file paths replaced and its length capped
  // (`describeError` in the renderer's analytics store).
  'Publish Scene Failed': {
    project_id: string | null;
    target: string;
    targetContent: string;
    error: string;
  };
  'Execute Scene Deployment Failed': {
    project_id: string | null;
    is_world: boolean | null;
    /** The `DeploymentError` name, such as `DEPLOYMENT_FAILED` or `INVALID_IDENTITY`. */
    error_name: string;
    error: string;
  };
  'Preview Scene Failed': {
    project_id: string | null;
    mobile: boolean;
    error: string;
  };
  'Publish Modal Opened': {
    project_id: string;
    initial_step: string;
    signed_in: boolean;
  };
  'Publish Step Viewed': {
    project_id: string;
    step: string;
  };
  'Publish Modal Closed': {
    project_id: string;
    step: string;
  };
  'Jump In Published Scene': {
    project_id: string;
    is_world: boolean;
    opened_in: 'client' | 'web';
  };
  'Analytics Jump In': {
    place_id: string;
  };
  'Delete Project': {
    project_id: string | null;
    delete_files: boolean;
  };
  'Duplicate Project': {
    project_id: string;
    source_project_id: string | null;
  };
  'Rename Project': {
    project_id: string;
  };
  'Import Project': {
    project_id: string;
  };
  'Template Clicked': {
    template: string;
  };
  'Template Code Viewed': {
    template: string;
  };
  'Templates Filtered': {
    difficulty: string;
  };
  'Preview On Mobile': {
    project_id: string | null;
  };
  'Mobile Preview Connected': {
    devices: number;
  };
  'Install Client Prompt': {
    action: 'shown' | 'download' | 'dismiss';
  };
  'Optimize Scan': {
    project_id: string | null;
    glb_count: number;
    total_bytes: number;
    has_backup: boolean;
  };
  'Optimize Scene': {
    project_id: string | null;
    glbs_processed: number;
    glbs_changed: number;
    glbs_failed: number;
    bytes_before: number;
    bytes_after: number;
  };
  'Optimize Scene Failed': {
    project_id: string | null;
    error: string;
  };
  'Optimize Revert': {
    project_id: string | null;
  };
  'Optimize Tools Installed': {
    project_id: string | null;
  };
  'Optimize Tools Install Failed': {
    project_id: string | null;
    error: string;
  };
  'Session Recording Started': {
    project_id: string;
  };
  'Session Recording Finished': {
    project_id: string;
    reason: 'user' | 'limit' | 'device';
    ended_with: 'stop' | 'send';
    duration_ms: number;
    action_count: number;
    transcribed: boolean;
  };
  'Session Recording Cancelled': {
    project_id: string;
  };
  'Session Recording Failed': {
    project_id: string;
    stage: 'start' | 'stop';
    error: string;
  };
  'AI Sharing Decided': {
    granted: boolean;
  };
  'AI Sharing Data Deleted': undefined;
  'Update Prompt': {
    action: 'shown' | 'install' | 'dismiss';
    version: string;
  };
  // Setting names only: never their values, which can hold a folder path.
  'App Settings Changed': {
    changed: string[];
  };
  'Learn Resource Opened': {
    page: 'learn' | 'docs' | 'videos';
    kind: 'video' | 'doc';
    title: string;
    url: string;
  };
  'Open Tutorial': {
    video_id: string;
    title: string;
  };
  // The matched route pattern, never the path: a path can hold a world name or a place id.
  'Page Viewed': {
    page: string;
    tab: string | null;
  };
  'Transfer LAND Started': {
    land_type: string;
  };
  'Transfer LAND Result': {
    land_type: string;
    result: 'done' | 'unconfirmed' | 'rejected' | 'expired' | 'failed';
  };
};
