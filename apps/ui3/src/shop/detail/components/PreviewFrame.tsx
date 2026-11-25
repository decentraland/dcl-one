// The item page's avatar in the wearable-preview iframe, the page decentraland.org's Shop and Sites' creators preview
// also embed. Its options travel in the URL it mounts with, and later changes as `update` messages (the
// @dcl/schemas preview protocol), so a change of view reloads the scene in place instead of the iframe.

import { useEffect, useRef, useState } from "react";

const MAINNET_PREVIEW = "https://wearable-preview.decentraland.org";
const TESTNET_PREVIEW = "https://wearable-preview.decentraland.zone";

export type PreviewRendererName = "unity" | "babylon";

export type PreviewFrameOptions = {
  urns: string[];
  profile?: string;
  bodyShape?: string;
  type?: "avatar" | "wearable";
  emote?: string;
  disableBackground?: boolean;
  wheelZoom?: number;
  wheelStart?: number;
  unity?: boolean;
  unityMode?: "marketplace";
};

// The option each prop travels as in the URL, where its name differs (decentraland-ui2's WearablePreview).
const PARAM_NAMES: Record<string, string> = { urns: "urn", unityMode: "mode" };

/** The preview that resolves these items: a testnet item is only on the testnet peers the .zone preview reads. */
export function previewBase(urns: string[]): string {
  return urns.some((urn) => /^urn:decentraland:(amoy|sepolia|mumbai|goerli):/i.test(urn)) ? TESTNET_PREVIEW : MAINNET_PREVIEW;
}

export function previewUrl(base: string, options: PreviewFrameOptions): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(options)) {
    if (value === undefined || value === null || value === "") continue;
    for (const item of Array.isArray(value) ? value : [value]) params.append(PARAM_NAMES[key] ?? key, String(item));
  }
  if (base === TESTNET_PREVIEW) params.set("env", "dev");
  return `${base}/?${params}`;
}

// Options that pick the renderer: they are read once, from the mount URL. Unity has no mapping for them in an update, and
// logs each one it is sent.
const RENDERER_KEYS = ["unity", "unityMode"] as const;

type Mount = { n: number; base: string; src: string; options: PreviewFrameOptions; serialized: string };

function mountWith(n: number, options: PreviewFrameOptions): Mount {
  const base = previewBase(options.urns);
  return { n, base, src: previewUrl(base, options), options, serialized: JSON.stringify(options) };
}

const isSet = (value: unknown) => value !== undefined && value !== null && value !== "";

/**
 * Whether these options can reach the mounted preview as an update, or need a new mount: another preview origin, another
 * renderer, or, under Unity, an option it holds that they leave unset. Unity applies an update key by key and skips an
 * unset one, so the option would keep its old value there (Babylon takes the update whole).
 */
function needsMount(mount: Mount, held: ReadonlySet<string>, options: PreviewFrameOptions): boolean {
  if (previewBase(options.urns) !== mount.base) return true;
  if (RENDERER_KEYS.some((key) => options[key] !== mount.options[key])) return true;
  return mount.options.unity === true && [...held].some((key) => !isSet(options[key as keyof PreviewFrameOptions]));
}

function setKeys(options: PreviewFrameOptions): Set<string> {
  return new Set(Object.entries(options).flatMap(([key, value]) => (isSet(value) ? [key] : [])));
}

type Props = {
  options: PreviewFrameOptions;
  /** Each change after the first plays the emote again, through the preview's controller. */
  replay?: number;
  /**
   * How long a mount may go without its first `load` or `error` before it counts as failed. The preview does not always
   * say: Unity whose config cannot be read shows the error on its own canvas and posts nothing.
   */
  loadTimeoutMs?: number;
  onLoad: (renderer: PreviewRendererName | undefined) => void;
  onError: (message: string) => void;
};

export default function PreviewFrame({ options, replay = 0, loadTimeoutMs, onLoad, onError }: Props) {
  const frame = useRef<HTMLIFrameElement>(null);
  const serialized = JSON.stringify(options);
  // The iframe keeps the URL it mounted with: a new `src` reloads the whole preview, not only its scene. It mounts again
  // only for a change an update cannot carry (needsMount).
  const [mount, setMount] = useState(() => mountWith(0, options));
  const origin = new URL(mount.base).origin;
  const ready = useRef(false);
  const delivered = useRef(serialized);
  // The options a Unity preview holds a value for: the mount URL's, plus every one an update has set since.
  const held = useRef(setKeys(options));
  const answered = useRef(false);
  const callbacks = useRef({ onLoad, onError });
  callbacks.current = { onLoad, onError };

  const post = (type: string, payload: unknown) => frame.current?.contentWindow?.postMessage({ type, payload }, origin);
  const push = useRef(() => undefined as void);
  push.current = () => {
    if (delivered.current === serialized) return;
    if (needsMount(mount, held.current, options)) {
      ready.current = false;
      answered.current = false;
      delivered.current = serialized;
      held.current = setKeys(options);
      setMount(mountWith(mount.n + 1, options));
      return;
    }
    if (!ready.current) return;
    delivered.current = serialized;
    const update: PreviewFrameOptions = { ...options };
    for (const key of RENDERER_KEYS) delete update[key];
    for (const key of setKeys(update)) held.current.add(key);
    post("update", { options: update });
  };

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      // Another preview on the page speaks the same protocol: only this iframe's messages count.
      if (event.source !== frame.current?.contentWindow || event.origin !== origin) return;
      const { type, payload } = (event.data ?? {}) as { type?: string; payload?: { renderer?: string; message?: string } };
      if (type === "ready") {
        // Every boot, the first or a reload of the frame, starts again from the URL: what it has lost goes again.
        ready.current = true;
        delivered.current = mount.serialized;
        held.current = setKeys(mount.options);
        push.current();
      } else if (type === "load") {
        answered.current = true;
        const renderer = payload?.renderer;
        callbacks.current.onLoad(renderer === "unity" || renderer === "babylon" ? renderer : undefined);
      } else if (type === "error") {
        answered.current = true;
        callbacks.current.onError(payload?.message ?? "");
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [origin, mount]);

  useEffect(() => {
    if (loadTimeoutMs === undefined) return;
    const timer = window.setTimeout(() => {
      if (!answered.current) callbacks.current.onError("The preview did not load in time.");
    }, loadTimeoutMs);
    return () => window.clearTimeout(timer);
  }, [mount, loadTimeoutMs]);

  useEffect(() => {
    push.current();
  }, [serialized]);

  const played = useRef(replay);
  useEffect(() => {
    if (replay === played.current) return;
    played.current = replay;
    if (ready.current) post("controller_request", { id: `shop-item-preview-${replay}`, namespace: "emote", method: "play", params: [] });
  }, [replay]);

  return <iframe key={mount.n} ref={frame} className="shop-preview__frame" src={mount.src} allow="autoplay" title="3D preview" />;
}
