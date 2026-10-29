import { afterEach, describe, expect, it, vi } from "vitest";
import {
  EMBED_BOOT_SCRIPT,
  EMBED_ESCAPE_MESSAGE,
  embedEscapeHref,
  embedLinkDisposition,
  frameEscapeHref,
  framedPathname,
  installEmbedEscapeRelay,
  installEmbedLinkGuard,
  isEmbedEscapeMessage,
  isEmbedScopePath,
  isEmbedded,
} from "./embed";

const ORIGIN = "https://example.org";

describe("embed scope", () => {
  it("keeps the shop and every marketplace page inside the frame", () => {
    for (const p of ["/shop", "/shop/", "/marketplace", "/marketplace/names", "/marketplace/checkout"]) {
      expect(isEmbedScopePath(p)).toBe(true);
      expect(embedEscapeHref(p, "?tab=cart")).toBeNull();
    }
  });

  it("keeps the web Explorer's app document inside the /play shell's frame", () => {
    expect(embedEscapeHref("/play/app.html", "?position=10,-20")).toBeNull();
    for (const p of ["/play", "/play/", "/play/app", "/play/app.htmlx"]) expect(isEmbedScopePath(p)).toBe(false);
  });

  it("escapes everything else to the top window, keeping search and hash", () => {
    for (const p of ["/", "/play/", "/discover", "/create", "/blog", "/shopping", "/marketplaces"]) {
      expect(isEmbedScopePath(p)).toBe(false);
    }
    expect(embedEscapeHref("/play/", "?position=10,-20", "#x")).toBe("/play/?position=10,-20#x");
  });

  it("knows the path a same-origin embedder framed the document at", () => {
    const at = (src: string | null, href = "https://example.org/play/?realm=x") =>
      framedPathname({ frameElement: src === null ? null : { getAttribute: () => src }, location: { href } } as unknown as Window);
    expect(at("/play/?realm=x&position=0,0")).toBe("/play/");
    expect(at(null)).toBeNull();
    expect(framedPathname({ get frameElement(): never { throw new DOMException("cross-origin"); } } as unknown as Window)).toBeNull();
  });

  it("takes a host's own scope instead: the Explorer's Create panel keeps /create and lets the shop go", () => {
    const scope = ["/create"];
    expect(isEmbedScopePath("/create/scenes", scope)).toBe(true);
    expect(embedEscapeHref("/create", "?creator=0xabc", "", scope)).toBeNull();
    expect(embedEscapeHref("/shop", "", "", scope)).toBe("/shop");
    expect(embedLinkDisposition("/create/publish", ORIGIN, scope)).toBe("frame");
    expect(embedLinkDisposition("/marketplace", ORIGIN, scope)).toBe("top");
  });
});

describe("embedLinkDisposition", () => {
  it("frames marketplace links, tops same-origin site links, blanks other origins", () => {
    expect(embedLinkDisposition("/marketplace/abc", ORIGIN)).toBe("frame");
    expect(embedLinkDisposition("https://example.org/shop?tab=cart", ORIGIN)).toBe("frame");
    expect(embedLinkDisposition("/play/?position=1,2", ORIGIN)).toBe("top");
    expect(embedLinkDisposition("/", ORIGIN)).toBe("top");
    expect(embedLinkDisposition("/discover", ORIGIN)).toBe("top");
    expect(embedLinkDisposition("https://opensea.io/x", ORIGIN)).toBe("blank");
    expect(embedLinkDisposition("//evil.example/shop", ORIGIN)).toBe("blank");
    expect(embedLinkDisposition("mailto:hi@example.org", ORIGIN)).toBe("frame");
    expect(embedLinkDisposition("javascript:void(0)", ORIGIN)).toBe("frame");
  });
});

describe("installEmbedLinkGuard", () => {
  const assign = vi.fn();
  const open = vi.fn();
  let uninstall: (() => void) | undefined;

  function guardedWindow(): Window {
    return {
      location: { origin: ORIGIN },
      top: { location: { assign } },
      open,
      addEventListener: window.addEventListener.bind(window),
      removeEventListener: window.removeEventListener.bind(window),
    } as unknown as Window;
  }

  function click(a: Element, init: MouseEventInit = {}) {
    const ev = new MouseEvent("click", { bubbles: true, cancelable: true, button: 0, ...init });
    const bubbled = vi.fn();
    document.body.addEventListener("click", bubbled);
    a.dispatchEvent(ev);
    document.body.removeEventListener("click", bubbled);
    return { prevented: ev.defaultPrevented, bubbled: bubbled.mock.calls.length > 0 };
  }

  function anchor(href: string, attrs: Record<string, string> = {}) {
    const a = document.createElement("a");
    a.href = href;
    for (const [k, v] of Object.entries(attrs)) a.setAttribute(k, v);
    const inner = document.createElement("span");
    inner.textContent = "x";
    a.appendChild(inner);
    document.body.appendChild(a);
    return { a, inner };
  }

  afterEach(() => {
    uninstall?.();
    uninstall = undefined;
    assign.mockReset();
    open.mockReset();
    document.body.innerHTML = "";
  });

  it("lets marketplace links propagate to the router untouched", () => {
    uninstall = installEmbedLinkGuard(guardedWindow());
    const { a } = anchor("https://example.org/marketplace/item-1");
    const r = click(a);
    expect(r.prevented).toBe(false);
    expect(r.bubbled).toBe(true);
    expect(assign).not.toHaveBeenCalled();
  });

  it("sends off-shop same-origin links to the top window and stops the in-frame navigation", () => {
    uninstall = installEmbedLinkGuard(guardedWindow());
    const { inner } = anchor("https://example.org/play/?position=10,-20");
    const r = click(inner);
    expect(r.prevented).toBe(true);
    expect(r.bubbled).toBe(false);
    expect(assign).toHaveBeenCalledWith("https://example.org/play/?position=10,-20");
    expect(open).not.toHaveBeenCalled();
  });

  it("opens other origins in a new tab", () => {
    uninstall = installEmbedLinkGuard(guardedWindow());
    const { a } = anchor("https://opensea.io/collection/x");
    expect(click(a).prevented).toBe(true);
    expect(open).toHaveBeenCalledWith("https://opensea.io/collection/x", "_blank", "noopener");
    expect(assign).not.toHaveBeenCalled();
  });

  it("hands every link leaving a host's scope to the host, Explorer deep links included", () => {
    const host = vi.fn();
    uninstall = installEmbedLinkGuard(guardedWindow(), { scope: ["/create"], open: host });
    expect(click(anchor("https://example.org/create/scenes").a).prevented).toBe(false);
    expect(click(anchor("https://example.org/play/?position=1,2").a).prevented).toBe(true);
    expect(click(anchor("https://opensea.io/x").a).prevented).toBe(true);
    expect(click(anchor("decentraland://realm=main&position=3,4").a).prevented).toBe(true);
    expect(click(anchor("mailto:hi@example.org").a).prevented).toBe(false);
    expect(click(anchor("https://example.org/play?realm=boedo.dcl.eth", { target: "_blank" }).a).prevented).toBe(true);
    expect(click(anchor("https://example.org/create/scenes", { target: "_blank" }).a).prevented).toBe(false);
    expect(host.mock.calls.map(([url]) => url)).toEqual([
      "https://example.org/play/?position=1,2",
      "https://opensea.io/x",
      "decentraland://realm=main&position=3,4",
      "https://example.org/play?realm=boedo.dcl.eth",
    ]);
    expect(assign).not.toHaveBeenCalled();
    expect(open).not.toHaveBeenCalled();
  });

  it("ignores modified clicks, explicit targets, downloads and uninstalls cleanly", () => {
    uninstall = installEmbedLinkGuard(guardedWindow());
    const { a } = anchor("https://example.org/discover");
    expect(click(a, { metaKey: true }).prevented).toBe(false);
    const blank = anchor("https://example.org/discover", { target: "_blank" });
    expect(click(blank.a).prevented).toBe(false);
    const dl = anchor("https://example.org/discover", { download: "" });
    expect(click(dl.a).prevented).toBe(false);
    expect(assign).not.toHaveBeenCalled();
    uninstall();
    uninstall = undefined;
    expect(click(a).prevented).toBe(false);
    expect(assign).not.toHaveBeenCalled();
  });
});

describe("frameEscapeHref", () => {
  function frameAt(loc: Partial<Location> | null): HTMLIFrameElement {
    return { contentWindow: loc ? { location: loc } : null } as unknown as HTMLIFrameElement;
  }

  it("keeps blank, unreadable and in-scope documents", () => {
    expect(frameEscapeHref(null)).toBeNull();
    expect(frameEscapeHref(frameAt(null))).toBeNull();
    expect(frameEscapeHref(frameAt({ href: "about:blank" }))).toBeNull();
    expect(frameEscapeHref(frameAt({ href: "https://example.org/shop?embed=1", protocol: "https:", pathname: "/shop" }))).toBeNull();
    expect(frameEscapeHref(frameAt({ href: "https://example.org/marketplace/cart", protocol: "https:", pathname: "/marketplace/cart" }))).toBeNull();
    const crossOrigin = {
      contentWindow: {
        get location(): Location {
          throw new DOMException("Blocked", "SecurityError");
        },
      },
    } as unknown as HTMLIFrameElement;
    expect(frameEscapeHref(crossOrigin)).toBeNull();
  });

  it("returns the href of a document that left the marketplace", () => {
    expect(frameEscapeHref(frameAt({ href: "https://example.org/play/?position=1,2", protocol: "https:", pathname: "/play/" })))
      .toBe("https://example.org/play/?position=1,2");
    expect(frameEscapeHref(frameAt({ href: "https://example.org/", protocol: "https:", pathname: "/" }))).toBe("https://example.org/");
  });
});

describe("escape relay", () => {
  function framedWindow(parent: object | null) {
    const listeners = new Map<string, EventListener>();
    const win = {
      parent,
      addEventListener: (type: string, fn: EventListener) => listeners.set(type, fn),
      removeEventListener: (type: string) => listeners.delete(type),
    } as unknown as Window;
    const key = (init: KeyboardEventInit & { prevented?: boolean }) => {
      const ev = new KeyboardEvent("keydown", { cancelable: true, ...init });
      if (init.prevented) ev.preventDefault();
      listeners.get("keydown")?.(ev);
    };
    return { win, key, listening: () => listeners.has("keydown") };
  }

  it("posts the escape marker to the parent on an unconsumed Escape only, and uninstalls", () => {
    const postMessage = vi.fn();
    const { win, key, listening } = framedWindow({ postMessage });
    const off = installEmbedEscapeRelay(win);
    key({ key: "a" });
    key({ key: "Escape", prevented: true });
    expect(postMessage).not.toHaveBeenCalled();
    key({ key: "Escape" });
    expect(postMessage).toHaveBeenCalledWith(EMBED_ESCAPE_MESSAGE, "*");
    off();
    expect(listening()).toBe(false);
  });

  it("stays quiet at the top window", () => {
    const top = framedWindow(null);
    (top.win as unknown as { parent: Window }).parent = top.win;
    installEmbedEscapeRelay(top.win);
    expect(() => top.key({ key: "Escape" })).not.toThrow();
  });

  it("accepts the marker only from the frame's own window", () => {
    const frame = document.createElement("iframe");
    document.body.appendChild(frame);
    const from = frame.contentWindow;
    const msg = (data: unknown, source: unknown) => ({ data, source }) as unknown as MessageEvent;
    expect(isEmbedEscapeMessage(msg(EMBED_ESCAPE_MESSAGE, from), frame)).toBe(true);
    expect(isEmbedEscapeMessage(msg(EMBED_ESCAPE_MESSAGE, window), frame)).toBe(false);
    expect(isEmbedEscapeMessage(msg("other", from), frame)).toBe(false);
    expect(isEmbedEscapeMessage(msg(EMBED_ESCAPE_MESSAGE, from), null)).toBe(false);
    frame.remove();
  });
});

describe("boot script", () => {
  afterEach(() => document.documentElement.removeAttribute("data-embed"));

  it("marks the document embedded when framed, and not otherwise", () => {
    expect(isEmbedded()).toBe(false);
    const run = (self: object, top: object) =>
      new Function("self", "top", "document", EMBED_BOOT_SCRIPT)(self, top, document);
    const w = {};
    run(w, w);
    expect(isEmbedded()).toBe(false);
    run(w, {});
    expect(isEmbedded()).toBe(true);
  });
});
