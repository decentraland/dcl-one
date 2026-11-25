// The item page's avatar in the wearable-preview iframe, the page decentraland.org's Shop and Sites' creators preview
// also embed. Its options travel in the URL it mounts with, and later changes as `update` messages (the
// @dcl/schemas preview protocol), so a change of view reloads the scene in place instead of the iframe.

import { useEffect, useRef } from "react";

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

type Props = {
  options: PreviewFrameOptions;
  /** Each change after the first plays the emote again, through the preview's controller. */
  replay?: number;
  onLoad: (renderer: PreviewRendererName | undefined) => void;
  onError: (message: string) => void;
};

export default function PreviewFrame({ options, replay = 0, onLoad, onError }: Props) {
  const frame = useRef<HTMLIFrameElement>(null);
  const base = previewBase(options.urns);
  const origin = new URL(base).origin;
  const serialized = JSON.stringify(options);
  // The iframe keeps the URL it mounted with: a new `src` would reload the whole preview, not only its scene.
  const src = useRef(previewUrl(base, options)).current;
  const ready = useRef(false);
  const delivered = useRef(serialized);
  const callbacks = useRef({ onLoad, onError });
  callbacks.current = { onLoad, onError };

  const post = (type: string, payload: unknown) => frame.current?.contentWindow?.postMessage({ type, payload }, origin);
  const push = useRef(() => undefined as void);
  push.current = () => {
    if (!ready.current || delivered.current === serialized) return;
    delivered.current = serialized;
    post("update", { options });
  };

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      // Another preview on the page speaks the same protocol: only this iframe's messages count.
      if (event.source !== frame.current?.contentWindow || event.origin !== origin) return;
      const { type, payload } = (event.data ?? {}) as { type?: string; payload?: { renderer?: string; message?: string } };
      if (type === "ready") {
        ready.current = true;
        push.current();
      } else if (type === "load") {
        const renderer = payload?.renderer;
        callbacks.current.onLoad(renderer === "unity" || renderer === "babylon" ? renderer : undefined);
      } else if (type === "error") {
        callbacks.current.onError(payload?.message ?? "");
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [origin]);

  useEffect(() => {
    push.current();
  }, [serialized]);

  const played = useRef(replay);
  useEffect(() => {
    if (replay === played.current) return;
    played.current = replay;
    if (ready.current) post("controller_request", { id: `shop-item-preview-${replay}`, namespace: "emote", method: "play", params: [] });
  }, [replay]);

  return <iframe ref={frame} className="shop-preview__frame" src={src} allow="autoplay" title="3D preview" />;
}
