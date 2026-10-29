import { useCallback, useEffect, useMemo, useRef, useState, type ComponentProps } from "react";

import { useEditorPageMachine } from "@ui/editor/pages/useEditorPageMachine";
import { editorPageDirty, type EditorPageRequest } from "@ui/editor/page-machine";
import { INITIAL_WORKSPACE, type EditorWorkspaceSnapshot } from "@ui/editor/workspace-lifecycle";
import DeWorkspace from "@ui/editor/pages/DeWorkspace";
import type { CaptureAuthoredScene, DeWorkspaceCode } from "@ui/editor/types";
import DeSceneSettings from "@ui/editor/components/DeSceneSettings";
import { openSceneSettingsFile } from "@/data/lib/fs/scene-settings-file";
import { runEditorEffect } from "@ui/editor/serial-effect";
import { createEditorBus, editorBusChannelFromViewportSrc } from "@ui/editor/editor-bus";
import { editorBusEditsProject } from "@ui/editor/bus-edits";
import { PROJECT_REALM_EFFECT, placedProjectContents } from "@ui/editor/project-cache";
import { currentProjectScope, migrateLegacyProjectAssets, rebaseLegacyProjectUrls, rememberProjectScope, scopeProjectViewport } from "@ui/editor/project-cache-name";
import DeEditorAppBar, { DeEditorControlsBar as Controls } from "@ui/editor/components/DeEditorAppBar";
import { STARTER_TEMPLATES } from "@ui/creatorhub/pages/ChTemplates";
import { type BusEnvelope } from "@ui/generated/editor-bus";

import type {
  HierarchyNode,
  SceneEditorSeed,
} from "@/data/lib/catalyst/creator-hub/scene-editor";
import type { CatalogItem } from "@/data/lib/catalyst/creator-hub/asset-catalog.server";
import { buildScaffoldFiles } from "@/data/lib/fs/scaffold-project";
import { hasTemplateComposite } from "@/data/lib/fs/template-composites";
import type { SdkProjectConnection, SdkProject } from "@/data/lib/fs/sdk-project";
import { useAuth } from "@/data/lib/auth/index";
import {
  NO_COMPOSITE_CODE_ONLY_HINT,
  NO_COMPOSITE_HINT,
  OPEN_FAILED_HINT,
} from "@/data/lib/fs/local-scene";
import {
  handleStore,
  ensureHandlePermission,
} from "@/data/lib/fs/handle-store";

export function sdkFileProject(connection: SdkProjectConnection, beforeWrite: () => Promise<void>): NonNullable<DeWorkspaceCode["project"]> {
  return {
    id: connection.url,
    list: () => connection.list(),
    read: path => connection.read(path),
    readOnly: path => connection.readOnly(path),
    write: async (path, content) => { await beforeWrite(); return connection.write(path, content); },
    remove: async path => { await beforeWrite(); return connection.remove(path); },
    createFileSession: () => sdkFileProject(connection.createFileSession(), beforeWrite),
    assistant: connection.assistant,
    assets: {
      ...connection.assets,
      write: async (path, content, revision) => { await beforeWrite(); return connection.assets.write(path, content, revision); },
      remove: async (path, revision) => { await beforeWrite(); return connection.assets.remove(path, revision); },
    },
    uiDesigner: connection.uiDesigner,
  };
}

export async function captureAuthoredForPersistence(capture: CaptureAuthoredScene | null, signal?: AbortSignal): Promise<string> {
  signal?.throwIfAborted();
  if (!capture) throw new Error("The editor is not ready to preserve your scene. Reconnect and try again.");
  // The SDK can rebuild as soon as its file changes. This must update the live
  // recovery controller, not only an external copy of the checkpoint in IDB.
  const composite = await capture();
  signal?.throwIfAborted();
  return composite;
}
export function editorPersistenceKey(projectSlug: string | undefined, scopeId: string, search = ""): string {
  // Titles are display labels, never authority to open another draft's folder
  // or import its virtual code. Explicit saved-project/draft identities remain
  // compatible; an unsaved browser project retains its own stable scope ID.
  return projectSlug?.trim()
    || new URLSearchParams(search).get("draft")?.trim()
    || `browser-${scopeId}`;
}


type EditorWizardProps = {
  seed: SceneEditorSeed;
  projectSlug?: string;
  onExit?: () => void;
  onPublish?: (id?: string, draft?: string) => void;
  viewportSrc?: string;
  previewSrc?: string;
  rawComposite?: string;
  draftAssets?: Record<string, string>;
  catalog?: CatalogItem[];
  template?: string;
  failedToLoadLocal?: boolean;
  from?: string | null;
  sdkProject?: SdkProjectConnection;
  sdkDescriptor?: SdkProject;
  onDevicePreview?: () => void;
};

type DiskSaveState =
  | { phase: "idle" }
  | { phase: "saving" }
  | {
      phase: "saved";
      via: "fsa-handle" | "download" | "account";
      entities: number;
      filename: string;
      serverSynced?: boolean;
    }
  | { phase: "canceled"; serverSynced?: boolean }
  | { phase: "error"; message: string };

type DiskOpenState =
  | { phase: "idle" }
  | { phase: "picking" | "reading" | "opening" }
  | { phase: "error"; message: string };

export default function EditorWizard(props: EditorWizardProps) {
  const viewportSrc = scopeProjectViewport(props.viewportSrc);
  const previewSrc = scopeProjectViewport(props.previewSrc);
  return <EditorWizardSession key={`${props.projectSlug || props.seed.scene.title}:${viewportSrc ?? ""}:${props.sdkProject?.url ?? ""}`} {...props} viewportSrc={viewportSrc} previewSrc={previewSrc} />;
}

function EditorWizardSession({
  sdkProject,
  sdkDescriptor,
  onDevicePreview,
  seed,
  projectSlug,
  onExit,
  onPublish,
  viewportSrc,
  previewSrc,
  rawComposite,
  draftAssets,
  catalog,
  template,
  failedToLoadLocal,
  from = null,
}: EditorWizardProps) {
  const { isConnected } = useAuth();
  const projectScope = useRef(currentProjectScope()).current;
  const [templateNoticeDismissed, setTemplateNoticeDismissed] = useState(false);
  const [localErrorDismissed, setLocalErrorDismissed] = useState(false);
  const [liveCopyNoteDismissed, setLiveCopyNoteDismissed] = useState(false);
  const [diskSave, setDiskSave] = useState<DiskSaveState>({ phase: "idle" });
  const [diskOpen, setDiskOpen] = useState<DiskOpenState>({ phase: "idle" });
  const [sharedSceneStatus, setSharedSceneStatus] = useState<{ error: boolean; message: string } | null>(null);
  const [externalSceneVersion, setExternalSceneVersion] = useState(0);
  const projectKey = useRef(editorPersistenceKey(projectSlug, projectScope.id,
    typeof window === "undefined" ? "" : new URL(window.location.href).search,
  )).current;
  const operations = useEditorPageMachine(`${projectKey}:${viewportSrc ?? ""}:${sdkProject?.url ?? ""}`);
  const [workspace, setWorkspace] = useState<EditorWorkspaceSnapshot>(INITIAL_WORKSPACE);
  const authoringCapture = useRef<CaptureAuthoredScene | null>(null);
  const onAuthoringCaptureReady = useCallback((capture: CaptureAuthoredScene | null) => {
    authoringCapture.current = capture;
  }, []);
  const activeViewportSrc = workspace.destination ?? viewportSrc;
  const enginePlaying = workspace.playing;
  const operationsBusy = !!operations.state.pending;
  const canOperate = workspace.capabilities.open && !operationsBusy;
  const canSave = canOperate && (!viewportSrc || workspace.capabilities.save);
  const finish = (request: EditorPageRequest, status: "completed" | "cancelled" | "failed", saved = false, error?: string) =>
    operations.send({ type: "finished", request, status, saved, error });
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [engineStatus, setEngineStatus] = useState<"connecting" | "online" | "offline">(
    "connecting",
  );

  const isDirty = () => editorPageDirty(operations.current.current);
  const workspaceRef = useRef(workspace);
  const onLifecycle = useCallback((snapshot: EditorWorkspaceSnapshot) => {
    if (snapshot.ready) rememberProjectScope(projectScope);
    const previous = workspaceRef.current;
    if (snapshot.session !== previous.session || snapshot.destination !== previous.destination || (previous.ready && !snapshot.ready)) {
      const pending = operations.current.current.pending;
      operations.send({ type: "connection-changed" });
      if (pending) {
        const message = "The editor reconnected. Check your scene and try again.";
        if (pending.command === "open") setDiskOpen({ phase: "error", message });
        else setDiskSave({ phase: "error", message });
      }
    }
    workspaceRef.current = snapshot;
    setWorkspace(snapshot);
  }, [operations.send]);
  useEffect(() => {
    if (typeof window === "undefined") return undefined;
    const channel = editorBusChannelFromViewportSrc(activeViewportSrc);
    const ch = !channel || typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel(channel);
    if (ch) ch.onmessage = (ev) => {
      const env = (ev.data ?? null) as BusEnvelope | null;
      if (!env || typeof env !== "object" || !env.msg) return;
      if (editorBusEditsProject(env)) operations.send({ type: "edited" });
    };
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
      if (!isDirty() && !operations.current.current.pending) return;
      e.preventDefault();
      e.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => {
      ch?.close();
      window.removeEventListener("beforeunload", onBeforeUnload);
    };
  }, [activeViewportSrc]);
  const guardedExit = onExit
    ? () => {
        if (operations.current.current.pending) return;
        if (
          isDirty() &&
          typeof window !== "undefined" &&
          !window.confirm("You have unsaved changes. Leave the editor without saving?")
        ) {
          return;
        }
        onExit();
      }
    : undefined;

  const sceneTitleRef = useRef(seed.scene.title);
  const diskCompositeRef = useRef(rawComposite);
  const onSceneNameChange = useCallback((name: string) => { if (sceneTitleRef.current !== name) operations.send({ type: "edited" }); sceneTitleRef.current = name; }, [operations.send]);
  const seedRef = useRef(seed);
  seedRef.current = seed;
  const draftAssetsRef = useRef(draftAssets);
  draftAssetsRef.current = draftAssets;
  const projectAssets = () => ({ ...draftAssetsRef.current, ...placedProjectContents(projectScope) });

  const exportComposite = (request: EditorPageRequest) =>
    captureAuthoredForPersistence(authoringCapture.current, operations.signal(request));
  const persistenceKey = `editor-project:${sdkProject?.url ?? projectKey}`;
  const persistPublishDraft = (request: EditorPageRequest): Promise<string | null> =>
    runEditorEffect(persistenceKey, () => operations.accepts(request), () => writePublishDraft(request));
  const writePublishDraft = async (request: EditorPageRequest): Promise<string | null> => {
    const check = () => { if (!operations.accepts(request)) throw new Error("The editor session changed. Try again."); };
    check();
    const scene = { ...seedRef.current.scene, title: sceneTitleRef.current };
    const slug = projectKey;
    let composite: string | null = null;
    if (viewportSrc) {
      const { requireEngineComposite } = await import("@/data/lib/fs/save-scene");
      check();
      composite = await requireEngineComposite(12000, () => exportComposite(request));
      check();
    }
    composite = composite ?? rawComposite ?? null;
    if (!composite) throw new Error("The scene could not be captured. Reconnect and try again.");
    if (sdkProject) {
      const saved = await sdkProject.saveComposite(sdkDescriptor?.compositePath || "main.composite", composite, operations.signal(request));
      if (saved.merged) setSharedSceneStatus({ error: false, message: "Saved together with the other editor's changes. Sync shared scene to bring them into this viewport." });
      check();
      return null;
    }
    await handleStore.putMeta(slug, {
      title: scene.title,
      base: scene.base,
      template: scene.template,
      composite,
      assets: projectAssets(),
    }, operations.signal(request));
    check();
    try {
      const { pushServerDraft } = await import(
        "@/data/lib/catalyst/creator-hub/scene-drafts-client"
      );
      check();
      await pushServerDraft(slug, {
        composite,
        title: scene.title,
        base: scene.base,
        ...(scene.template ? { template: scene.template } : {}),
        assets: projectAssets(),
      }, operations.signal(request));
    } catch {
    }
    check();
    return slug;
  };

  const guardedPublish = onPublish
    ? (id?: string) => {
        if (!canSave) return;
        if (
          !sdkProject && isDirty() &&
          typeof window !== "undefined" &&
          !window.confirm(
            "You have unsaved changes. Continue to publish? A draft of this scene will be kept so you can come back.",
          )
        ) {
          return;
        }
        const request = operations.begin("publish", canSave);
        if (!request) return;
        void persistPublishDraft(request)
          .then((draft) => {
            if (!operations.accepts(request)) return;
            finish(request, "completed", true);
            onPublish(id, draft ?? undefined);
          })
          .catch((error) => {
            if (!operations.accepts(request)) return;
            const message = error instanceof Error ? error.message : "The scene could not be prepared for publishing.";
            finish(request, "failed", false, message);
            setDiskSave({ phase: "error", message });
          });
      }
    : undefined;

  const catalogItems = (catalog && catalog.length > 0
    ? catalog
    : seed.assetCatalog.models) as unknown as ComponentProps<typeof DeWorkspace>["catalog"];

  const tree = buildHierarchyTree(seed.hierarchy) as unknown as ComponentProps<
    typeof DeWorkspace
  >["tree"];
  const workspaceCode = useMemo(() => {
    if (sdkProject) return {
      typesUrl: "/dcl-sdk-types.json",
      project: sdkFileProject(sdkProject, async () => {
        // A broken scene must remain repairable in the code editor. Only an
        // actively editable runtime needs a fresh capture; Play already saved
        // its authored checkpoint before entering the simulation.
        if (workspaceRef.current.ready && !workspaceRef.current.playing) {
          await captureAuthoredForPersistence(authoringCapture.current);
        }
      }),
    };
    const slug = projectKey;
    return {
      typesUrl: "/dcl-sdk-types.json",
      modelNamespace: projectKey,
      virtualFiles: buildScaffoldFiles({
        name: seed.scene.title,
        template: seed.scene.template,
        parcels: seed.scene.parcels,
      }),
      getDir: async () => {
        try {
          const h = await handleStore.get(slug);
          if (!h) return null;
          return (await ensureHandlePermission(h, "readwrite")) ? h : null;
        } catch {
          return null;
        }
      },
      hydrate: async () => {
        try {
          const meta = await handleStore.getMeta(slug);
          return meta?.codeFiles ?? null;
        } catch {
          return null;
        }
      },
      persist: async (path: string, text: string) => {
        await handleStore.putMeta(slug, {
          title: seed.scene.title,
          base: seed.scene.base,
          ...(seed.scene.template ? { template: seed.scene.template } : {}),
          codeFiles: { [path]: text },
        });
      },
    };
  }, [projectKey, seed.scene.title, seed.scene.base, seed.scene.template, sdkProject]);
  const loadSettings = useCallback(() => openSceneSettingsFile(workspaceCode), [workspaceCode]);
  const localAssets =
    (seed.assetCatalog as { local?: { path: string; folder: string }[] }).local ?? [];
  const selectedNode = seed.hierarchy.find((n) => n.selected) ?? seed.hierarchy[0];
  const inspector = selectedNode
    ? {
        name: selectedNode.name,
        id: String(selectedNode.entity),
        components: selectedNode.components,
        transform: seed.transformIdentity,
      }
    : undefined;

  const templateId = template ?? seed.scene.template;
  const starter = templateId
    ? STARTER_TEMPLATES.find((t) => t.id === templateId)
    : undefined;

  const gameTemplateId = templateId && hasTemplateComposite(templateId) ? templateId : null;
  const prepareRealm = useCallback(async (signal?: AbortSignal) => {
    const scope = projectScope;
    await migrateLegacyProjectAssets(scope, rawComposite, { ...draftAssetsRef.current, ...placedProjectContents(scope) });
    signal?.throwIfAborted();
    // Folder projects keep their own manifest; replacing it with the starter realm would
    // discard their asset map. Import stages them before navigating; a saved one opened
    // from the scenes list, a reload or a link is staged from its stored folder.
    if (!gameTemplateId && new URL(window.location.href).searchParams.get("source") === "local") {
      if (projectSlug) {
        const { stagePersistedLocalProject } = await import("@/data/lib/fs/local-scene");
        await stagePersistedLocalProject(projectSlug, scope, signal);
      }
      return;
    }
    const { populateTemplateRealm, populateProjectRealm } = await import("@/data/lib/fs/project-realm");
    const composite = rawComposite ?? null;
    const res = gameTemplateId ? await populateTemplateRealm({
      scope,
      signal,
      template: gameTemplateId,
      name: seedRef.current.scene.title,
      composite,
      assets: { ...draftAssetsRef.current, ...placedProjectContents(projectScope) },
    }) : await populateProjectRealm({}, JSON.stringify({ display: { title: seedRef.current.scene.title } }), {
      signal, scope, composite, assets: { ...draftAssetsRef.current, ...placedProjectContents(scope) },
    });
    if (!res.ok) {
      throw new Error(`Could not prepare the template realm: ${res.reason}`);
    }
  }, [gameTemplateId, rawComposite, projectScope, projectSlug]);

  const templateNotice =
    starter && !templateNoticeDismissed ? (
      <div
        className="editor-wizard__note"
        role="note"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
          margin: "0 0 8px",
          padding: "8px 12px",
          fontSize: "13px",
          lineHeight: 1.5,
          borderRadius: "8px",
          background: "var(--ink-1, rgba(255, 255, 255, 0.06))",
          color: "var(--ink-8, rgba(255, 255, 255, 0.8))",
        }}
      >
        <span>
          This scene started from the <strong>{starter.title}</strong> template &#x2014;
          its entities are yours to edit or delete. Press <strong>Play</strong> to
          run the template&rsquo;s starter game (Stop restores your scene). The
          Code panel shows the same starter code, but edits there don&rsquo;t
          change what Play runs yet &#x2014; run edited code with <code>npm start</code>{" "}
          after saving the project to disk.
          {starter.github_link ? (
            <>
              {" \u{B7} "}
              <a
                href={starter.github_link}
                target="_blank"
                rel="noreferrer"
                style={{ color: "inherit", textDecoration: "underline" }}
              >
                View the original scene on GitHub
              </a>
            </>
          ) : null}
        </span>
        <button
          type="button"
          className="editor-wizard__btn"
          aria-label="Dismiss template notice"
          onClick={() => setTemplateNoticeDismissed(true)}
        >
          Dismiss
        </button>
      </div>
    ) : null;

  const liveCopyNote =
    seed.scene.live && !liveCopyNoteDismissed ? (
      <div
        className="editor-wizard__note"
        role="note"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
          margin: "0 0 8px",
          padding: "8px 12px",
          fontSize: "13px",
          lineHeight: 1.5,
          borderRadius: "8px",
          background: "var(--ink-1, rgba(255, 255, 255, 0.06))",
          color: "var(--ink-8, rgba(255, 255, 255, 0.8))",
        }}
      >
        <span>
          You are editing your deployed copy &#x2014; changes stay in this browser
          until saved.
        </span>
        <button
          type="button"
          className="editor-wizard__btn"
          aria-label="Dismiss"
          onClick={() => setLiveCopyNoteDismissed(true)}
        >
          Dismiss
        </button>
      </div>
    ) : null;

  const localErrorBanner =
    failedToLoadLocal && !localErrorDismissed ? (
      <div
        className="editor-wizard__banner"
        role="alert"
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
          margin: "0 0 8px",
          padding: "8px 12px",
          fontSize: "13px",
          lineHeight: 1.5,
          borderRadius: "8px",
          background: "var(--error-bg, rgba(255, 92, 92, 0.14))",
          color: "var(--error, #ff8080)",
        }}
      >
        <span>
          We couldn&rsquo;t load that scene from your computer, so this is a fresh
          empty scene. Reopen the project, or reload to try again.
        </span>
        <span style={{ display: "flex", gap: "8px", flexShrink: 0 }}>
          <button
            type="button"
            className="editor-wizard__btn editor-wizard__btn--primary"
            onClick={() => {
              if (typeof window !== "undefined") window.location.reload();
            }}
          >
            Reload
          </button>
          <button
            type="button"
            className="editor-wizard__btn"
            aria-label="Dismiss"
            onClick={() => setLocalErrorDismissed(true)}
          >
            Dismiss
          </button>
        </span>
      </div>
    ) : null;

  async function onOpenFromDisk() {
    if (!canOperate) return;
    if (
      isDirty() &&
      typeof window !== "undefined" &&
      !window.confirm("You have unsaved changes. Open another scene and lose them?")
    ) {
      return;
    }
    const request = operations.begin("open", canOperate);
    if (!request) return;
    setDiskOpen({ phase: "picking" });
    try {
      const { openLocalScene, stageLocalSceneForEditor } = await import(
        "@/data/lib/fs/local-scene"
      );
      if (!operations.accepts(request)) return;
      const out = await openLocalScene({ onPhase: (phase) => { if (operations.accepts(request)) setDiskOpen({ phase }); } });
      if (!operations.accepts(request)) return;
      if (out.status === "cancelled") {
        finish(request, "cancelled");
        setDiskOpen({ phase: "idle" });
        return;
      }
      if (out.status === "no-composite") {
        finish(request, "failed");
        setDiskOpen({
          phase: "error",
          message: out.hasSceneJson ? NO_COMPOSITE_CODE_ONLY_HINT : NO_COMPOSITE_HINT,
        });
        return;
      }
      setDiskOpen({ phase: "opening" });
      await runEditorEffect(PROJECT_REALM_EFFECT, () => operations.accepts(request), () => stageLocalSceneForEditor(out.result, operations.signal(request)));
      if (!operations.accepts(request)) return;
      finish(request, "completed", true);
      if (typeof window !== "undefined") {
        window.location.assign(
          `/create/scene-editor?source=local&from=${encodeURIComponent(from ?? "editor")}`,
        );
      }
    } catch {
      if (!operations.accepts(request)) return;
      finish(request, "failed", false, OPEN_FAILED_HINT);
      setDiskOpen({ phase: "error", message: OPEN_FAILED_HINT });
    }
  }

  async function onSaveToDisk(destination?: "account") {
    const request = operations.begin("save", canSave);
    if (!request) return;
    if (sdkProject) {
      setDiskSave({ phase: "saving" });
      try {
        await persistPublishDraft(request);
        if (!operations.accepts(request)) return;
        finish(request, "completed", true);
        setDiskSave({ phase: "saved", via: "fsa-handle", filename: sdkDescriptor?.compositePath || "main.composite", entities: seed.hierarchy.length });
      } catch (error) {
        if (!operations.accepts(request)) return;
        finish(request, "failed");
        setDiskSave({ phase: "error", message: error instanceof Error ? error.message : String(error) });
      }
      return;
    }
    setDiskSave({ phase: "saving" });
    try {
      const { saveSceneFromEngine } = await import("@/data/lib/fs/save-scene");
      if (!operations.accepts(request)) return;
      const res = await runEditorEffect(persistenceKey, () => operations.accepts(request), () => saveSceneFromEngine(
        seed.hierarchy,
        {},
        {
          destination,
          signal: operations.signal(request),
          expectedComposite: diskCompositeRef.current,
          exportComposite: () => exportComposite(request),
          project: {
            slug: projectKey,
            title: sceneTitleRef.current,
            base: seed.scene.base,
            template: seed.scene.template,
            assets: projectAssets(),
          },
        },
      ));
      if (!operations.accepts(request)) return;
      if (!res.written) {
        finish(request, "cancelled", res.serverSynced === true);
        setDiskSave({ phase: "canceled", serverSynced: res.serverSynced });
      } else {
        if (res.via === "fsa-handle") diskCompositeRef.current = res.text;
        finish(request, "completed", true);
        setDiskSave({
          phase: "saved",
          via: res.via === "account" ? "account" : res.via === "download" ? "download" : "fsa-handle",
          entities: res.entities,
          filename: res.filename,
          serverSynced: res.serverSynced,
        });
      }
    } catch (e) {
      if (!operations.accepts(request)) return;
      finish(request, "failed");
      setDiskSave({
        phase: "error",
        message: e instanceof Error ? e.message : String(e ?? "unknown error"),
      });
    }
  }

  async function syncSharedScene(discardLocal = false) {
    if (!sdkProject || !canSave) return;
    if (discardLocal && !window.confirm("Replace your current scene with the saved shared scene? Unsaved changes in this viewport will be discarded.")) return;
    const request = operations.begin("sync", canSave);
    if (!request) return;
    setSharedSceneStatus({ error: false, message: "Synchronizing the shared scene\u2026" });
    const path = sdkDescriptor?.compositePath || "main.composite";
    const bus = createEditorBus(activeViewportSrc);
    try {
      const { requireEngineComposite } = await import("@/data/lib/fs/save-scene");
      const local = discardLocal ? undefined : await requireEngineComposite(12000, () => exportComposite(request));
      if (!operations.accepts(request)) return;
      const shared = await sdkProject.sharedComposite(path, local ?? undefined, operations.signal(request));
      if (!operations.accepts(request)) return;
      if (operations.current.current.revision !== request.revision) throw new Error("The scene changed during synchronization. Try Sync shared scene again.");
      await bus.rpc("restoreComposite", [shared.content], 30000);
      if (!operations.accepts(request)) return;
      // A later SDK rebuild must recover the newly hydrated shared state, not
      // the pre-sync checkpoint captured while computing the three-way merge.
      await exportComposite(request);
      if (!operations.accepts(request)) return;
      sdkProject.acknowledgeComposite(path, shared.baseline);
      setExternalSceneVersion(version => version + 1);
      setDiskSave({ phase: "idle" });
      finish(request, "completed", discardLocal);
      setSharedSceneStatus({ error: false, message: discardLocal ? "Opened the saved shared scene." : "Shared changes merged into your scene. Your unsaved changes are preserved." });
    } catch (error) {
      if (!operations.accepts(request)) return;
      finish(request, "failed");
      setSharedSceneStatus({ error: true, message: error instanceof Error ? error.message : String(error) });
    } finally { bus.close(); }
  }

  const openStatusStrip =
    diskOpen.phase === "idle" ? null : (
      <Controls label="Open">
        {diskOpen.phase === "error" ? (
          <span role="alert" style={{ color: "var(--error, #ff8080)" }}>
            {diskOpen.message}
          </span>
        ) : (
          <span className="editor-wizard__spinner" role="status">
            {diskOpen.phase === "picking"
              ? "Pick the project folder that holds your saved scene\u{2026}"
              : diskOpen.phase === "reading"
                ? "Reading the project folder\u{2026}"
                : "Opening the scene\u{2026}"}
          </span>
        )}
        {diskOpen.phase === "error" && (
          <button
            type="button"
            className="editor-wizard__btn"
            onClick={() => setDiskOpen({ phase: "idle" })}
          >
            Dismiss
          </button>
        )}
      </Controls>
    );

  const saveStatusStrip =
    diskSave.phase === "idle" ? null : (
      <Controls label="Save">
        {diskSave.phase === "saving" ? (
          <span className="editor-wizard__spinner" role="status">
            Saving your scene&#x2026;
          </span>
        ) : diskSave.phase === "saved" ? (
          <span className="editor-wizard__saved" role="status">
            {diskSave.via === "account"
              ? "Saved to your account."
              : diskSave.via === "download"
              ? "Scene composite downloaded \u{2014} re-save it over your project's main.composite."
              : `Wrote ${diskSave.filename} to disk (${diskSave.entities} entities).`}
            {diskSave.serverSynced === true && diskSave.via !== "account"
              ? " Also saved to your account."
              : diskSave.serverSynced === false
                ? " Cloud copy not saved \u{2014} sign in to keep scenes across browsers."
                : null}
          </span>
        ) : diskSave.phase === "canceled" ? (
          <span role="status" style={{ color: "var(--ink-7, rgba(255, 255, 255, 0.7))" }}>
            {diskSave.serverSynced === true
              ? "No local folder chosen \u{2014} but the scene was saved to your account."
              : "Save cancelled \u{2014} nothing was written."}
          </span>
        ) : (
          <span role="alert" style={{ color: "var(--error, #ff8080)" }}>
            Save failed: {diskSave.message}
          </span>
        )}
        {diskSave.phase !== "saving" && (
          <button
            type="button"
            className="editor-wizard__btn"
            onClick={() => setDiskSave({ phase: "idle" })}
          >
            Dismiss
          </button>
        )}
      </Controls>
    );

  return (
    <div className="editor-wizard">
      <DeWorkspace
        externalSceneVersion={externalSceneVersion}
        onRecovered={() => operations.send({ type: "edited" })}
        recoveryKey={projectScope.runtimeId}
        previousRecoveryKey={projectScope.previousRuntimeId}
        onLifecycle={onLifecycle}
        onAuthoringCaptureReady={onAuthoringCaptureReady}
        operationsBusy={operationsBusy}
        renderHeader={(navigation, scene) => (
          <DeEditorAppBar
            exitDisabled={operationsBusy}
            busy={enginePlaying || workspace.busy || operationsBusy}
            title={scene.name}
            onRename={scene.rename}
            projectTools={<nav aria-label="Project tools" className="editor-wizard__project-tools">
              {!sdkProject && isConnected && <button type="button" className="editor-wizard__btn" disabled={!canSave} onClick={() => void onSaveToDisk("account")}>Save to account</button>}
              <button type="button" className="editor-wizard__btn" disabled={!canOperate} onClick={() => setSettingsOpen(true)}>Scene settings</button>
              {sdkProject && <a href={sdkProject.link("storage")} target="_blank" rel="noreferrer">Storage</a>}
              {onDevicePreview && <button type="button" className="editor-wizard__btn" onClick={onDevicePreview}>Device preview</button>}
              {sdkProject && <span title={sdkDescriptor?.capabilities.watch ? "Preview rebuilds on save" : "File watching is off"}>SDK project</span>}
              {sdkProject && <button type="button" className="editor-wizard__btn" disabled={!canSave} onClick={() => void syncSharedScene()}>Sync shared scene</button>}
            </nav>}
            viewportSrc={viewportSrc}
            previewSrc={previewSrc}
            engine={engineStatus}
            publishOptions={buildPublishOptions(seed.scene)}
            onExit={guardedExit}
            onPublish={canSave ? guardedPublish : undefined}
          >{navigation}</DeEditorAppBar>
        )}
        title={seed.scene.title}
        onSceneNameChange={onSceneNameChange}
        tree={tree}
        inspector={inspector}
        catalog={catalogItems}
        local={localAssets}
        viewportSrc={viewportSrc}
        rawComposite={rebaseLegacyProjectUrls(rawComposite, projectScope)}
        code={workspaceCode}
        prepareRealm={!sdkProject ? prepareRealm : undefined}
        onEngineStatus={setEngineStatus}
        onSaveToDisk={canSave ? onSaveToDisk : undefined}
        onSceneSettings={!canOperate ? undefined : () => setSettingsOpen(true)}
        onOpenFromDisk={!canOperate || sdkProject ? undefined : () => void onOpenFromDisk()}
        onPublish={canSave && guardedPublish ? () => guardedPublish() : undefined}
        sceneInfo={{
          base: seed.scene.base,
          parcels: seed.scene.parcels,
          template: seed.scene.template ?? null,
        }}
        saveState={
          diskSave.phase === "saving"
            ? "saving"
            : diskSave.phase === "error"
              ? "error"
              : (diskSave.phase === "saved" ||
                    (diskSave.phase === "canceled" && diskSave.serverSynced === true)) &&
                  !isDirty()
                ? "saved"
                : "idle"
        }
      />

      {localErrorBanner}
      {templateNotice}
      {liveCopyNote}
      {operations.state.pending?.command === "publish" && <Controls label="Publish"><span role="status">{"Preparing your scene for publishing\u2026"}</span></Controls>}
      {openStatusStrip}
      {saveStatusStrip}
      {sharedSceneStatus && <Controls label="Shared scene"><span role={sharedSceneStatus.error ? "alert" : "status"}>{sharedSceneStatus.message}</span>
        {sharedSceneStatus.error && <button type="button" disabled={!canSave} onClick={() => void syncSharedScene(true)}>Open saved shared scene</button>}
      </Controls>}
      {settingsOpen && <DeSceneSettings load={loadSettings} onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}

type TreeNode = {
  id: string;
  name: string;
  selected: boolean;
  expanded: boolean;
  children: TreeNode[];
};

function buildHierarchyTree(nodes: HierarchyNode[]): TreeNode[] {
  const byId = new Map<number, TreeNode>();
  for (const n of nodes) {
    byId.set(n.entity, {
      id: String(n.entity),
      name: n.name,
      selected: n.selected,
      expanded: false,
      children: [],
    });
  }
  const roots: TreeNode[] = [];
  for (const n of nodes) {
    const node = byId.get(n.entity)!;
    const parent = n.parent !== n.entity ? byId.get(n.parent) : undefined;
    if (parent) {
      parent.children.push(node);
      parent.expanded = true;
    } else {
      roots.push(node);
    }
  }
  return roots;
}

type PublishOption = { id: string; label: string };

function buildPublishOptions(scene: SceneEditorSeed["scene"]): PublishOption[] {
  const options: PublishOption[] = [{ id: "publish-scene", label: "Publish Scene" }];
  if (scene.live) {
    const isCoords = /^-?\d+\s*,\s*-?\d+$/.test(scene.pointer);
    const destination = !isCoords && scene.pointer ? scene.pointer : scene.base;
    if (destination) {
      options.push({ id: "republish", label: `Republish to ${destination}` });
    }
  }
  return options;
}
