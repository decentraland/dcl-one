/** Authored engine snapshots only: never store the running preview's state. */
export interface SceneCheckpoint {
  version: 1;
  key: string;
  input: string | null;
  composite: string;
  savedAt: number;
}

export interface SceneCheckpointStore {
  read(key: string): Promise<SceneCheckpoint | null>;
  write(checkpoint: SceneCheckpoint, current?: () => boolean): Promise<void>;
}

const DATABASE = "dcl-editor-recovery-v1";
const STORE = "checkpoints";

export function validSceneComposite(value: unknown): value is string {
  if (typeof value !== "string") return false;
  try {
    const parsed: unknown = JSON.parse(value);
    return !!parsed && typeof parsed === "object" && Array.isArray((parsed as { components?: unknown }).components);
  } catch { return false; }
}

export function matchingCheckpoint(value: unknown, key: string, input: string | null, normalizeInput: (input: string | null) => string | null = value => value): SceneCheckpoint | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Partial<SceneCheckpoint>;
  return record.version === 1 && record.key === key &&
    (record.input === null || typeof record.input === "string") &&
    normalizeInput(record.input) === normalizeInput(input) &&
    typeof record.savedAt === "number" && Number.isFinite(record.savedAt) && validSceneComposite(record.composite)
    ? record as SceneCheckpoint : null;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    if (typeof indexedDB === "undefined") return reject(new Error("Browser recovery storage is unavailable"));
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE, { keyPath: "key" });
    request.onerror = () => reject(request.error ?? new Error("Could not open recovery storage"));
    request.onblocked = () => reject(new Error("Recovery storage is blocked by another editor tab"));
    request.onsuccess = () => resolve(request.result);
  });
}

export const sceneCheckpointStore: SceneCheckpointStore = {
  async read(key) {
    const database = await openDatabase();
    try {
      return await new Promise<SceneCheckpoint | null>((resolve, reject) => {
        const transaction = database.transaction(STORE, "readonly");
        const request = transaction.objectStore(STORE).get(key);
        transaction.oncomplete = () => resolve(request.result ?? null);
        transaction.onerror = transaction.onabort = () => reject(transaction.error ?? new Error("Could not read the recovery checkpoint"));
      });
    } finally { database.close(); }
  },
  async write(checkpoint, current = () => true) {
    const database = await openDatabase();
    try {
      if (!current()) return;
      await new Promise<void>((resolve, reject) => {
        const transaction = database.transaction(STORE, "readwrite");
        transaction.objectStore(STORE).put(checkpoint);
        // A request succeeding does not prove the transaction was committed.
        transaction.oncomplete = () => resolve();
        transaction.onerror = transaction.onabort = () => reject(transaction.error ?? new Error("Could not save the recovery checkpoint"));
      });
    } finally { database.close(); }
  },
};

/** Serializes committed snapshots. Retired generations cannot overwrite newer work. */
export function createSceneRecovery(key: string, input: string | null, store = sceneCheckpointStore, previousKey?: string | null, active: () => boolean = () => true, normalizeInput: (input: string | null) => string | null = value => value) {
  let retired = false;
  let latest: SceneCheckpoint | null = null;
  let durableComposite: string | null = null;
  let tail = Promise.resolve();
  const loaded = store.read(key).then(async value => {
    if (value === null && previousKey && previousKey !== key) {
      const previous = matchingCheckpoint(await store.read(previousKey), previousKey, input, normalizeInput);
      if (previous && !retired && active()) {
        value = { ...previous, key, input };
        latest = value;
        // Copy the recovery lineage on every reload, even if no further edits occur.
        // Duplicated tabs fork into different keys and never share mutable snapshots.
        await store.write(value, () => !retired && active());
      }
    }
    if (!retired && active()) {
      latest = matchingCheckpoint(value, key, input, normalizeInput);
      durableComposite = latest?.composite ?? null;
    }
  });
  // Callers still observe read failures through restore/checkpoint; no unhandled rejection during boot.
  void loaded.catch(() => {});
  return {
    async restore(): Promise<string | null> {
      // A reconnect may race a slow durable write while a newer acknowledged
      // export is queued behind it. Drain the queue present at reconnect time,
      // including failed writes' in-memory fallback, before choosing a snapshot.
      await tail;
      try { await loaded; } catch (error) { if (!latest) throw error; }
      return retired || !active() ? input : latest?.composite ?? input;
    },
    checkpoint(composite: unknown, current: () => boolean = () => true, validated: Promise<boolean> | boolean = true): Promise<boolean> {
      if (!validSceneComposite(composite)) return Promise.reject(new Error("The engine returned an invalid recovery snapshot"));
      const next = tail.catch(() => {}).then(async () => {
        if (!await validated) return false;
        await loaded.catch(() => {});
        if (retired || !active() || !current()) return false;
        if (durableComposite === composite) return true;
        const record: SceneCheckpoint = { version: 1, key, input, composite, savedAt: Date.now() };
        // Keep an acknowledged in-memory snapshot even if persistent storage is full.
        latest = record;
        await store.write(record, () => !retired && active() && current());
        durableComposite = composite;
        return true;
      });
      tail = next.then(() => {}, () => {});
      return next;
    },
    get recovered() { return latest !== null && latest.composite !== input; },
    get durable() { return latest === null || latest.composite === durableComposite; },
    retire() { retired = true; },
  };
}
