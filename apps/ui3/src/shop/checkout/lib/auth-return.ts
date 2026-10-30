// from: shop@94584a5c:app/src/lib/auth-return.ts
// A gated action a signed-out buyer started (cart checkout, buy now), finished for them after sign-in.
// Sign-in may leave the page, so the intent rides in sessionStorage, is consumed once, and is dropped
// when older than fifteen minutes so it can never resurrect a later, unrelated sign-in.

const RESUME_KEY = "shop:resume_after_signin";
const MAX_AGE_MS = 15 * 60 * 1000;

export type ResumeIntent = { type: "cart-checkout" } | { type: "item-buy"; path: string };

type StoredIntent = ResumeIntent & { at: number };

function store(): Storage | null {
  try {
    return typeof sessionStorage !== "undefined" ? sessionStorage : null;
  } catch {
    return null;
  }
}

export function stashResumeIntent(intent: ResumeIntent, now: number = Date.now()): void {
  try {
    store()?.setItem(RESUME_KEY, JSON.stringify({ ...intent, at: now }));
  } catch {
    // Private mode or storage full: the resume just won't auto-trigger.
  }
}

/** Read-and-clear the pending intent; only a fresh one of `type` comes back. */
export function takeResumeIntent<T extends ResumeIntent["type"]>(type: T, now: number = Date.now()): Extract<ResumeIntent, { type: T }> | null {
  const s = store();
  let raw: string | null = null;
  try {
    raw = s?.getItem(RESUME_KEY) ?? null;
    if (raw !== null) s?.removeItem(RESUME_KEY);
  } catch {
    return null;
  }
  if (!raw) return null;
  let parsed: StoredIntent | null = null;
  try {
    parsed = JSON.parse(raw) as StoredIntent;
  } catch {
    return null;
  }
  if (!parsed || parsed.type !== type || typeof parsed.at !== "number" || now - parsed.at > MAX_AGE_MS) return null;
  if (parsed.type === "item-buy" && (typeof parsed.path !== "string" || !parsed.path.startsWith("/") || parsed.path.startsWith("//"))) return null;
  const intent: ResumeIntent = parsed.type === "cart-checkout" ? { type: "cart-checkout" } : { type: "item-buy", path: parsed.path };
  return intent as Extract<ResumeIntent, { type: T }>;
}
