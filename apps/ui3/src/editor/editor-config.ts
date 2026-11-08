export const BOOT_TIMEOUT_MS = 75_000;

export const BOOT_PROGRESS_POLL_MS = 400;

export const INIT_ANNOUNCE_INTERVAL_MS = 1_200;

export const RPC_TIMEOUT_MS = 8_000;

export const RESTORE_COMPOSITE_TIMEOUT_MS = 30_000;

export const EXPORT_COMPOSITE_TIMEOUT_MS = 12_000;

export const BOOT_LEAVE_MS = 260;

// The engine acknowledges the edit camera before its loading fade and first
// rendered grid frame have reached the viewport.
export const EDIT_VIEW_SETTLE_MS = 1200;

// Project realm readiness: each wait polls with this backoff until its budget,
// the same budgets the fixed-interval loops had.
export const PROJECT_REALM_BACKOFF = { initialMs: 50, maxMs: 1000, factor: 2 };

export const PROJECT_SERVICE_WORKER_TIMEOUT_MS = 6000;
export const PROJECT_PREPARE_TIMEOUT_MS = 4000;
export const PROJECT_CACHE_SEED_TIMEOUT_MS = 1500;
export const PROJECT_REOPEN_TIMEOUT_MS = 8000;
