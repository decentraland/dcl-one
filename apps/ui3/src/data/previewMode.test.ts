import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { claimPreviewToken, forgetPreviewToken, isReloadFrame, previewHost, previewLaunch, previewSocketUrl, previewToken, subscribePreviewStatus, toPreviewManifest } from "./previewMode";

afterEach(() => { forgetPreviewToken(); window.history.replaceState(null, "", "/"); });

describe("preview launch", () => {
  it("recognises the SDK launch URL and derives the preview server origin", () => {
    const launch = previewLaunch("?position=3%2C12&preview=true&realm=http%3A%2F%2Flocalhost%3A8002");
    expect(launch).toEqual({ preview: true, realm: "http://localhost:8002", origin: "http://localhost:8002", assistant: false });
    expect(previewLaunch("?preview=true&realm=http://localhost:8002&assistant=1").assistant).toBe(true);
    expect(previewLaunch("?preview=true&realm=tophub.dcl.eth&assistant=1").assistant).toBe(false);
    expect(previewHost(launch.origin)).toBe("localhost:8002");
    expect(previewSocketUrl(launch.origin!)).toBe("ws://localhost:8002/");
    expect(previewLaunch("?realm=http://localhost:8002/about&preview=1").origin).toBe("http://localhost:8002");
    expect(previewLaunch("?realm=main").preview).toBe(false);
    expect(previewLaunch("?preview=true&editorUi=1").preview).toBe(false);
    expect(previewLaunch("?preview=true&realm=tophub.dcl.eth")).toEqual({ preview: true, realm: "tophub.dcl.eth", origin: null, assistant: false });
    // The engine drops `realm` from the address bar when it is the bundle's default, the page's
    // own origin: the player dcl-scene-cli embeds previews the SDK that serves it.
    expect(previewLaunch("?preview=true&position=2%2C12&assistant=1")).toEqual({ preview: true, realm: window.location.origin, origin: window.location.origin, assistant: true });
  });

  it("claims the SDK's capability token from the launch fragment, keeps it in memory and off the URL", () => {
    expect(previewToken()).toBeNull();
    expect(claimPreviewToken("")).toEqual({ token: null, rest: "" });
    expect(claimPreviewToken("#mcptoken=m1")).toEqual({ token: null, rest: "mcptoken=m1" });
    expect(previewToken()).toBeNull();
    window.history.replaceState(null, "", "/?preview=true&realm=http://localhost:8002#mcptoken=m1&token=abc-DEF_123");
    const launch = previewLaunch(window.location.search);
    expect(launch).toEqual({ preview: true, realm: "http://localhost:8002", origin: "http://localhost:8002", assistant: false });
    expect(previewToken()).toBe("abc-DEF_123");
    expect(window.location.hash).toBe("#mcptoken=m1");
    expect(window.location.search).toBe("?preview=true&realm=http://localhost:8002");
    expect(localStorage.length).toBe(0);
    // Later boots (the lobby, the realm map) see no fragment and leave the claimed token alone.
    expect(previewLaunch(window.location.search).preview).toBe(true);
    expect(previewToken()).toBe("abc-DEF_123");
    forgetPreviewToken();
    // Outside a preview the fragment is not the SDK's: nothing is claimed or rewritten.
    window.history.replaceState(null, "", "/?realm=main#token=zzz");
    expect(previewLaunch(window.location.search).preview).toBe(false);
    expect(previewToken()).toBeNull();
    expect(window.location.hash).toBe("#token=zzz");
  });

  it("only counts the SDK's SCENE_UPDATE frames as reloads", () => {
    expect(isReloadFrame(JSON.stringify({ type: "SCENE_UPDATE", payload: { sceneId: "s", sceneType: "scene" } }))).toBe(true);
    expect(isReloadFrame("ping")).toBe(false);
    expect(isReloadFrame(new ArrayBuffer(2))).toBe(false);
  });
});

describe("scene manifest", () => {
  it("turns SDK spawn points into parcels and world metres from the base parcel", () => {
    const manifest = toPreviewManifest({
      display: { title: " Hexabricks " },
      scene: { base: "-3,12", parcels: ["-3,12", "-2,12"] },
      spawnPoints: [
        { name: "gate", default: true, position: { x: [2, 6], y: 0, z: 3 } },
        { position: { x: 20, y: [1, 3], z: 8 } },
      ],
    });
    expect(manifest?.title).toBe("Hexabricks");
    expect(manifest?.parcels).toBe(2);
    expect(manifest?.spawns).toEqual([
      { name: "gate", isDefault: true, parcel: [-3, 12], world: { x: -48 + 4, y: 0, z: 192 + 3 } },
      { name: "Spawn 2", isDefault: false, parcel: [-2, 12], world: { x: -48 + 20, y: 2, z: 192 + 8 } },
    ]);
    expect(toPreviewManifest({ scene: { parcels: ["4,4"] } })?.base).toEqual([4, 4]);
    expect(toPreviewManifest({ display: { title: "x" } })).toBeNull();
    expect(toPreviewManifest(null)).toBeNull();
    vi.restoreAllMocks();
  });
});

describe("shared preview link", () => {
  class FakeSocket {
    static instances: FakeSocket[] = [];
    readyState = 0; closed = false; onopen: (() => void) | null = null; onmessage: ((e: { data: unknown }) => void) | null = null; onclose: (() => void) | null = null; onerror: (() => void) | null = null;
    constructor(public url: string) { FakeSocket.instances.push(this); }
    close() { this.closed = true; this.readyState = 3; }
    open() { this.readyState = 1; this.onopen?.(); }
  }
  beforeEach(() => { FakeSocket.instances = []; vi.useFakeTimers(); vi.stubGlobal("WebSocket", FakeSocket); });
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

  it("opens one socket per origin for any number of subscribers and closes it after the last leaves", () => {
    const seenA: unknown[] = []; const seenB: unknown[] = [];
    const offA = subscribePreviewStatus("http://localhost:8002", (s) => seenA.push(s.connected));
    const offB = subscribePreviewStatus("http://localhost:8002", (s) => seenB.push(s.reloads));
    expect(FakeSocket.instances).toHaveLength(1);
    FakeSocket.instances[0]!.open();
    FakeSocket.instances[0]!.onmessage?.({ data: JSON.stringify({ type: "SCENE_UPDATE", payload: {} }) });
    expect(seenA.at(-1)).toBe(true);
    expect(seenB.at(-1)).toBe(1);
    offA();
    vi.runAllTimers();
    expect(FakeSocket.instances[0]!.closed).toBe(false);
    offB();
    vi.runAllTimers();
    expect(FakeSocket.instances[0]!.closed).toBe(true);
  });

  it("keeps its socket through an unmount and remount, as React's development mode does", () => {
    const off = subscribePreviewStatus("http://localhost:8003", () => {});
    off();
    const again = subscribePreviewStatus("http://localhost:8003", () => {});
    vi.runAllTimers();
    expect(FakeSocket.instances).toHaveLength(1);
    expect(FakeSocket.instances[0]!.closed).toBe(false);
    FakeSocket.instances[0]!.open();
    again();
    vi.runAllTimers();
    expect(FakeSocket.instances[0]!.closed).toBe(true);
  });

  it("closes a socket that is still connecting only once it opens, and does not reconnect", () => {
    const off = subscribePreviewStatus("http://localhost:8004", () => {});
    off();
    vi.runAllTimers();
    const socket = FakeSocket.instances[0]!;
    expect(socket.closed).toBe(false);
    socket.open();
    expect(socket.closed).toBe(true);
    socket.onclose?.();
    vi.runAllTimers();
    expect(FakeSocket.instances).toHaveLength(1);
  });
});
