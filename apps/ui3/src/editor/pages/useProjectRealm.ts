import { useEffect, useRef, useState } from "react";
import { initialProjectRealmMachine, transitionProjectRealm, type ProjectRealmStatus } from "./project-realm-machine";
import { BOOT_TIMEOUT_MS } from "../editor-config";
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
    const sleepMs = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

    const ensureWorker = async () => {
      try {
        const swc = typeof navigator !== "undefined" ? navigator.serviceWorker : null;
        if (!swc || typeof swc.register !== "function") return;
        const viewport = new URL(viewportSrc!, window.location.href);
        if (viewport.origin !== window.location.origin) return;
        const enginePath = viewport.pathname.replace(/\/?$/, "/");
        const reg = await swc.register(`${enginePath}service_worker.js`, { scope: enginePath });
        for (let i = 0; i < 60 && current() && !reg.active; i += 1) await sleepMs(100);
        // The outer /create document is deliberately outside the engine scope
        // and will never acquire its controller. The iframe bootstrap waits
        // for control itself; do not add 1.5s to every editor entry here.
      } catch {
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
              for (let i = 0; i < 20 && !prepareRealmRef.current; i += 1) {
                if (!current()) return;
                await sleepMs(200);
              }
            }
            for (let attempt = 0; attempt < 5; attempt += 1) {
              if (!current()) return;
              if (prepareRealmRef.current) {
                // Immutable uploaded assets belong to this project, including
                // unsaved recovery checkpoints. Rebuild the manifest without
                // deleting those bytes on reload or on a transport Retry.
                await populate();
                if (!current()) return;
                if ((await cache.keys()).length > 0) break;
              }
              await sleepMs(300);
            }
            if (current() && prepareRealmRef.current && !(await cache.keys()).length) throw new Error("The project realm is empty");
          } else if (prepareRealmRef.current) {
            await populate();
            if (!current()) return;
            if (!(await cache.keys()).length) throw new Error("The project realm is empty");
          } else {
            let populated = false;
            for (let i = 0; i < 40; i += 1) {
              if (!current()) return;
              const keys = await cache.keys();
              if (keys.length > 0) {
                populated = true;
                break;
              }
              await sleepMs(200);
            }
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
