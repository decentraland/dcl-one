import { backoffDelay, sleep, waitUntil } from "@dcl/async";
import { useEffect, useRef, useState } from "react";
import { initialProjectRealmMachine, transitionProjectRealm, type ProjectRealmStatus } from "./project-realm-machine";
import {
  BOOT_TIMEOUT_MS,
  PROJECT_REALM_BACKOFF,
  PROJECT_SERVICE_WORKER_TIMEOUT_MS,
  PROJECT_PREPARE_TIMEOUT_MS,
  PROJECT_CACHE_SEED_TIMEOUT_MS,
  PROJECT_REOPEN_TIMEOUT_MS,
} from "../editor-config";
import { PROJECT_REALM_EFFECT, setProjectPlayState, projectScopeFromViewport } from "../project-cache";
import { runEditorEffect } from "../serial-effect";
import { restoreProjectCache } from "../project-cache-name";

export function useProjectRealm(
  viewportSrc: string | null | undefined,
  prepareRealm: ((signal?: AbortSignal) => Promise<unknown>) | null | undefined,
): ProjectRealmStatus {
  const projectScope = projectScopeFromViewport(viewportSrc);
  const isProjectRealm = projectScope !== null;
  const machine = useRef(initialProjectRealmMachine(isProjectRealm));
  const [status, setStatus] = useState<ProjectRealmStatus>(machine.current.status);
  const request = useRef(0);
  const session = useRef(0);
  const scope = useRef(viewportSrc);
  const scopeChanged = scope.current !== viewportSrc;
  const transition = (event: Parameters<typeof transitionProjectRealm>[1]) => {
    machine.current = transitionProjectRealm(machine.current, event);
    setStatus(machine.current.status);
  };
  const prepareRealmRef = useRef(prepareRealm);
  prepareRealmRef.current = prepareRealm;

  useEffect(() => {
    const activeRequest = ++request.current;
    const activeSession = ++session.current;
    scope.current = viewportSrc;
    if (!isProjectRealm) {
      transition({ type: "start", request: activeRequest, session: activeSession });
      transition({ type: "ready", request: activeRequest, session: activeSession });
      return undefined;
    }
    transition({ type: "start", request: activeRequest, session: activeSession });
    let cancelled = false;
    const controller = new AbortController();
    const current = () => !cancelled && !controller.signal.aborted && request.current === activeRequest && session.current === activeSession && machine.current.status === "pending";
    const timeout = setTimeout(() => {
      if (!current()) return;
      controller.abort();
      transition({ type: "error", request: activeRequest, session: activeSession });
    }, BOOT_TIMEOUT_MS);
    const wait = (timeoutMs: number, description: string) => ({
      timeoutMs, description, signal: controller.signal,
      initialDelayMs: PROJECT_REALM_BACKOFF.initialMs, maxDelayMs: PROJECT_REALM_BACKOFF.maxMs,
    });
    // A wait that ran out is where the fixed-interval loops fell through; an abort or a failed
    // preparation still propagates.
    const ranOut = (error: unknown): false => {
      if (error instanceof Error && error.message.startsWith("Timed out:")) return false;
      throw error;
    };
    // Repeats `attempt` until it reports done or the budget ends. The budget bounds the pauses
    // between attempts, not a preparation in progress: the boot timeout aborts that, as before.
    const attempts = async (attempt: () => Promise<boolean>, timeoutMs: number): Promise<boolean> => {
      const deadline = Date.now() + timeoutMs;
      for (let tries = 0; ; tries += 1) {
        if (await attempt()) return true;
        if (!current()) return false;
        const remaining = deadline - Date.now();
        if (remaining <= 0) return false;
        await sleep(backoffDelay(tries, PROJECT_REALM_BACKOFF, remaining), controller.signal);
      }
    };

    const ensureWorker = async () => {
      try {
        const swc = typeof navigator !== "undefined" ? navigator.serviceWorker : null;
        if (!swc || typeof swc.register !== "function") return;
        const viewport = new URL(viewportSrc!, window.location.href);
        if (viewport.origin !== window.location.origin) return;
        const enginePath = viewport.pathname.replace(/\/?$/, "/");
        const reg = await swc.register(`${enginePath}service_worker.js`, { scope: enginePath });
        await waitUntil(() => reg.active !== null, wait(PROJECT_SERVICE_WORKER_TIMEOUT_MS, "the engine service worker activating"));
        // The outer /create document is deliberately outside the engine scope
        // and will never acquire its controller. The iframe bootstrap waits
        // for control itself; do not add 1.5s to every editor entry here.
      } catch {
        // A registration that is refused or a worker that never activates does not
        // block the editor: the iframe bootstrap waits for control itself.
      }
    };

    const populate = async () => {
      const fn = prepareRealmRef.current;
      if (typeof fn !== "function") return;
      await fn(controller.signal);
    };

    const run = async () => {
      const C = typeof caches !== "undefined" ? caches : null;
      const search = typeof window !== "undefined" ? window.location.search : "";
      const isLocalReopen = /[?&]source=local(?:[&=]|$)/.test(search);
      const isDraftReopen = /[?&]draft=/.test(search);
      if (C) {
        await restoreProjectCache(projectScope!);
        if (!current()) return;
        const cache = await C.open(projectScope!.cacheName);
        if (!current()) return;
        if (cache) {
          if (!isLocalReopen) {
            if (isDraftReopen) {
              await waitUntil(() => !!prepareRealmRef.current, wait(PROJECT_PREPARE_TIMEOUT_MS, "the draft's preparation")).catch(ranOut);
            }
            // Immutable uploaded assets belong to this project, including
            // unsaved recovery checkpoints. Rebuild the manifest without
            // deleting those bytes on reload or on a transport Retry.
            const seeded = await attempts(async () => {
              if (prepareRealmRef.current) await populate();
              return current() && (await cache.keys()).length > 0;
            }, PROJECT_CACHE_SEED_TIMEOUT_MS);
            if (!current()) return;
            if (!seeded && prepareRealmRef.current) throw new Error("The project realm is empty");
          } else if (prepareRealmRef.current) {
            await populate();
            if (!current()) return;
            if (!(await cache.keys()).length) throw new Error("The project realm is empty");
          } else {
            const populated = await attempts(async () => (await cache.keys()).length > 0, PROJECT_REOPEN_TIMEOUT_MS);
            if (!current()) return;
            if (!populated) {
              await populate();
              if (!current()) return;
            }
          }
        }
      }
      if (!current()) return;
      await setProjectPlayState(false, false, projectScope!);
      if (!current()) return;
      await ensureWorker();
      if (current()) {
        transition({ type: "ready", request: activeRequest, session: activeSession });
      }
    };
    void runEditorEffect(`${PROJECT_REALM_EFFECT}:${projectScope!.id}`, current, run)
      .catch(() => {
        if (current()) transition({ type: "error", request: activeRequest, session: activeSession });
      })
      .finally(() => clearTimeout(timeout));
    return () => {
      cancelled = true;
      controller.abort();
      clearTimeout(timeout);
    };
  }, [viewportSrc, isProjectRealm]);

  return scopeChanged && isProjectRealm ? "pending" : status;
}
