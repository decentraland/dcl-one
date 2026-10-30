const EMBED_ATTR = "data-embed";
// Pages that stay in their frame when embedded. /play/app.html is the web Explorer's app document,
// which only ever runs inside the /play shell's iframe: escaping it to the top would drop the shell
// and its service worker (and an isolated app may not touch the shell's location at all).
const EMBED_SCOPE_PREFIXES = ["/shop", "/marketplace", "/play/app.html"] as const;

export const EMBED_BOOT_SCRIPT =
  '(function(){try{if(self!==top)document.documentElement.setAttribute("data-embed","1")}catch(e){}})()';

export function isEmbedScopePath(pathname: string, scope: readonly string[] = EMBED_SCOPE_PREFIXES): boolean {
  return scope.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}

export function isEmbedded(doc: Document | undefined = typeof document === "undefined" ? undefined : document): boolean {
  return doc?.documentElement.getAttribute(EMBED_ATTR) === "1";
}

/** The path a same-origin embedder framed this document at (its frame element's `src`), or null.
    A frame that is still on that path was put there on purpose (the scene editor frames the /play
    shell as its viewport); only a navigation away from it leaves the frame. */
export function framedPathname(win: Window): string | null {
  try {
    const src = win.frameElement?.getAttribute("src");
    return src ? new URL(src, win.location.href).pathname : null;
  } catch {
    return null;
  }
}

type EmbedLinkDisposition = "frame" | "top" | "blank";

export function embedLinkDisposition(href: string, origin: string, scope?: readonly string[]): EmbedLinkDisposition {
  let url: URL;
  try {
    url = new URL(href, origin);
  } catch {
    return "frame";
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return "frame";
  if (url.origin !== origin) return "blank";
  return isEmbedScopePath(url.pathname, scope) ? "frame" : "top";
}

export function embedEscapeHref(pathname: string, search = "", hash = "", scope?: readonly string[]): string | null {
  return isEmbedScopePath(pathname, scope) ? null : `${pathname}${search}${hash}`;
}

type EmbedLinkGuardOptions = {
  scope?: readonly string[];
  open?: (url: string) => void;
};

export function installEmbedLinkGuard(win: Window, { scope, open }: EmbedLinkGuardOptions = {}): () => void {
  const onClick = (e: MouseEvent) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    const el = e.target instanceof Element ? e.target.closest("a[href]") : null;
    if (!(el instanceof HTMLAnchorElement) || el.hasAttribute("download")) return;
    const target = el.getAttribute("target");
    if (target && target !== "_self" && !open) return;
    const explorerLink = open !== undefined && el.href.startsWith("decentraland:");
    const disposition = explorerLink ? "top" : embedLinkDisposition(el.href, win.location.origin, scope);
    if (disposition === "frame") return;
    e.preventDefault();
    e.stopPropagation();
    if (open) open(el.href);
    else if (disposition === "top") (win.top ?? win).location.assign(el.href);
    else win.open(el.href, "_blank", "noopener");
  };
  win.addEventListener("click", onClick, true);
  return () => win.removeEventListener("click", onClick, true);
}

export const EMBED_ESCAPE_MESSAGE = "dcl-embed-escape";

export function installEmbedEscapeRelay(win: Window): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    const parent = win.parent;
    if (!parent || parent === win) return;
    parent.postMessage(EMBED_ESCAPE_MESSAGE, "*");
  };
  win.addEventListener("keydown", onKey);
  return () => win.removeEventListener("keydown", onKey);
}

export function installEmbedGuards(win: Window, options: EmbedLinkGuardOptions = {}): () => void {
  const offLinks = installEmbedLinkGuard(win, options);
  const offEscape = options.open ? undefined : installEmbedEscapeRelay(win);
  return () => {
    offLinks();
    offEscape?.();
  };
}

export function isEmbedEscapeMessage(e: MessageEvent, frame: HTMLIFrameElement | null): boolean {
  const from = frame?.contentWindow;
  return e.data === EMBED_ESCAPE_MESSAGE && from != null && e.source === from;
}

export function frameEscapeHref(frame: HTMLIFrameElement | null): string | null {
  try {
    const loc = frame?.contentWindow?.location;
    if (!loc || loc.href === "about:blank") return null;
    if (loc.protocol !== "http:" && loc.protocol !== "https:") return null;
    return isEmbedScopePath(loc.pathname) ? null : loc.href;
  } catch {
    return null;
  }
}
