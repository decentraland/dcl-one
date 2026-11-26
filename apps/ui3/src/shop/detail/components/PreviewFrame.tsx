// The Shop's avatars in the wearable-preview iframe, the page decentraland.org's Shop and Sites' creators preview also
// embed. Its options travel in the URL it mounts with, and later changes as `update` messages (the @dcl/schemas
// preview protocol), so a change of view or of items reloads the scene in place instead of the iframe.

import { useEffect, useRef, useState } from "react";

import styles from "./PreviewFrame.module.css";

const MAINNET_PREVIEW = "https://wearable-preview.decentraland.org";
const TESTNET_PREVIEW = "https://wearable-preview.decentraland.zone";

export type PreviewRendererName = "unity" | "babylon";

export type PreviewFrameOptions = {
  urns: string[];
  /** Item definitions sent whole, for what the preview cannot name: a published emote (lib/emoteBase64). */
  base64s?: string[];
  profile?: string;
  bodyShape?: string;
  skin?: string;
  hair?: string;
  eyes?: string;
  type?: "avatar" | "wearable";
  emote?: string;
  disableBackground?: boolean;
  disableDefaultEmotes?: boolean;
  disableFadeEffect?: boolean;
  /** The camera's distance, as a percentage of the framed view (Babylon). */
  zoom?: number;
  wheelZoom?: number;
  wheelStart?: number;
  unity?: boolean;
  /** Unity's mode: `marketplace` for an item on an avatar, `builder` for a mannequin dressed in a set of items. */
  unityMode?: "marketplace" | "builder";
};

// The option each prop travels as in the URL, where its name differs (decentraland-ui2's WearablePreview).
const PARAM_NAMES: Record<string, string> = { urns: "urn", base64s: "base64", unityMode: "mode" };

/**
 * How long Unity may go without drawing or failing before the page falls back to Babylon: when it cannot read its config
 * it shows the error on its own canvas and tells the page nothing. The three.js preview it replaced allowed a load the
 * same 45 seconds.
 */
export const UNITY_LOAD_TIMEOUT_MS = 45_000;

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

// The renderer and its mode are picked once, from the mount URL; a different one mounts again. An update never carries
// them: Unity has no mapping for `unity` there, and logs each one it is sent.
const RENDERER_KEYS = ["unity", "unityMode"] as const;

// The options Unity applies from an update (wearable-preview@6f1e470 src/lib/unity/messages.ts: PROPERTY_METHOD_MAP and
// SPECIAL_PROPERTIES). Any other, `disableFadeEffect` among them, reaches Unity only through the mount URL: sent in an
// update it is logged as unmapped, and a change to it mounts again.
const UNITY_UPDATE_KEYS = new Set(["urns", "base64s", "profile", "bodyShape", "skin", "hair", "eyes", "type", "emote", "disableBackground", "disableDefaultEmotes", "wheelZoom", "wheelStart"]);

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
  if (mount.options.unity !== true) return false;
  const value = (from: PreviewFrameOptions, key: string) => (isSet(from[key as keyof PreviewFrameOptions]) ? JSON.stringify(from[key as keyof PreviewFrameOptions]) : undefined);
  const mountOnly = new Set([...Object.keys(mount.options), ...Object.keys(options)].filter((key) => !UNITY_UPDATE_KEYS.has(key) && !(RENDERER_KEYS as readonly string[]).includes(key)));
  if ([...mountOnly].some((key) => value(options, key) !== value(mount.options, key))) return true;
  return [...held].some((key) => !isSet(options[key as keyof PreviewFrameOptions]));
}

function setKeys(options: PreviewFrameOptions): Set<string> {
  return new Set(Object.entries(options).flatMap(([key, value]) => (isSet(value) ? [key] : [])));
}

/**
 * The update for these options. Babylon lays an update over the mount URL's options, so a key the URL set and these
 * options leave unset goes as an own property holding undefined (postMessage keeps it): left out, the URL's value would
 * come back. Unity skips such a key, which needsMount has already ruled out.
 */
function updateFor(mount: Mount, options: PreviewFrameOptions): Record<string, unknown> {
  const update: Record<string, unknown> = {};
  for (const key of setKeys(mount.options)) update[key] = undefined;
  Object.assign(update, options);
  for (const key of RENDERER_KEYS) delete update[key];
  if (mount.options.unity === true) for (const key of Object.keys(update)) if (!UNITY_UPDATE_KEYS.has(key)) delete update[key];
  return update;
}

/**
 * How long an update under Unity waits for the answer to the scene before it. Unity answers every reload with a `load`,
 * a superseded one included (Babylon drops those itself), so it is sent one scene at a time to tell which scene an answer
 * is for. A slow scene should not hold the next one for long: past this the next one mounts a fresh iframe instead, whose
 * answers cannot be confused with the retired one's. (A change only to the camera reloads nothing and gets no answer, so
 * the change after it mounts again too; the Shop makes none.)
 */
export const UNITY_ANSWER_WAIT_MS = 3_000;

/**
 * What one mounted iframe has said and been told. A new mount starts a new one, so the retired iframe's are dropped.
 * `awaiting`: the options of the scene Unity is drawing and has not answered for, sent at `sentAt`.
 */
type Live = { n: number; ready: boolean; answered: boolean; delivered: string; held: Set<string>; awaiting: string | null; sentAt: number };

const liveFor = (n: number, serialized: string, options: PreviewFrameOptions): Live => ({
  n,
  ready: false,
  answered: false,
  delivered: serialized,
  held: setKeys(options),
  awaiting: serialized,
  sentAt: performance.now(),
});

type Props = {
  options: PreviewFrameOptions;
  /** Each change after the first plays the emote again, through the preview's controller. */
  replay?: number;
  /**
   * How long a mount may go without its first `load` or `error` before it counts as failed. The preview does not always
   * say: Unity whose config cannot be read shows the error on its own canvas and posts nothing. Only the mount's first
   * answer is timed, not the reload an update starts, and only while the page is visible: a hidden tab runs no animation
   * frames, so the preview cannot draw there.
   */
  loadTimeoutMs?: number;
  onLoad: (renderer: PreviewRendererName | undefined) => void;
  onError: (message: string) => void;
  /**
   * The frame put a fresh iframe in place of its own: for a change an update cannot carry, a Unity scene that answered too
   * late, or Unity booting again. Nothing is drawn in it yet, whatever the options.
   */
  onReplaced?: () => void;
  /**
   * Ambient playback, as decentraland.org's home page promo tiles run it (shop@32377959 LivePromo's PreviewTuning): once a
   * scene has drawn its sound is off, and its emote plays again each time it ends, through the preview's controller.
   */
  ambient?: boolean;
};

export default function PreviewFrame({ options, replay = 0, loadTimeoutMs, onLoad, onError, onReplaced, ambient = false }: Props) {
  const frame = useRef<HTMLIFrameElement>(null);
  const serialized = JSON.stringify(options);
  // The iframe keeps the URL it mounted with: a new `src` reloads the whole preview, not only its scene. It mounts again
  // only for a change an update cannot carry (needsMount).
  const [mount, setMount] = useState(() => mountWith(0, options));
  const origin = new URL(mount.base).origin;
  // `held`: the options a Unity preview holds a value for, the mount URL's plus every one an update has set since.
  const live = useRef<Live>(liveFor(0, serialized, options));
  // The options the page shows now: an answer counts only for them.
  const latest = useRef(serialized);
  latest.current = serialized;
  const latestOptions = useRef(options);
  latestOptions.current = options;
  const callbacks = useRef({ onLoad, onError, onReplaced, ambient });
  callbacks.current = { onLoad, onError, onReplaced, ambient };
  const retry = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(retry.current), []);

  const post = (type: string, payload: unknown) => frame.current?.contentWindow?.postMessage({ type, payload }, origin);
  const requests = useRef(0);
  const control = (method: string) => post("controller_request", { id: `shop-preview-${method}-${++requests.current}`, namespace: "emote", method, params: [] });
  const push = useRef(() => undefined as void);
  push.current = () => {
    const now = live.current;
    if (now.n !== mount.n || now.delivered === serialized) return;
    if (needsMount(mount, now.held, options)) {
      // The iframe retires here, not when React replaces it: nothing it says from now on is the new mount's.
      live.current = liveFor(mount.n + 1, serialized, options);
      setMount(mountWith(mount.n + 1, options));
      return;
    }
    if (!now.ready) return;
    if (mount.options.unity === true && now.awaiting !== null) {
      // Unity applies no update before its first scene has loaded, so until that scene answers (or the load timeout
      // gives up on it) an update waits for it however long it takes.
      if (!now.answered) return;
      const waited = performance.now() - now.sentAt;
      if (waited < UNITY_ANSWER_WAIT_MS) {
        // Sent once the scene before it answers, or once the wait is over.
        window.clearTimeout(retry.current);
        retry.current = window.setTimeout(() => push.current(), UNITY_ANSWER_WAIT_MS - waited);
        return;
      }
      // The scene it is drawing may still answer, for options the page has left: these go to a fresh iframe instead.
      live.current = liveFor(mount.n + 1, serialized, options);
      setMount(mountWith(mount.n + 1, options));
      return;
    }
    now.delivered = serialized;
    now.awaiting = serialized;
    now.sentAt = performance.now();
    const update = updateFor(mount, options);
    for (const key of setKeys(update as PreviewFrameOptions)) now.held.add(key);
    post("update", { options: update });
  };

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      // Another preview on the page speaks the same protocol: only this iframe's messages count.
      const now = live.current;
      if (now.n !== mount.n || event.source !== frame.current?.contentWindow || event.origin !== origin) return;
      const { type, payload } = (event.data ?? {}) as { type?: string; payload?: { renderer?: string; message?: string; type?: string } };
      if (type === "emote_event") {
        if (callbacks.current.ambient && payload?.type === "animation_end") control("play");
      } else if (type === "ready" && now.ready && mount.options.unity === true) {
        // Unity booting again has lost its scene, and its answer to the first one belongs to the boot before: a fresh
        // iframe with the current options starts over, with a deadline of its own.
        live.current = liveFor(mount.n + 1, latest.current, latestOptions.current);
        setMount(mountWith(mount.n + 1, latestOptions.current));
      } else if (type === "ready") {
        // Every boot, the first or (under Babylon) a reload of the frame, starts again from the URL: what it has lost goes
        // again. What it draws first is the mount URL's scene.
        now.ready = true;
        now.delivered = mount.serialized;
        now.held = setKeys(mount.options);
        now.awaiting = mount.serialized;
        now.sentAt = performance.now();
        push.current();
      } else if (type === "load" || type === "error") {
        const first = !now.answered;
        now.answered = true;
        // Under Unity an answer is for the scene it was drawing; one for options the page has moved on from is not the
        // page's. Babylon answers only for the scene it was last sent (Preview.tsx drops a superseded one).
        const answeredFor = now.awaiting;
        now.awaiting = null;
        // Except a failed first scene: Unity that could not start applies no update after it, whatever options it was for.
        const failedToStart = type === "error" && first;
        if (mount.options.unity !== true || answeredFor === null || answeredFor === latest.current || failedToStart) {
          if (type === "load") {
            if (callbacks.current.ambient) control("disableSound");
            const renderer = payload?.renderer;
            callbacks.current.onLoad(renderer === "unity" || renderer === "babylon" ? renderer : undefined);
          } else callbacks.current.onError(payload?.message ?? "");
        }
        if (!(mount.options.unity === true && failedToStart)) push.current();
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [origin, mount]);

  useEffect(() => {
    if (loadTimeoutMs === undefined) return;
    // The visible time left, and whether the timeout has run: a page hidden just as the time ran out, before the
    // timeout could run, still has it to run (at once) when it is shown again; one that has run never runs again.
    // Visible time is measured on the monotonic clock, so a change to the wall clock neither spends nor extends it.
    let left = loadTimeoutMs;
    let expired = false;
    let since = 0;
    let timer: number | undefined;
    const expire = () => {
      timer = undefined;
      expired = true;
      const now = live.current;
      if (now.n === mount.n && !now.answered) callbacks.current.onError("The preview did not load in time.");
    };
    const run = () => {
      since = performance.now();
      timer = window.setTimeout(expire, left);
    };
    const pause = () => {
      window.clearTimeout(timer);
      timer = undefined;
      left = Math.max(0, left - (performance.now() - since));
    };
    const onVisibility = () => {
      if (document.hidden) {
        if (timer !== undefined) pause();
      } else if (timer === undefined && !expired) run();
    };
    if (!document.hidden) run();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      window.clearTimeout(timer);
    };
  }, [mount, loadTimeoutMs]);

  useEffect(() => {
    push.current();
  }, [serialized]);

  useEffect(() => {
    if (mount.n > 0) callbacks.current.onReplaced?.();
  }, [mount.n]);

  const played = useRef(replay);
  useEffect(() => {
    if (replay === played.current) return;
    played.current = replay;
    if (live.current.n === mount.n && live.current.ready) control("play");
  }, [replay]);

  return <iframe key={mount.n} ref={frame} className={styles.frame} src={mount.src} allow="autoplay" title="3D preview" />;
}
