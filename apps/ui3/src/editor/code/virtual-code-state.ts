interface ScopedFiles {
  anonymous: string;
  views: Map<string, Map<string, string>>;
}

// A workspace may retain its file store while switching projects. Keep saved
// virtual baselines separate too, not just Monaco's document-global models.
const scopes = new WeakMap<Map<string, string>, ScopedFiles>();

export function virtualCodeState(store?: Map<string, string>, namespace?: string) {
  const owner = store ?? new Map<string, string>();
  let scoped = scopes.get(owner);
  if (!scoped) {
    const anonymous = crypto.randomUUID();
    scoped = { anonymous, views: new Map([[namespace ?? anonymous, owner]]) };
    scopes.set(owner, scoped);
  }
  const identity = namespace ?? scoped.anonymous;
  let files = scoped.views.get(identity);
  if (!files) { files = new Map(); scoped.views.set(identity, files); }
  const encoded = Array.from(new TextEncoder().encode(identity), byte => byte.toString(16).padStart(2, "0")).join("");
  return { files, modelRoot: `browser/${encoded}/` };
}
