import { lazy, Suspense, useEffect, useState } from "react";
import { redirect, useNavigate } from "react-router";

import { recallSdkToken, rememberSdkToken, sdkTokenFromHash } from "@/data/lib/fs/sdk-project";

import { worldsBase } from "@/data/lib/catalyst/client";

import {
  loadSceneEditorSeed,
  newSceneSeed,
  emptySeed,
  seedFromCompositeJSON,
  buildViewportUrl,
  type SceneEditorSeed,
} from "@/data/lib/catalyst/creator-hub/scene-editor";
import type { CatalogItem } from "@/data/lib/catalyst/creator-hub/asset-catalog.server";
import type { SceneEditorLoaderData } from "@/data/lib/catalyst/creator-hub/scene-editor-drafts";
import {
  buildTemplateCompositeText,
  hasTemplateComposite,
} from "@/data/lib/fs/template-composites";
import { type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { pageLoader } from "@/core/lib/telemetry/page-loader";
import { withDeadline } from "@/data/lib/request-deadline";

import { resolveBreadcrumbOrigin } from "@/features/components/creator-hub/breadcrumbOrigins";
import EditorBootShell from "@ui/editor/frames/EditorBootShell";

import { creatorHubMeta } from "@/core/lib/seo/creator-hub-meta";

import type { Route } from "./+types/create.scene-editor";

const EditorWizard = lazy(() => import("@/features/creator-hub/scene-editor-place-items/EditorWizard"));
const SdkEditorWorkspace = lazy(() => import("@/features/creator-hub/scene-editor-place-items/SdkEditorWorkspace"));
// The editor scene Sites serves next to /play (apps/ui3/editor-scene, the route editor-scene.EditorScene.about).
const EDITOR_SCENE_PATH = "/editor-scene/EditorScene";

export const meta = () => creatorHubMeta("Scene editor");

// No cross-origin isolation here: the viewport frames the Explorer's shell (/play/), which its
// service worker keeps unisolated, and the app inside it isolates itself
// (Document-Isolation-Policy). An isolated page can only frame isolated documents.

const FALLBACK: PageDefaults = {
  variant: "guided",
};

// A client navigation reaches the loader as the single-fetch request (`/create/scene-editor.data?…&_routes=…`);
// a redirect has to name the document URL, or the browser lands on the data endpoint.
function documentUrl(request: Request): URL {
  const url = new URL(request.url);
  url.pathname = url.pathname.replace(/\.data$/, "");
  url.searchParams.delete("_routes");
  return url;
}

export async function loader({ request }: Route.LoaderArgs) {
  const url = documentUrl(request);
  // A deliberate New action creates a fresh browser draft even when its
  // title/template match an earlier scene. Keep that identity in the URL so
  // reload/recovery preserve it; saved drafts, folders and SDK links are not rekeyed.
  const freshIntent = url.searchParams.get("new") === "1"
    && !["source", "draft", "project", "pointer", "world"].some(key => url.searchParams.has(key));
  if (freshIntent && !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(url.searchParams.get("newProject") ?? "")) {
    url.searchParams.set("newProject", crypto.randomUUID());
    throw redirect(`${url.pathname}${url.search}`, {
      headers: { "Cache-Control": "private, no-store" },
    });
  }
  const step = url.searchParams.get("step")?.trim() || null;
  const pointer = url.searchParams.get("pointer")?.trim() || undefined;
  const world = url.searchParams.get("world")?.trim() || undefined;
  const isLocal = url.searchParams.get("source") === "local";
  const isNew = url.searchParams.get("new") === "1" || (!isLocal && !pointer);
  const newName = url.searchParams.get("name")?.trim() || undefined;
  const newTemplate = url.searchParams.get("template")?.trim() || undefined;
  const newLayout = url.searchParams.get("layout")?.trim() || undefined;
  const projectSlug = url.searchParams.get("project")?.trim() || undefined;
  const sdkProjectUrl = url.searchParams.get("source") === "sdk" ? url.searchParams.get("projectUrl") || undefined : undefined;
  const from = url.searchParams.get("from")?.trim() || null;

  const [{ sid, assignment, wrap }, { seed, rawComposite }] = await Promise.all([
    pageLoader(request,  FALLBACK),
    (async () => {
      let seed: SceneEditorSeed;
      let rawComposite: string | undefined;
      if (isNew) {
        seed = newSceneSeed({ name: newName, template: newTemplate, layout: newLayout });
        if (newTemplate && hasTemplateComposite(newTemplate)) {
          const text = buildTemplateCompositeText(newTemplate);
          if (text) {
            try {
              seed = seedFromCompositeJSON(JSON.parse(text), seed, []);
              rawComposite = text;
            } catch {
            }
          }
        }
      } else if (isLocal) {
        seed = emptySeed(pointer);
      } else {
        try {
          seed = await withDeadline((signal) => loadSceneEditorSeed({ pointer, world, signal }), 5000, request.signal);
        } catch {
          request.signal.throwIfAborted();
          seed = emptySeed(pointer);
          if (world) seed.scene.pointer = world;
        }
      }
      return { seed, rawComposite };
    })(),
  ]);

  // The editor's super-user scene, where a deployment supplies one. Without it the viewport is
  // the engine alone on the project's preview realm. /play trusts, and opens the editor bus
  // with, only the scene served next to it (apps/ui3/src/play/lib/editorEmbed.ts).
  const configuredEditorScene =
    process.env.BEVY_EDITOR_SCENE_URL ||
    process.env.EDITOR_SYSTEM_SCENE_URL ||
    null;
  const editorSceneUrl =
    configuredEditorScene?.trim().replace(/\/+$/, "") === EDITOR_SCENE_PATH ? EDITOR_SCENE_PATH : null;
  if (configuredEditorScene && !editorSceneUrl) {
    console.warn(`scene editor: ignoring ${configuredEditorScene}; /play runs only ${EDITOR_SCENE_PATH} as the editor scene`);
  }

  const useProjectRealm = isNew || isLocal;
  const projectRealm = process.env.BEVY_PROJECT_REALM_URL || "/_project";
  const publicRealm =
    process.env.REALM_URL ||
    process.env.PUBLIC_CATALYST_URL ||
    url.origin;
  const projectBase = seed.scene.base || "0,0";
  const launch = {
    playUrl: process.env.BEVY_PLAY_URL || "/play",
    realm: useProjectRealm ? projectRealm : world ? `${worldsBase()}/world/${encodeURIComponent(world)}` : publicRealm,
    position: useProjectRealm ? projectBase : seed.scene.base || pointer || "0,0",
    preview: useProjectRealm,
    portables: process.env.BEVY_CONTROLLER_REALM_URL || "",
  };
  const editorSession = crypto.randomUUID();
  const viewportSrc = buildViewportUrl({ ...launch, systemScene: editorSceneUrl, engineOnly: !editorSceneUrl, editorSession });
  const previewSrc = buildViewportUrl(launch);

  const payload = {
    sid,
    step,
    assignment,
    seed,
    viewportSrc,
    previewSrc,
    from,
    projectSlug,
    sdkProjectUrl,
    ...(rawComposite ? { rawComposite } : {}),
  };
  return wrap(payload);
}

export async function clientLoader({ request, serverLoader }: Route.ClientLoaderArgs) {
  const base = serverLoader() as Promise<SceneEditorLoaderData>;
  const url = new URL(request.url);
  if (!url.searchParams.get("draft")?.trim() && url.searchParams.get("source") !== "local") return base;
  const draft = import("@/data/lib/catalyst/creator-hub/scene-editor-drafts")
    .then(({ loadSceneEditorDraft }) => loadSceneEditorDraft(request, base));
  // Observe route failure immediately while the optional chunk loads, and keep
  // signed draft loading concurrent with the server route after it arrives.
  return Promise.all([base, draft]).then(([, restored]) => restored);
}

clientLoader.hydrate = true as const;

export function HydrateFallback() {
  return (
    <main className="creator-hub-scene-editor">
      <EditorBootShell />
    </main>
  );
}

const EDITOR_MIN_WIDTH = 900;

function useBelowEditorWidth(): boolean | null {
  // Resolve the viewport before mounting either lazy editor (including hydration).
  const [narrow, setNarrow] = useState<boolean | null>(null);
  useEffect(() => {
    const mq = window.matchMedia(`(max-width: ${EDITOR_MIN_WIDTH - 1}px)`);
    const sync = () => setNarrow(mq.matches);
    sync();
    mq.addEventListener("change", sync);
    return () => mq.removeEventListener("change", sync);
  }, []);
  return narrow;
}

function SceneEditorDesktopGate({ onExit, onContinue }: { onExit: () => void; onContinue: () => void }) {
  const [copied, setCopied] = useState(false);
  const copyLink = async () => {
    const href = window.location.href;
    try {
      await navigator.clipboard.writeText(href);
      setCopied(true);
      return;
    } catch {
    }
    try {
      const ta = document.createElement("textarea");
      ta.value = href;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
      setCopied(true);
    } catch {
    }
  };
  return (
    <div
      role="status"
      style={{
        minHeight: "100dvh",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: "var(--s-4)",
        padding: "var(--s-6) var(--s-5)",
        textAlign: "center",
        background: "var(--lm-bg, #0d0c11)",
        color: "#fff",
      }}
    >
      <h1 style={{ margin: 0, fontSize: "var(--fs-24)", fontWeight: "var(--fw-bold)" }}>
        Choose how to continue
      </h1>
      <p
        style={{
          margin: 0,
          maxWidth: 420,
          fontSize: "var(--fs-15)",
          lineHeight: 1.5,
          color: "var(--lm-ink-3, rgba(255,255,255,0.7))",
        }}
      >
        The scene editor works best on a larger screen. You can continue in this
        browser or copy the link to keep building on a desktop.
      </p>
      <button
        type="button"
        onClick={copyLink}
        style={{
          marginTop: "var(--s-2)",
          padding: "var(--s-2-5) var(--s-5)",
          borderRadius: 999,
          border: "none",
          background: "var(--brand-cta)",
          color: "#fff",
          fontSize: "var(--fs-15)",
          fontWeight: "var(--fw-semibold)",
          cursor: "pointer",
        }}
      >
        {copied ? "Link copied" : "Copy link for desktop"}
      </button>
      <button type="button" onClick={onContinue} style={{
        padding: "var(--s-2-5) var(--s-5)", borderRadius: 999,
        border: "1px solid rgba(255,255,255,0.3)", background: "transparent",
        color: "#fff", fontSize: "var(--fs-15)", cursor: "pointer",
      }}>Continue in browser</button>
      {copied && (
        <p
          role="status"
          style={{
            margin: 0,
            fontSize: "var(--fs-13)",
            color: "var(--lm-ink-3, rgba(255,255,255,0.7))",
          }}
        >
          Open the copied link in a desktop browser to continue this scene.
        </p>
      )}
      <button
        type="button"
        onClick={onExit}
        style={{
          padding: "var(--s-2-5) var(--s-5)",
          borderRadius: 999,
          border: "1px solid rgba(255,255,255,0.3)",
          background: "transparent",
          color: "#fff",
          fontSize: "var(--fs-15)",
          fontWeight: "var(--fw-semibold)",
          cursor: "pointer",
        }}
      >
        Back to Creator Hub
      </button>
    </div>
  );
}

/** Claims the SDK run token from the page fragment and takes it off the URL bar, so the address
    the person copies or bookmarks carries no write access to their project. The claim is kept in
    module scope by project URL (sdk-project.ts), so a remount of this route without the fragment
    (Exit and then Back, any in-app navigation away and back) finds the token again. */
export function claimSdkToken(projectUrl: string): string | null {
  if (typeof window === "undefined") return null;
  const { token, rest } = sdkTokenFromHash(window.location.hash);
  if (!token) return recallSdkToken(projectUrl);
  rememberSdkToken(projectUrl, token);
  try {
    window.history.replaceState(window.history.state, "", `${window.location.pathname}${window.location.search}${rest ? `#${rest}` : ""}`);
  } catch {}
  return token;
}

export default function CreatorHubSceneEditor({ loaderData }: Route.ComponentProps) {
  const { seed, viewportSrc, previewSrc, rawComposite, draftAssets, from, projectSlug, failedToLoadLocal } =
    loaderData;
  const navigate = useNavigate();
  const [sdkToken] = useState(() => (loaderData.sdkProjectUrl ? claimSdkToken(loaderData.sdkProjectUrl) : null));

  const handlePublish = (_id?: string, draft?: string) => {
    const isCoords = /^-?\d+\s*,\s*-?\d+$/.test(seed.scene.pointer ?? "");
    const isWorld = seed.scene.live && Boolean(seed.scene.pointer) && !isCoords;
    const proj = projectSlug ? `&project=${encodeURIComponent(projectSlug)}` : "";
    const world =
      isWorld && seed.scene.pointer
        ? `&world=${encodeURIComponent(seed.scene.pointer)}`
        : "";
    const draftQ = draft ? `&draft=${encodeURIComponent(draft)}` : "";
    navigate("/create/publish?from=scene-editor" + world + proj + draftQ);
  };

  const handleExit = () => navigate(resolveBreadcrumbOrigin(from ?? null).to);

  const belowEditorWidth = useBelowEditorWidth();
  const [continueInBrowser, setContinueInBrowser] = useState(false);
  useEffect(() => {
    // Once entered, resizing must not unmount an editor containing unsaved work.
    if (belowEditorWidth === false) setContinueInBrowser(true);
  }, [belowEditorWidth]);
  const enabled = belowEditorWidth === false || continueInBrowser;
  const [catalog, setCatalog] = useState<CatalogItem[]>();
  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    void fetch("/api/creator-hub/asset-catalog", { signal: controller.signal })
      .then(response => {
        if (!response.ok) throw new Error("Asset catalog unavailable");
        return response.json() as Promise<CatalogItem[]>;
      })
      .then(items => { if (!controller.signal.aborted) setCatalog(items); })
      // The editor retains its bundled starter catalog if the full catalog is unavailable.
      .catch(() => {});
    return () => controller.abort();
  }, [enabled]);
  if (belowEditorWidth === null) return <HydrateFallback />;
  if (!enabled) {
    return (
      <main className="creator-hub-scene-editor">
        <SceneEditorDesktopGate onExit={handleExit} onContinue={() => setContinueInBrowser(true)} />
      </main>
    );
  }

  if (loaderData.sdkProjectUrl) return <main className="creator-hub-scene-editor">
    <Suspense fallback={<EditorBootShell />}>
      <SdkEditorWorkspace projectUrl={loaderData.sdkProjectUrl} token={sdkToken} viewportSrc={viewportSrc} catalog={catalog} onExit={handleExit} />
    </Suspense>
  </main>;

  return (
    <main className="creator-hub-scene-editor">
      <Suspense fallback={<EditorBootShell />}>
        <EditorWizard
          seed={seed}
          projectSlug={projectSlug}
          viewportSrc={viewportSrc}
          previewSrc={previewSrc}
          rawComposite={rawComposite}
          draftAssets={draftAssets}
          catalog={catalog}
          failedToLoadLocal={failedToLoadLocal}
          from={from}
          onExit={handleExit}
          onPublish={handlePublish}
        />
      </Suspense>
    </main>
  );
}
