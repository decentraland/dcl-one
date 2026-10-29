import { useEffect, useMemo, useRef, useState, type RefObject } from "react";
import type { BusEnvelope } from "../bus-protocol";
import { editorBusEditsProject } from "../bus-edits";
import { editorBusChannelFromViewportSrc, type EditorBus } from "../editor-bus";
import { createSceneRecovery, validSceneComposite } from "../scene-recovery";
import { canonicalProjectComposite, prepareProjectComposite, projectScopeFromViewport, ProjectUploadUnavailableError } from "../project-cache-name";

export const RECOVERY_DELAY_MS = 500;
export const RECOVERY_POLL_MS = 5000;

function runtimeIdentity(stats: unknown): string | null {
  if (typeof stats !== "string") return null;
  const hash = /^hash:\s*(\S+)/m.exec(stats)?.[1];
  const instance = /^instance:\s*(\S+)/m.exec(stats)?.[1];
  return hash && instance ? `${hash}:${instance}` : null;
}

function authoredRuntimeReady(stats: unknown): boolean {
  return typeof stats === "string" && Number(/^tick:\s*(\d+)/m.exec(stats)?.[1]) >= 3 &&
    !/^broken:\s*true\s*$/m.test(stats);
}

export function useSceneRecovery({ key, previousKey, input, busRef, viewportSrc, enabled, playing = false, onRecovered }: {
  key: string | null;
  previousKey?: string | null;
  input: string | null;
  busRef: RefObject<EditorBus | null>;
  viewportSrc: string | null | undefined;
  enabled: boolean;
  playing?: boolean;
  onRecovered?: () => void;
}) {
  const scope = useMemo(() => projectScopeFromViewport(viewportSrc), [viewportSrc]);
  const identity = useRef({ key, input, previousKey, scope });
  identity.current = { key, input, previousKey, scope };
  const recovery = useMemo(() => key ? createSceneRecovery(key, input, undefined, previousKey,
    () => identity.current.key === key && identity.current.input === input && identity.current.previousKey === previousKey && identity.current.scope === scope,
    scope ? value => canonicalProjectComposite(value, scope) : undefined) : null, [key, previousKey, input, scope]);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [assetError, setAssetError] = useState<string | null>(null);
  const active = useRef({ recovery, enabled, playing });
  active.current = { recovery, enabled, playing };
  const markChanged = useRef<(() => void) | null>(null);
  const notified = useRef<typeof recovery>(null);
  const captureEpoch = useRef(0);
  const explicitCaptures = useRef(0);
  const captureSession = useRef<{ bus: EditorBus; runtime: Promise<string | null>; current: () => boolean; revision: () => number } | null>(null);
  const pendingExplicit = useRef<{ recovery: typeof recovery; session: NonNullable<typeof captureSession.current>; revision: number; promise: Promise<string> } | null>(null);
  const acknowledgedSession = useRef<{ recovery: typeof recovery; viewportSrc: typeof viewportSrc; runtime: Promise<string | null>; received: (composite: string, runtime: string) => void } | null>(null);
  useEffect(() => {
    setNotice(null);
    setError(null);
    setAssetError(null);
  }, [recovery]);

  const failure = (message?: string) => setError(message ?? "Automatic recovery could not be saved. Save your scene to a folder before closing this tab.");
  const preserve = async (composite: unknown, current: () => boolean = () => true, validated: Promise<boolean> | boolean = true) => {
    if (!recovery) return false;
    const accepted = () => active.current.recovery === recovery && current();
    try {
      const retained = await recovery.checkpoint(composite, accepted, validated);
      if (retained && accepted()) setError(null);
      return retained && accepted();
    } catch {
      if (!accepted()) return false;
      failure();
      const retained = await recovery.restore().catch(() => null);
      return accepted() && retained === composite;
    }
  };
  const captureSnapshot = async (session: NonNullable<typeof captureSession.current>): Promise<string> => {
    if (!recovery || !session || !active.current.enabled || !session.current()) throw new Error("The editor must be connected and stopped before saving authored changes.");
    const epoch = ++captureEpoch.current;
    explicitCaptures.current++;
    const current = () => active.current.recovery === recovery && active.current.enabled && captureSession.current === session && session.current() && captureEpoch.current === epoch;
    const requireCurrent = () => { if (!current()) throw new Error("The editor changed while capturing authored changes. Retry the save."); };
    try {
      const expected = await session.runtime;
      requireCurrent();
      const before = await session.bus.rpc("sceneStats");
      requireCurrent();
      if (!expected || runtimeIdentity(before) !== expected || !authoredRuntimeReady(before)) throw new Error("The authored scene is being replaced. Retry once it reconnects.");
      const composite = await session.bus.exportComposite();
      requireCurrent();
      if (!validSceneComposite(composite)) throw new Error("The engine returned an invalid authored scene snapshot.");
      const after = await session.bus.rpc("sceneStats");
      requireCurrent();
      if (runtimeIdentity(after) !== expected || !authoredRuntimeReady(after)) throw new Error("The authored scene changed during capture. Retry once it reconnects.");
      if (!await preserve(composite, current)) throw new Error("The editor could not retain the authored scene snapshot. Retry the save.");
      requireCurrent();
      return composite;
    } finally { explicitCaptures.current--; }
  };
  const captureAuthoring = (): Promise<string> => {
    const session = captureSession.current;
    if (!recovery || !session || !active.current.enabled || !session.current()) return Promise.reject(new Error("The editor must be connected and stopped before saving authored changes."));
    const prior = pendingExplicit.current;
    const sameSession = prior?.recovery === recovery && prior.session === session;
    const revision = session.revision();
    if (sameSession && prior.revision === revision) return prior.promise;
    const promise = (sameSession ? prior.promise.catch(() => {}) : Promise.resolve()).then(() => captureSnapshot(session));
    const pending = { recovery, session, revision, promise };
    pendingExplicit.current = pending;
    const settled = () => { if (pendingExplicit.current === pending) pendingExplicit.current = null; };
    void promise.then(settled, settled);
    return promise;
  };
  const reportRecovered = () => {
    if (active.current.recovery !== recovery || !recovery?.recovered) return;
    setNotice("Recovered your unsaved scene changes. Save to a folder to keep a permanent copy.");
    if (notified.current !== recovery) { notified.current = recovery; onRecovered?.(); }
  };
  const restore = async () => {
    if (!recovery) return input;
    let result = input;
    let preparingAssets = false;
    try {
      result = await recovery.restore();
      preparingAssets = true;
      const prepared = scope ? await prepareProjectComposite(result, scope) : result;
      preparingAssets = false;
      if (active.current.recovery === recovery) setAssetError(null);
      if (prepared !== result && recovery.recovered && active.current.recovery === recovery) await preserve(prepared);
      result = prepared;
      if (active.current.recovery === recovery && recovery.durable) setError(null);
      if (active.current.recovery === recovery && recovery.recovered) {
        if (!recovery.durable) failure();
        reportRecovered();
      }
      return result;
    } catch (error) {
      if (active.current.recovery === recovery) {
        if (preparingAssets) setAssetError(error instanceof ProjectUploadUnavailableError
          ? error.message
          : "Your scene changes were recovered, but their uploaded files could not be restored. Retry or reopen the original project folder.");
        else failure();
      }
      reportRecovered();
      return result;
    }
  };

  useEffect(() => {
    const channelName = editorBusChannelFromViewportSrc(viewportSrc);
    if (!recovery || !channelName || typeof BroadcastChannel === "undefined") return;
    let retired = false;
    const channel = new BroadcastChannel(channelName);
    channel.onmessage = event => {
      const session = acknowledgedSession.current;
      if (retired || active.current.recovery !== recovery || active.current.playing || session?.recovery !== recovery || session.viewportSrc !== viewportSrc) return;
      const envelope = event.data as BusEnvelope | undefined;
      if (envelope?.to !== "page" || envelope.msg?.type !== "authoring-checkpoint") return;
      const { composite, runtime: acknowledgedRuntime } = envelope.msg;
      if (!validSceneComposite(composite) || typeof acknowledgedRuntime !== "string") return;
      session.received(composite, acknowledgedRuntime);
      const validated = session.runtime.then(expected => expected !== null && expected === acknowledgedRuntime, () => false);
      void preserve(composite, () => active.current.recovery === recovery, validated);
    };
    return () => { retired = true; channel.close(); };
  }, [recovery, viewportSrc]);

  useEffect(() => {
    const bus = busRef.current;
    const channelName = editorBusChannelFromViewportSrc(viewportSrc);
    if (!recovery || !enabled || !bus || !channelName || typeof BroadcastChannel === "undefined") return;
    let retired = false;
    let capturing = false;
    let revision = 0;
    let capturedRevision = 0;
    let previous: unknown;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const current = () => !retired && active.current.enabled && active.current.recovery === recovery && busRef.current === bus;
    const runtime = bus.rpc("sceneStats").then(runtimeIdentity);
    void runtime.catch(() => {});
    const session = { bus, runtime, current, revision: () => revision };
    captureSession.current = session;
    let acknowledged: { composite: string; runtime: string } | null = null;
    acknowledgedSession.current = { recovery, viewportSrc, runtime, received: (composite, acknowledgedRuntime) => {
      if (acknowledged?.composite === composite && acknowledged.runtime === acknowledgedRuntime) return;
      acknowledged = { composite, runtime: acknowledgedRuntime };
      captureEpoch.current++;
      revision++;
      clearTimeout(timer);
    } };
    const capture = async () => {
      if (!current() || capturing || explicitCaptures.current) return;
      capturing = true;
      const epoch = captureEpoch.current;
      const captureCurrent = () => current() && captureEpoch.current === epoch && !explicitCaptures.current;
      const startedRevision = revision;
      try {
        const initialRuntime = await runtime;
        if (!current()) return;
        if (!initialRuntime) throw new Error("The engine runtime could not be identified for recovery");
        const before = await bus.rpc("sceneStats");
        if (runtimeIdentity(before) !== initialRuntime || !authoredRuntimeReady(before) || !current()) return;
        if (revision !== startedRevision || !captureCurrent()) return;
        const composite = await bus.exportComposite();
        if (!captureCurrent()) return;
        const after = await bus.rpc("sceneStats");
        if (runtimeIdentity(after) !== initialRuntime || !authoredRuntimeReady(after) || !current()) return;
        if (revision !== startedRevision || !captureCurrent()) return;
        if (startedRevision !== capturedRevision || (previous !== undefined && composite !== previous) || !recovery.durable) await preserve(composite, captureCurrent);
        if (!captureCurrent()) return;
        if (current() && recovery.durable) setError(null);
        previous = composite;
        capturedRevision = startedRevision;
      } catch { if (captureCurrent()) failure(); }
      finally {
        capturing = false;
        if (current() && revision !== startedRevision) timer = setTimeout(() => { void capture(); }, RECOVERY_DELAY_MS);
      }
    };
    const schedule = () => {
      if (!current()) return;
      revision++;
      clearTimeout(timer);
      timer = setTimeout(() => { void capture(); }, RECOVERY_DELAY_MS);
    };
    markChanged.current = schedule;
    const pending = new Set<string | number>();
    const channel = new BroadcastChannel(channelName);
    channel.onmessage = event => {
      if (!current()) return;
      const envelope = event.data as BusEnvelope | undefined;
      if (!envelope?.msg || typeof envelope.msg !== "object") return;
      if (envelope.to === "page" && envelope.msg.type === "rpc-reply" && pending.delete(envelope.msg.id)) {
        if (envelope.msg.ok) schedule();
      } else if (editorBusEditsProject(envelope) || (envelope.to === "scene" && envelope.msg.type === "rpc" && envelope.msg.method === "restoreComposite")) {
        if (envelope.msg.type === "rpc") pending.add(envelope.msg.id);
        else schedule();
      }
    };
    const interval = setInterval(() => { void capture(); }, RECOVERY_POLL_MS);
    const onVisibility = () => { if (document.visibilityState === "hidden") void capture(); };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      retired = true;
      if (captureSession.current === session) captureSession.current = null;
      if (markChanged.current === schedule) markChanged.current = null;
      clearTimeout(timer);
      clearInterval(interval);
      document.removeEventListener("visibilitychange", onVisibility);
      channel.close();
    };
  }, [recovery, viewportSrc, enabled]);

  return { restore, preserve, captureAuthoring, changed: () => markChanged.current?.(), notice, error: assetError ?? error, dismiss: () => { setNotice(null); setError(null); setAssetError(null); } };
}
