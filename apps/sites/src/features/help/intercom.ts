import { useEffect, useMemo, useState } from "react";

/**
 * The Intercom chat widget, ported from decentraland/sites src/components/Intercom (IntercomWidget, Intercom). The
 * script loads only in the browser, from an effect, so after hydration; the server render never touches it.
 */

export type IntercomSettings = {
  readonly alignment?: "left" | "right";
};

type IntercomFn = (...args: unknown[]) => void;
type IntercomWindow = Window & {
  Intercom?: IntercomFn;
  intercomSettings?: IntercomSettings;
};

export const INTERCOM_WIDGET_URL = "https://widget.intercom.io/widget/";

function intercomWindow(): IntercomWindow | null {
  return typeof window === "undefined" ? null : (window as IntercomWindow);
}

function client(): IntercomFn | null {
  const fn = intercomWindow()?.Intercom;
  return typeof fn === "function" ? fn : null;
}

type IntercomStub = IntercomFn & { q: unknown[][]; c: (args: unknown[]) => void };

/**
 * Intercom's own install snippet: a queue that stands in for `window.Intercom` until the messenger frame, which the
 * widget script starts asynchronously, defines the real one and replays the queue.
 */
function installStub(win: IntercomWindow): IntercomStub {
  const stub = ((...args: unknown[]) => stub.c(args)) as IntercomStub;
  stub.q = [];
  stub.c = (args) => {
    stub.q.push(args);
  };
  win.Intercom = stub;
  return stub;
}

/**
 * Injects the widget script once per page. Calls made before the messenger is ready are queued, as Intercom's
 * snippet does. Rejects, and removes the queue, when the script fails to load.
 */
export function loadIntercom(appId: string, settings: IntercomSettings, doc: Document = document): Promise<void> {
  const win = intercomWindow();
  if (!win) return Promise.reject(new Error("Intercom loads only in the browser"));
  win.intercomSettings = settings;
  let script = Array.from(doc.querySelectorAll<HTMLScriptElement>("script[data-intercom]")).find(
    (candidate) => candidate.dataset.intercom === appId,
  );
  if (script?.dataset.loaded === "true") return Promise.resolve();
  if (!script) {
    if (!client()) installStub(win);
    script = doc.createElement("script");
    script.async = true;
    script.src = `${INTERCOM_WIDGET_URL}${encodeURIComponent(appId)}`;
    script.dataset.intercom = appId;
    doc.body.appendChild(script);
  }
  const loading = script;
  return new Promise((resolve, reject) => {
    const done = () => {
      loading.removeEventListener("error", failed);
      loading.dataset.loaded = "true";
      resolve();
    };
    const failed = () => {
      loading.removeEventListener("load", done);
      loading.remove();
      if ((win.Intercom as Partial<IntercomStub> | undefined)?.q) delete win.Intercom;
      reject(new Error("Failed to load the Intercom script"));
    };
    loading.addEventListener("load", done, { once: true });
    loading.addEventListener("error", failed, { once: true });
  });
}

/** Opens the chat, or queues the request while the messenger starts. False when no widget was ever requested. */
export function showIntercom(): boolean {
  const fn = client();
  if (!fn) return false;
  fn("show");
  return true;
}

/** Enable chat only after the workspace accepts this visitor's session. */
export function useIntercom(appId: string, settings: IntercomSettings, wallet?: string | null): boolean {
  const { alignment } = settings;
  const session = useMemo(
    () => ({ app_id: appId, alignment, ...(wallet ? { Wallet: wallet } : {}) }),
    [appId, alignment, wallet],
  );
  const [readySession, setReadySession] = useState<typeof session | null>(null);
  useEffect(() => {
    if (!session.app_id) return;
    let active = true;
    loadIntercom(session.app_id, { alignment: session.alignment })
      .then(() => {
        const fn = client();
        if (!active || !fn) return;
        fn("ready", () => {
          if (!active) return;
          setReadySession(session);
        });
        fn("boot", session);
      })
      .catch((error: unknown) => {
        console.error("Could not render Intercom", error instanceof Error ? error.message : error);
      });
    return () => {
      active = false;
      client()?.("shutdown");
    };
  }, [session]);
  return readySession === session;
}
