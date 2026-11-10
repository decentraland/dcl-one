// Opens an installed Decentraland desktop app through its custom scheme (decentraland/ui2@2cca0cf3
// modules/jumpIn.ts, Apache-2.0; download contract §5). /download forwards position, realm and
// community as dcl-one's /download already did (plan behavior 2), on top of source's dclenv,
// scene-console and multi-instance.

export type LaunchOptions = {
  position?: string;
  realm?: string;
  /** Sent as the deep link's `community` key, as ui2 does. */
  communityId?: string;
  dclenv?: string;
  sceneConsole?: string;
  multiInstance?: string;
  timeoutMs?: number;
  mobile?: boolean;
};

const ENV_TO_DCLENV: Record<string, string> = {
  dev: "zone",
  stg: "today",
  prd: "org",
  prod: "org",
};
const DCLENV_VALUES = new Set(Object.values(ENV_TO_DCLENV));

/**
 * The launch flags a visitor's URL may carry (source hooks/useDeepLinkQueryParams.ts). Each is
 * validated, because the deep link presence-checks them: `multi-instance=false` would otherwise
 * switch it on, and an arbitrary `dclenv` would pick which Explorer environment opens.
 */
export function launchFlags(
  params: URLSearchParams,
): Pick<LaunchOptions, "dclenv" | "sceneConsole" | "multiInstance"> {
  const dclenv = params.get("dclenv");
  return {
    dclenv:
      dclenv && DCLENV_VALUES.has(dclenv)
        ? dclenv
        : ENV_TO_DCLENV[params.get("env") ?? ""],
    sceneConsole: params.get("scene-console") === "true" ? "true" : undefined,
    multiInstance: params.get("multi-instance") === "true" ? "true" : undefined,
  };
}

export function desktopAppUrl(opts: LaunchOptions): string {
  const params = new URLSearchParams();
  if (opts.position) params.set("position", opts.position);
  if (opts.realm) params.set("realm", opts.realm);
  if (opts.communityId) params.set("community", opts.communityId);
  if (opts.dclenv) params.set("dclenv", opts.dclenv);
  if (opts.sceneConsole) params.set("scene-console", opts.sceneConsole);
  if (opts.multiInstance) params.set("multi-instance", opts.multiInstance);
  return `decentraland://?${params.toString()}`;
}

function isElectron(): boolean {
  const w = window as Window & { process?: { type?: string } };
  return (
    w.process?.type === "renderer" || navigator.userAgent.includes("Electron")
  );
}

/**
 * Resolves true when the page loses focus or visibility within the timeout (the app took over),
 * false when the scheme is not registered, on mobile, inside Electron or on timeout. Call it
 * from a user gesture.
 */
export function launchDesktopApp(opts: LaunchOptions = {}): Promise<boolean> {
  if (opts.mobile || isElectron()) return Promise.resolve(false);

  let opened = false;
  const onLoseFocus = () => {
    opened = true;
  };
  const onVisibility = () => {
    if (document.visibilityState === "hidden") onLoseFocus();
  };
  const cleanup = () => {
    document.removeEventListener("visibilitychange", onVisibility);
    window.removeEventListener("pagehide", onLoseFocus);
    window.removeEventListener("blur", onLoseFocus);
  };
  document.addEventListener("visibilitychange", onVisibility, {
    passive: true,
  });
  window.addEventListener("pagehide", onLoseFocus, { passive: true });
  window.addEventListener("blur", onLoseFocus, { passive: true });

  try {
    window.location.assign(desktopAppUrl(opts));
  } catch {
    cleanup();
    return Promise.resolve(false);
  }

  return new Promise((resolve) => {
    setTimeout(() => {
      cleanup();
      resolve(opened);
    }, opts.timeoutMs ?? 750);
  });
}
