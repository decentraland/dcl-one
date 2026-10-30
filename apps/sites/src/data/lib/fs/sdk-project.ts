import { mergeSceneComposites, sameSceneComposite } from "./composite-merge";

export interface SdkProject {
  version: 1;
  name: string;
  scene: Record<string, unknown>;
  compositePath: string | null;
  capabilities: { files: boolean; write: boolean; watch: boolean; dataLayer: boolean; mcp: boolean };
  links: { files: string; file: string; reload: string; preview: string; settings: string; publish: string; storage: string; inspector: string | null; uiDesigner?: string | null; uiDesignerRuntime?: string | null; dataLayer: string | null; mcp: string | null };
}

export interface ProjectFile { path: string; content: string; revision: string }
export interface SdkDebugDescriptor {
  nativeUrl: string;
  multiInstanceUrl: string;
  mobileUrl: string | null;
  mobileQr: string | null;
  eventsUrl: string;
  sessionsUrl: string;
  commandUrl: string;
}
export interface SdkDebugSession {
  id: number;
  sessionId: string | null;
  deviceName: string | null;
  connectedAt: string;
  disconnectedAt: string | null;
  status: "active" | "ended";
  messageCount: number;
}
export type SdkDebugEvent = { type: "sessions"; sessions: SdkDebugSession[] }
  | { type: "entries"; sessionId: number; entries: unknown[]; seq: number }
  | { type: "gap"; missed: number };
type AssistantConversation = { id: string; provider: string; title: string; updatedAt: number; truncated: boolean };
type AssistantSelection = { id: string; name: string };
type AssistantHistoryEvent = AssistantEvent | { type: "user"; text: string; selectedEntities: AssistantSelection[] };
type AssistantEvent = { type: "started"; turnId: string; provider: string; conversationId?: string } | { type: "session"; sessionId: string } | { type: "text"; text: string } | { type: "tool"; name: string; detail: string } | { type: "error"; message: string } | { type: "done"; exitCode: number | null; cancelled: boolean };

async function readJsonLines<T>(body: NonNullable<Response["body"]>, onEvent: (event: T) => void): Promise<void> {
  const reader = body.pipeThrough(new TextDecoderStream()).getReader();
  let pending = "";
  const accept = (line: string) => { if (line.trim()) onEvent(JSON.parse(line) as T); };
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      pending += value;
      let index: number;
      while ((index = pending.indexOf("\n")) >= 0) { accept(pending.slice(0, index)); pending = pending.slice(index + 1); }
    }
    accept(pending);
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
}

export function sdkProjectUrl(value: string): string {
  const url = new URL(value.trim());
  const loopback = url.hostname === "localhost" || url.hostname === "127.0.0.1" || url.hostname === "[::1]";
  if ((url.protocol !== "https:" && !(url.protocol === "http:" && loopback)) || url.username || url.password || url.search || url.hash) {
    throw new Error("Use the SDK project URL: HTTPS, or HTTP on localhost, without credentials or query parameters.");
  }
  return url.href.replace(/\/+$/, "");
}

/** The SDK run's capability token from a page's location hash (`#token=…`, as the link dcl-scene-cli
    prints carries it), and the fragment without it. The token never rides a query string: it is read
    here, kept in memory and put nowhere else. */
export function sdkTokenFromHash(hash: string): { token: string | null; rest: string } {
  const raw = hash.replace(/^#/, "");
  if (!raw) return { token: null, rest: "" };
  const params = new URLSearchParams(raw);
  const token = params.get("token")?.trim() || null;
  if (!token) return { token: null, rest: raw };
  params.delete("token");
  return { token, rest: params.toString() };
}

/** Tokens claimed from a page fragment, by project URL. The fragment leaves the URL bar as soon
    as it is read, so a remount of the editor route (Exit, then the browser's Back; any in-app
    navigation away and back) finds no hash: this is where it looks next. Memory only, never
    storage, gone with the tab. */
const claimedTokens = new Map<string, string>();

export function rememberSdkToken(projectUrl: string, token: string): void {
  try { claimedTokens.set(sdkProjectUrl(projectUrl), token); } catch {}
}

export function recallSdkToken(projectUrl: string): string | null {
  try { return claimedTokens.get(sdkProjectUrl(projectUrl)) ?? null; } catch { return null; }
}

/** Forgets every claimed token (tests). */
export function forgetSdkTokens(): void {
  claimedTokens.clear();
}

const TOKEN_HINT = "The SDK did not accept this page's token. Open the editor from the link dcl-scene-cli start prints: it carries the token this run expects.";

/** The error for a non-ok SDK response. A 401 is the same thing on every route (the run's token is
    missing or stale) and gets the same pointer at the link that carries it, ahead of whatever the
    route would otherwise read into the status; `detail` gets the response body for anything else. */
async function sdkFailure(response: Response, detail: (body: string) => string): Promise<Error> {
  if (response.status === 401) return new Error(TOKEN_HINT);
  return new Error(detail(await response.text().catch(() => "")));
}

export class SdkProjectConnection {
  readonly url: string;
  private revisions = new Map<string, string>();
  private contents = new Map<string, string>();
  private fileOperations = new Map<string, Promise<unknown>>();
  private descriptor: SdkProject | null = null;
  /** `Authorization: Bearer` for every project request once a token is known; bare requests keep an
      older SDK answering reads. */
  private readonly auth: Record<string, string>;
  constructor(url: string, private request: typeof fetch = (...args) => fetch(...args), private readonly token: string | null = null) {
    this.url = sdkProjectUrl(url);
    this.auth = token ? { authorization: `Bearer ${token}` } : {};
  }

  createFileSession(): SdkProjectConnection {
    const session = new SdkProjectConnection(this.url, this.request, this.token);
    session.descriptor = this.descriptor;
    return session;
  }

  private async json<T>(path: string, init?: RequestInit): Promise<T> {
    let response: Response;
    try {
      response = await this.request(`${this.url}${path}`, {
        ...init, headers: { ...this.auth, ...init?.headers }, credentials: "omit", cache: "no-store", signal: init?.signal ?? AbortSignal.timeout(15000),
      });
    } catch (error) {
      throw new Error(`Could not reach the SDK project. Keep dcl-scene-cli start running and allow this site's origin with DCL_SCENE_CLI_ALLOWED_ORIGINS. ${error instanceof Error ? error.message : ""}`);
    }
    if (!response.ok) {
      if (response.status === 409 && path.startsWith("/api/project/file?")) throw new Error("This file changed outside the editor. Reopen it before saving your changes.");
      throw await sdkFailure(response, (body) => `SDK project request failed (${response.status}). ${body}`);
    }
    return response.json() as Promise<T>;
  }

  async connect(signal?: AbortSignal): Promise<SdkProject> {
    const project = await this.json<SdkProject>("/api/project", { signal });
    if (project.version !== 1 || typeof project.name !== "string" || !project.scene || !project.capabilities?.files || !project.links) {
      throw new Error("This server does not expose the SDK project bridge. Update dcl-scene-cli and restart the project.");
    }
    this.descriptor = project;
    return project;
  }

  async list(): Promise<string[]> {
    const result = await this.json<{ files: { path: string; size: number }[] }>("/api/project/files");
    return result.files.map((file) => file.path);
  }

  async read(path: string): Promise<string> {
    return this.fileOperation(path, async () => {
      const file = await this.json<ProjectFile>(`/api/project/file?path=${encodeURIComponent(path)}`);
      this.revisions.set(path, file.revision);
      this.contents.set(path, file.content);
      return file.content;
    });
  }

  private fileOperation<T>(path: string, operation: () => Promise<T>): Promise<T> {
    const pending = (this.fileOperations.get(path) ?? Promise.resolve()).catch(() => {}).then(operation);
    this.fileOperations.set(path, pending);
    const clear = () => { if (this.fileOperations.get(path) === pending) this.fileOperations.delete(path); };
    void pending.then(clear, clear);
    return pending;
  }

  async readOnly(path: string): Promise<string> {
    return (await this.json<ProjectFile>(`/api/project/file?path=${encodeURIComponent(path)}`)).content;
  }

  async write(path: string, content: string, signal?: AbortSignal): Promise<void> {
    return this.fileOperation(path, async () => {
      signal?.throwIfAborted();
      const file = await this.json<ProjectFile>(`/api/project/file?path=${encodeURIComponent(path)}`, {
        method: "PUT", headers: { "content-type": "application/json" },
        body: JSON.stringify({ content, revision: this.revisions.get(path) ?? null }),
        ...(signal ? { signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]) } : {}),
      });
      this.revisions.set(path, file.revision);
      this.contents.set(path, file.content);
    });
  }

  async saveComposite(path: string, content: string, signal?: AbortSignal): Promise<{ content: string; merged: boolean }> {
    return this.fileOperation(path, async () => {
      const baseline = this.contents.get(path);
      for (let attempt = 0; attempt < 4; attempt++) {
        signal?.throwIfAborted();
        const response = await this.request(`${this.url}/api/project/file?path=${encodeURIComponent(path)}`, {
          headers: this.auth, credentials: "omit", cache: "no-store", signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(15000)]) : AbortSignal.timeout(15000),
        });
        if (!response.ok && response.status !== 404) throw await sdkFailure(response, body => `Could not read the shared scene (${response.status}). ${body}`);
        const current = response.status === 404 ? null : await response.json() as ProjectFile;
        if (current && baseline === undefined) throw new Error("The scene was created by another editor. Reopen the saved scene before saving your changes.");
        if (!current && baseline !== undefined) throw new Error("The scene was deleted outside the editor. Reopen the project before saving.");
        const next = current && baseline !== undefined ? mergeSceneComposites(baseline, content, current.content) : content;
        const saved = await this.request(`${this.url}/api/project/file?path=${encodeURIComponent(path)}`, {
          method: "PUT", headers: { ...this.auth, "content-type": "application/json" }, credentials: "omit",
          body: JSON.stringify({ content: next, revision: current?.revision ?? null }),
          signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(15000)]) : AbortSignal.timeout(15000),
        });
        if (saved.status === 409) continue;
        if (!saved.ok) throw await sdkFailure(saved, body => `Could not save the shared scene (${saved.status}). ${body}`);
        const file = await saved.json() as ProjectFile;
        this.revisions.set(path, file.revision);
        // This editor still owns the pre-merge view until its engine acknowledges
        // hydration. Keeping that view as its base prevents a later save from
        // treating another editor's newly merged entities as local deletions.
        this.contents.set(path, content);
        return { content: file.content, merged: !sameSceneComposite(file.content, content) };
      }
      throw new Error("Other editors are still saving this scene. Your changes have not been discarded; try Save again.");
    });
  }

  async sharedComposite(path: string, local?: string, signal?: AbortSignal): Promise<{ content: string; baseline: string }> {
    const remote = await this.json<ProjectFile>(`/api/project/file?path=${encodeURIComponent(path)}`, { signal });
    const baseline = this.contents.get(path);
    if (local !== undefined && baseline === undefined) throw new Error("Reopen the saved scene before merging shared changes.");
    return { content: local === undefined ? remote.content : mergeSceneComposites(baseline!, local, remote.content), baseline: remote.content };
  }

  acknowledgeComposite(path: string, baseline: string): void {
    this.contents.set(path, baseline);
  }

  async remove(path: string): Promise<void> {
    return this.fileOperation(path, async () => {
      const revision = this.revisions.get(path);
      if (!revision) throw new Error("Read the source file before removing it.");
      await this.json(`/api/project/file?path=${encodeURIComponent(path)}`, { method: "DELETE", headers: { "if-match": revision } });
      this.revisions.delete(path);
    });
  }

  get uiDesigner(): { url: string; runtimeUrl: string } | undefined {
    const { uiDesigner, uiDesignerRuntime } = this.descriptor?.links ?? {};
    if (!uiDesigner || !uiDesignerRuntime) return undefined;
    const url = new URL(uiDesigner, `${this.url}/`), runtime = new URL(uiDesignerRuntime, `${this.url}/`);
    if (url.origin !== new URL(this.url).origin || runtime.origin !== url.origin) throw new Error("The SDK returned a UI Designer on another server.");
    return { url: url.href, runtimeUrl: runtime.href };
  }

  link(name: "settings" | "publish" | "storage"): string {
    if (!this.descriptor) throw new Error("Connect to the SDK project first.");
    const url = new URL(this.descriptor.links[name], `${this.url}/`);
    if (url.origin !== new URL(this.url).origin) throw new Error("The SDK returned a link to another server.");
    return url.href;
  }

  /** The data-layer websocket, with the token as `?token=` on the upgrade URL: a browser WebSocket
      carries no headers, and this URL stays on the loopback link, never in a shareable page URL. */
  get dataLayerUrl(): string | null {
    if (!this.descriptor) throw new Error("Connect to the SDK project first.");
    if (!this.descriptor.links.dataLayer) return null;
    const url = new URL(this.descriptor.links.dataLayer, `${this.url}/`);
    if (url.host !== new URL(this.url).host) throw new Error("The SDK returned a data layer on another server.");
    if (this.token) url.searchParams.set("token", this.token);
    return url.href;
  }

  readonly assets = {
    revision: async (path: string): Promise<string> => {
      const response = await this.request(`${this.url}/api/project/asset?path=${encodeURIComponent(path)}`, { method: "HEAD", headers: this.auth, credentials: "omit", cache: "no-store", signal: AbortSignal.timeout(15000) });
      const revision = response.headers.get("etag");
      if (!response.ok || !revision) throw await sdkFailure(response, () => `Could not inspect ${path} (${response.status}).`);
      return revision;
    },
    list: async (): Promise<{ path: string; size: number }[]> => {
      return (await this.json<{ files: { path: string; size: number }[] }>("/api/project/assets")).files;
    },
    read: async (path: string): Promise<{ content: ArrayBuffer; revision: string }> => {
      const response = await this.request(`${this.url}/api/project/asset?path=${encodeURIComponent(path)}`, { headers: this.auth, credentials: "omit", cache: "no-store", signal: AbortSignal.timeout(30000) });
      if (!response.ok) throw await sdkFailure(response, () => `Could not read ${path} (${response.status}).`);
      const revision = response.headers.get("etag");
      if (!revision) throw new Error(`The SDK did not provide a revision for ${path}.`);
      return { content: await response.arrayBuffer(), revision };
    },
    write: async (path: string, content: ArrayBuffer, revision?: string): Promise<void> => {
      const response = await this.request(`${this.url}/api/project/asset?path=${encodeURIComponent(path)}`, {
        method: "PUT", credentials: "omit", signal: AbortSignal.timeout(30000),
        headers: { ...this.auth, "content-type": "application/octet-stream", ...(revision ? { "if-match": revision } : { "if-none-match": "*" }) }, body: content,
      });
      if (!response.ok) throw await sdkFailure(response, (body) => `Could not save ${path} (${response.status}). ${body}`);
    },
    remove: async (path: string, revision: string): Promise<void> => {
      const response = await this.request(`${this.url}/api/project/asset?path=${encodeURIComponent(path)}`, {
        method: "DELETE", credentials: "omit", signal: AbortSignal.timeout(15000), headers: { ...this.auth, "if-match": revision },
      });
      if (!response.ok) throw await sdkFailure(response, () => `Could not remove ${path} (${response.status}). Refresh the asset list before trying again.`);
    },
  };

  readonly assistant = {
    conversations: () => this.json<{ conversations: AssistantConversation[] }>("/api/project/assistant/conversations"),
    conversation: (id: string) => this.json<AssistantConversation & { events: AssistantHistoryEvent[]; resumable: boolean }>(`/api/project/assistant/conversations/${encodeURIComponent(id)}`),
    deleteConversation: async (id: string): Promise<void> => { await this.json(`/api/project/assistant/conversations/${encodeURIComponent(id)}`, { method: "DELETE" }); },
    providers: () => this.json<{ providers: { id: string; label: string; available: boolean }[]; sceneTools: { available: boolean; url: string | null; paired?: boolean; bridge?: string | null }; busy: boolean }>("/api/project/assistant/providers"),
    turn: async (input: { provider: string; prompt: string; conversationId?: string; selectedEntities?: AssistantSelection[] }, onEvent: (event: AssistantEvent) => void, signal: AbortSignal): Promise<void> => {
      const response = await this.request(`${this.url}/api/project/assistant/turn`, {
        method: "POST", credentials: "omit", signal, headers: { ...this.auth, "content-type": "application/json" }, body: JSON.stringify(input),
      });
      if (!response.ok) throw await sdkFailure(response, (body) => `Assistant request failed (${response.status}): ${body}`);
      if (!response.body) throw new Error(`Assistant request failed (${response.status}): empty response`);
      let completed = false;
      await readJsonLines<AssistantEvent>(response.body, (event) => {
        if (event.type === "done") completed = true;
        onEvent(event);
      });
      if (!completed) throw new Error("The assistant disconnected before completing the turn. Check the SDK and try again.");
    },
    cancel: async (turnId: string): Promise<void> => {
      await this.json(`/api/project/assistant/turn/${encodeURIComponent(turnId)}`, { method: "DELETE" });
    },
  };

  readonly debug = {
    descriptor: async (): Promise<SdkDebugDescriptor> => {
      const descriptor = await this.json<SdkDebugDescriptor>("/api/project/debug");
      for (const url of [descriptor.nativeUrl, descriptor.multiInstanceUrl, descriptor.mobileUrl]) {
        if (url !== null && new URL(url).protocol !== "decentraland:") throw new Error("The SDK returned an unsupported device preview link.");
      }
      if (descriptor.mobileQr && !descriptor.mobileQr.startsWith("data:image/svg+xml")) throw new Error("The SDK returned an unsupported mobile QR image.");
      return descriptor;
    },
    sessions: () => this.json<{ sessions: SdkDebugSession[] }>("/api/project/debug/sessions"),
    command: async (sessionId: number, cmd: "pause" | "resume" | "reload_scene"): Promise<void> => {
      const result = await this.json<{ ok: boolean; data?: { error?: string } }>("/api/project/debug/command", {
        method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ sessionId, cmd }),
      });
      if (!result.ok) throw new Error(result.data?.error || "The device did not acknowledge the command.");
    },
    events: async (onEvent: (event: SdkDebugEvent) => void, signal: AbortSignal): Promise<void> => {
      const response = await this.request(`${this.url}/api/project/debug/events`, { headers: this.auth, credentials: "omit", cache: "no-store", signal });
      if (!response.ok || !response.body) throw await sdkFailure(response, () => `Device logs could not connect (${response.status}).`);
      await readJsonLines(response.body, onEvent);
      throw new Error("Device logs disconnected. Reconnect to continue.");
    },
  };
}
