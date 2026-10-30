import type { ReactNode, RefObject } from "react";
import { useEffect, useReducer, useRef, useState } from "react";
import EngineViewport from "./EngineViewport";
import { isEngineOnlyViewport } from "../editor-bus";
import { BOOT_TIMEOUT_MS, BOOT_PROGRESS_POLL_MS, BOOT_LEAVE_MS, EDIT_VIEW_SETTLE_MS } from "../editor-config";
import {
  bootReducer, bootOverlay, bootClockElapsed, bootClockRelease, bootClockStart, formatElapsed,
  INITIAL_BOOT, type BootStage, type BootState,
} from "../boot-machine";
import "./dcleditorchrome.css";
import "./dcleditorchrome-toolbar.css";
import "./dcleditorchrome-menus.css";
import "./dcleditorchrome-hierarchy.css";
import "./dcleditorchrome-inspector.css";
import "./dcleditorchrome-shell.css";
import "./dcleditorchrome-assets.css";
import "./dcleditorchrome-overlays.css";

const BOOT_STAGES: readonly BootStage[] = ["download", "compile", "init", "workers", "gpu"];

function readEngineWindow(
  ref: RefObject<HTMLIFrameElement | null> | null,
): { progress: number | null; stage: BootStage | null; ready: boolean; crashed: boolean; exposed: boolean } {
  try {
    const w = ref?.current?.contentWindow as
      | (Window & {
          dclLoadingProgress?: unknown;
          dclLoadingStep?: unknown;
          dclEngineReady?: unknown;
          dclBootstrapSnapshot?: { phase?: string };
          engine_console_command?: unknown;
          engine_console_command_args?: unknown;
          __engineCrashed?: unknown;
        })
      | null
      | undefined;
    if (!w) return { progress: null, stage: null, ready: false, crashed: false, exposed: false };
    const p = w.dclLoadingProgress;
    const s = w.dclLoadingStep;
    return {
      progress: typeof p === "number" && isFinite(p) ? p : null,
      stage: BOOT_STAGES.includes(s as BootStage) ? (s as BootStage) : null,
      ready: w.dclBootstrapSnapshot
        ? w.dclBootstrapSnapshot.phase === "World"
        : w.dclEngineReady === true || typeof w.engine_console_command === "function" || typeof w.engine_console_command_args === "function",
      crashed: Boolean(w.__engineCrashed) || w.dclBootstrapSnapshot?.phase === "Failed",
      // The Explorer's shell frames its app in an isolated document this page cannot read; then
      // only the editor scene's answers on the bus say the engine runs.
      exposed: p !== undefined || w.dclBootstrapSnapshot !== undefined || w.dclEngineReady !== undefined
        || w.engine_console_command !== undefined || w.engine_console_command_args !== undefined,
    };
  } catch {
    return { progress: null, stage: null, ready: false, crashed: false, exposed: false };
  }
}

interface EditorBridge {
  onState: (listener: (push: unknown) => void) => () => void;
  send: (action: string, payload: object) => void;
}

function readEditorBridge(ref: RefObject<HTMLIFrameElement | null> | null): EditorBridge | null {
  try {
    return (ref?.current?.contentWindow as (Window & { dclBridge?: EditorBridge }) | null)?.dclBridge ?? null;
  } catch {
    return null;
  }
}

export type EditorEngineStatus = "connecting" | "online" | "offline";

export interface EditorBootstrapSnapshot { ready: boolean; error: string | null; stage: BootState["stage"]; progress: number | null }

interface DclEditorChromeProps {
  onBootstrap?: (snapshot: EditorBootstrapSnapshot) => void;
  onRetry?: () => void;
  children?: ReactNode;
  viewportSrc?: string | null;
  viewportRef?: RefObject<HTMLIFrameElement | null> | null;
  sceneReady?: boolean;
  editViewReady?: boolean;
  sceneError?: string | null;
  onViewportLoad?: () => void;
  loading?: boolean;
  loadError?: boolean;
  onEngineStatus?: ((status: EditorEngineStatus) => void) | null;
}

export default function DclEditorChrome({
  onBootstrap,
  onRetry,
  children,
  viewportSrc = null,
  viewportRef = null,
  sceneReady = undefined,
  editViewReady,
  sceneError = null,
  onViewportLoad,
  loading = false,
  loadError = false,
  onEngineStatus = null,
}: DclEditorChromeProps) {
  const [bootNonce, setBootNonce] = useState(0);
  const viewOnly = isEngineOnlyViewport(viewportSrc);
  const [boot, dispatchBoot] = useReducer(bootReducer, INITIAL_BOOT);
  const [engineLost, setEngineLost] = useState(false);
  const [prepTimedOut, setPrepTimedOut] = useState(false);
  const [viewTimedOut, setViewTimedOut] = useState(false);
  const [viewRevealed, setViewRevealed] = useState(false);
  const [worldReady, setWorldReady] = useState(false);
  const needsEditView = editViewReady !== undefined;
  const lostTicksRef = useRef(0);
  const engineExposedRef = useRef(false);
  const sceneReadyRef = useRef(sceneReady);
  sceneReadyRef.current = sceneReady;

  useEffect(() => {
    dispatchBoot({ type: "viewport", src: viewportSrc });
    lostTicksRef.current = 0;
    engineExposedRef.current = false;
    setEngineLost(false);
    setViewTimedOut(false);
    setViewRevealed(false);
    setWorldReady(false);
  }, [viewportSrc, bootNonce]);

  useEffect(() => {
    if (!viewportSrc || !needsEditView) return undefined;
    let unsubscribe: (() => void) | undefined;
    let misses = 0;
    const connect = () => {
      const bridge = readEditorBridge(viewportRef);
      if (!bridge) {
        // No lifecycle bridge to ask after a few polls (the app is in a document this page cannot
        // read): the editor scene having answered is the readiness there is.
        misses += 1;
        if (misses > 2 && sceneReadyRef.current && !engineExposedRef.current) setWorldReady(true);
        return;
      }
      clearInterval(id);
      unsubscribe = bridge.onState((push) => {
        if (!push || typeof push !== "object") return;
        const state = push as { kind?: string; ready?: boolean; pendingAssets?: number; snapshot?: { readiness?: { canExplore?: boolean; pendingAssets?: number } } };
        if (state.kind === "loading") setWorldReady(state.ready === true && state.pendingAssets === 0);
        else if (state.kind === "lifecycle" && state.snapshot?.readiness) {
          const readiness = state.snapshot.readiness;
          setWorldReady(readiness.canExplore === true && readiness.pendingAssets === 0);
        }
      });
      bridge.send("GetLifecycleSnapshot", {});
    };
    const id = setInterval(connect, BOOT_PROGRESS_POLL_MS);
    connect();
    return () => { clearInterval(id); unsubscribe?.(); };
  }, [viewportSrc, viewportRef, bootNonce, needsEditView]);

  useEffect(() => {
    if (sceneReady) {
      // the editor scene answers from inside the engine
      if (!engineExposedRef.current) dispatchBoot({ type: "engine-ready" });
      dispatchBoot({ type: "scene-ready" });
    }
    else {
      dispatchBoot({ type: "bus-reset" });
      if (sceneReady === false) setViewRevealed(false);
    }
  }, [sceneReady]);

  useEffect(() => {
    if (editViewReady === undefined || sceneReady === false || !editViewReady || !worldReady || viewRevealed) return undefined;
    const id = setTimeout(() => {
      setViewRevealed(true);
      setViewTimedOut(false);
    }, EDIT_VIEW_SETTLE_MS);
    return () => clearTimeout(id);
  }, [sceneReady, editViewReady, worldReady, viewRevealed]);

  const viewConfirmed = editViewReady === undefined || viewRevealed;

  useEffect(() => {
    if (!viewportSrc) return undefined;
    const id = setInterval(() => {
      const { progress, stage, ready, crashed, exposed } = readEngineWindow(viewportRef);
      if (exposed) engineExposedRef.current = true;
      if (boot.phase !== "ready") {
        if (progress != null) dispatchBoot({ type: "progress", pct: progress, stage });
        if (ready) dispatchBoot({ type: "engine-ready" });
      }
      if (crashed) {
        lostTicksRef.current = 0;
        setEngineLost(true);
        dispatchBoot({ type: "engine-error", reason: "The engine stopped. Retry to reconnect the editor." });
      } else if (ready) {
        lostTicksRef.current = 0;
        setEngineLost(false);
      } else if (boot.phase === "ready" && engineExposedRef.current) {
        lostTicksRef.current += 1;
        if (lostTicksRef.current >= 2) setEngineLost(true);
      }
    }, BOOT_PROGRESS_POLL_MS);
    return () => clearInterval(id);
  }, [viewportSrc, viewportRef, boot.phase, bootNonce, sceneReady]);

  useEffect(() => {
    if (!viewportSrc || viewOnly || boot.phase === "error" || (boot.phase === "ready" && viewConfirmed)) return undefined;
    const id = setTimeout(() => {
      if (boot.phase === "ready") setViewTimedOut(true);
      else dispatchBoot({ type: "timeout" });
    }, BOOT_TIMEOUT_MS);
    return () => clearTimeout(id);
  }, [viewportSrc, viewOnly, boot.phase, bootNonce, viewConfirmed]);

  useEffect(() => {
    if (viewportSrc || !loading || loadError) {
      setPrepTimedOut(false);
      return undefined;
    }
    const id = setTimeout(() => setPrepTimedOut(true), BOOT_TIMEOUT_MS);
    return () => clearTimeout(id);
  }, [viewportSrc, loading, loadError]);

  const prepFailed = !viewportSrc && (loadError || prepTimedOut);
  const status: EditorEngineStatus =
    viewOnly || boot.phase === "error" || engineLost || prepFailed || viewTimedOut || sceneError
      ? "offline"
      : boot.phase === "ready" && viewConfirmed
        ? "online"
        : "connecting";
  useEffect(() => {
    onBootstrap?.({ ready: boot.engineReady && !engineLost, error: engineLost ? "The engine disconnected." : boot.phase === "error" ? boot.reason : null, stage: boot.stage, progress: boot.progress });
  }, [onBootstrap, boot.engineReady, boot.phase, boot.reason, boot.stage, boot.progress, engineLost]);
  const onEngineStatusRef = useRef(onEngineStatus);
  onEngineStatusRef.current = onEngineStatus;
  useEffect(() => {
    onEngineStatusRef.current?.(status);
  }, [status]);

  const retryBoot = () => {
    onRetry?.();
    onViewportLoad?.();
    dispatchBoot({ type: "retry" });
    setBootNonce((n) => n + 1);
  };

  const overlay = sceneError
    ? ({ show: true, kind: "error", text: sceneError } as const)
    : viewTimedOut
    ? ({ show: true, kind: "error", text: "The editor camera did not become ready." } as const)
    : prepFailed
    ? ({
        show: true,
        kind: "error",
        text: loadError
          ? "The editor couldn't prepare your project files."
          : "Preparing your project is taking longer than usual.",
      } as const)
    : loading && !viewportSrc
      ? ({ show: true, kind: "loading", text: "Preparing project files\u{2026}" } as const)
      : viewOnly
      ? ({ show: false, kind: null, text: "" } as const)
      : boot.phase === "ready" && !viewConfirmed
      ? ({ show: true, kind: "loading", text: "Preparing editor view\u{2026}" } as const)
      : bootOverlay(boot);
  const overlayLoading = overlay.show && overlay.kind === "loading";

  const wasLoading = useRef(false);
  const [bootLeaving, setBootLeaving] = useState(false);
  useEffect(() => {
    if (overlayLoading) {
      wasLoading.current = true;
      setBootLeaving(false);
      return undefined;
    }
    if (!wasLoading.current) return undefined;
    wasLoading.current = false;
    setBootLeaving(true);
    const t = setTimeout(() => setBootLeaving(false), BOOT_LEAVE_MS);
    return () => clearTimeout(t);
  }, [overlayLoading]);

  const [elapsedS, setElapsedS] = useState(0);
  useEffect(() => {
    if (!overlayLoading) {
      setElapsedS(0);
      return undefined;
    }
    bootClockStart();
    setElapsedS(bootClockElapsed());
    const id = setInterval(() => setElapsedS(bootClockElapsed()), 1000);
    return () => {
      clearInterval(id);
      bootClockRelease();
    };
  }, [overlayLoading]);
  const retry = prepFailed
    ? () => {
        if (typeof window !== "undefined") window.location.reload();
      }
    : retryBoot;

  return (
    <div className="eui-root eui-viewport ui2" role="region" aria-label="DCL Editor">
      {viewportSrc ? (
        <EngineViewport
          key={bootNonce}
          viewportRef={viewportRef}
          src={viewportSrc}
          onLoad={() => {
            dispatchBoot({ type: "viewport", src: viewportSrc });
            onViewportLoad?.();
          }}
        />
      ) : (
        <>
          <div className="eui-vp-horizon" aria-hidden="true" />
          <div className="eui-vp-grid" aria-hidden="true" />
          <div className="eui-vp-object" aria-hidden="true">
            <div className="eui-vp-cube" />
            <div className="eui-vp-gizmo">
              <i className="ax-z" />
              <i className="ax-x" />
              <i className="ax-y" />
            </div>
          </div>
        </>
      )}
      {overlayLoading || bootLeaving ? (
        <div
          className={"eui-boot" + (bootLeaving ? " is-leaving" : "")}
          role={bootLeaving ? undefined : "status"}
          aria-hidden={bootLeaving ? true : undefined}
        >
          <span className="eui-boot-spinner" aria-hidden="true" />
          <span className="eui-boot-stage">{overlay.text}</span>
          {elapsedS >= 1 ? (
            <span className="eui-boot-elapsed">{formatElapsed(elapsedS)}</span>
          ) : null}
        </div>
      ) : null}
      {viewOnly ? (
        <div className="eui-vp-note" role="note">
          This Explorer runs your scene without the editor&rsquo;s own scene, so the viewport shows
          the project as saved and edits in it are not available.
        </div>
      ) : null}
      {overlay.show && overlay.kind === "error" ? (
        <div className="eui-boot eui-boot--error" role="alert">
          <span>{overlay.text} This sometimes happens on first load.</span>
          <button className="eui-btn primary" type="button" onClick={retry}>
            Retry
          </button>
        </div>
      ) : null}
      {children}
    </div>
  );
}
