// Shop item preview with avatar, wearable, and emote states. On the web the avatar is the wearable-preview iframe that
// decentraland.org's Shop embeds; inside the Explorer it is the Explorer's own preview host, fed the worn outfit.

import { useEffect, useState } from "react";

import { fetchProfile } from "../../../data/catalyst/profile";
import type { AvatarStatus } from "../../../wearable-preview/avatar";
import WearablePreview from "../../../wearable-preview/WearablePreview";
import type { OutfitData } from "../../../wearable-preview/outfit";
import { t } from "@shop/discovery/intl";
import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { useShopWallet } from "../../wallet/context";
import { avatarShape as profileShape, BASE_MALE, itemShapes, shapeLabel, type BodyShapeUrn } from "@shop/detail/lib/bodyShape";
import { pickRenderer } from "../lib/pickRenderer";
import { itemUrn } from "@shop/detail/lib/urn";
import { fetchWearableRules, keepEquipped, sharedEntityBase } from "../lib/wearable-rules";
import { fetchEmoteClipUrl } from "../lib/emoteBase64";
import { useShopQuery } from "../../discovery/query";
import { REMOTE_FLAG, useRemoteFlagState } from "../../discovery/lib/remoteFlags";
import PreviewFrame, { previewBase, UNITY_LOAD_TIMEOUT_MS, type PreviewFrameOptions, type PreviewRendererName } from "./PreviewFrame";
import "./itempreview.css";

const VIEW_TEXT = { previewMode: "Preview mode", onAvatar: "On avatar", itemView: "Item", playEmote: "Play emote" };

// The Explorer draws the preview itself and announces it before it is ready (bevy-host.ts).
const insideExplorer = () => typeof window !== "undefined" && (!!window.dclAvatarPreview || !!window.dclAvatarPreviewExpected);

function ViewToggle({ view, onView }: { view: "avatar" | "item"; onView: (view: "avatar" | "item") => void }) {
  return (
    <div className="shop-preview__toggle" role="group" aria-label={VIEW_TEXT.previewMode} data-testid="preview-toggle">
      <button type="button" data-active={view === "avatar" || undefined} aria-pressed={view === "avatar"} aria-label={VIEW_TEXT.onAvatar} onClick={() => onView("avatar")}>
        <span className="shop-preview__toggle-label">{VIEW_TEXT.onAvatar}</span>
        <span className="shop-preview__toggle-ico" aria-hidden>
          <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor"><circle cx="10" cy="5" r="3" /><path d="M4 18c0-4 2.7-7 6-7s6 3 6 7z" /></svg>
        </span>
      </button>
      <button type="button" data-active={view === "item" || undefined} aria-pressed={view === "item"} aria-label={VIEW_TEXT.itemView} onClick={() => onView("item")}>
        <span className="shop-preview__toggle-label">{VIEW_TEXT.itemView}</span>
        <span className="shop-preview__toggle-ico" aria-hidden>
          <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor"><path d="M7 3 3 5.5 4.5 9 6 8.3V17h8V8.3l1.5.7L17 5.5 13 3c-.5 1.3-1.6 2-3 2s-2.5-.7-3-2z" /></svg>
        </span>
      </button>
    </div>
  );
}

function EmotePlaybar({ replay, onReplay }: { replay: number; onReplay: () => void }) {
  return (
    <div className="shop-preview__playbar" data-testid="emote-playbar">
      <button type="button" className="shop-preview__play" onClick={onReplay} aria-label={VIEW_TEXT.playEmote}>
        <svg viewBox="0 0 20 20" width="16" height="16" fill="currentColor" aria-hidden><path d="M6 4v12l10-6z" /></svg>
      </button>
      <span className="shop-preview__track" aria-hidden>
        <span key={replay} className="shop-preview__knob" />
      </span>
    </div>
  );
}

const wornItemUrn = (urn: string) => {
  const p = urn.split(":");
  return p.length === 7 && /^collections-v[12]$/.test(p[3] ?? "") ? p.slice(0, 6).join(":") : urn;
};

type WearerProfile = Awaited<ReturnType<typeof fetchProfile>>;

async function fetchWornOutfit(profile: WearerProfile, tryOn: { urn: string; category: string } | null, base: string | undefined): Promise<OutfitData | null> {
  const avatar = profile?.avatar;
  if (!avatar?.bodyShape) return null;
  const worn = [...new Set((avatar.wearables ?? []).map(wornItemUrn))];
  // The body and the item join this read; the preview, given the same base, mounts them next from the same entity cache.
  const rules = await fetchWearableRules(worn, [avatar.bodyShape, ...(tryOn ? [tryOn.urn] : [])], base);
  const kept = new Set(keepEquipped(rules, tryOn ? [tryOn.category] : []).map((u) => u.toLowerCase()));
  const known = new Set(rules.map((r) => r.urn.toLowerCase()));
  const wearables = worn.filter((u) => kept.has(u.toLowerCase()) || !known.has(u.toLowerCase()));
  return {
    bodyShape: avatar.bodyShape,
    wearables: tryOn ? [...wearables, tryOn.urn] : wearables,
    skin: avatar.skin ?? undefined,
    hair: avatar.hair ?? undefined,
    eyes: avatar.eyes ?? undefined,
  };
}

function defaultProfileFor(key: string): string {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return `default${(h % 159) + 1}`;
}

type ItemPreviewProps = { item: CatalogItem; avatarShape?: BodyShapeUrn | null; renderer?: "full" | "lite" };

function ExplorerItemPreview({ item, avatarShape = null, renderer }: ItemPreviewProps) {
  const [show3d, setShow3d] = useState(renderer === "full");
  useEffect(() => {
    if (!renderer && pickRenderer().reason !== "no-webgl") setShow3d(true);
  }, [renderer]);
  const [status, setStatus] = useState<AvatarStatus>("loading");
  const [view, setView] = useState<"avatar" | "item">("avatar");
  const [replay, setReplay] = useState(0);
  const urn = item.urn ?? itemUrn(item);
  const canPreview = !!urn && item.category !== "ens";
  const isEmote = item.category === "emote";
  const { account } = useShopWallet();
  const tryOn = isEmote ? null : { urn: urn!, category: item.wearableCategory ?? "" };
  const wearer = account ?? defaultProfileFor(urn ?? item.id);
  const profile = useShopQuery(canPreview && show3d ? ["wearer-profile", wearer] : null, (signal) => fetchProfile(wearer, { signal }), { staleMs: 5 * 60_000 });
  // One Catalyst for the outfit's rules read and the scene, so the scene finds the entities that read cached, when one
  // peer serves the whole outfit. Otherwise each keeps its own: the rules read its network's peer, the scene its default.
  const base = urn && profile.data !== undefined ? sharedEntityBase([urn, profile.data?.avatar?.bodyShape ?? "", ...(profile.data?.avatar?.wearables ?? [])].filter(Boolean)) : undefined;
  const worn = useShopQuery(profile.data !== undefined ? ["worn-outfit", wearer, urn, tryOn?.category ?? null] : null, () => fetchWornOutfit(profile.data!, tryOn, base), { staleMs: 5 * 60_000 });
  const waiting = canPreview && show3d && (profile.isLoading || worn.isLoading);
  const wornShape = profileShape(worn.data ? { avatar: worn.data } : null);
  const shopperShape = avatarShape ?? (account ? wornShape : null);
  const shapes = itemShapes(item);
  const body = shopperShape && shapes.includes(shopperShape) ? shopperShape : (shapes[0] ?? BASE_MALE);
  const mismatch = !!shopperShape && !shapes.includes(shopperShape);
  const outfit = worn.data && !mismatch && (!wornShape || shapes.includes(wornShape)) ? worn.data : undefined;
  const failed = status === "error" || status === "empty";
  // The scene's WebGL start runs during the outfit's entity read (during the profile read the page still renders), and
  // the scene loads its parts after that read; the thumbnail stays over the canvas until the first drawn frame.
  const preview3d = show3d && canPreview && !failed && (isEmote || view === "avatar");
  const scene = preview3d && !profile.isLoading;
  // A scene unmounted while it was ready (a new wearer's profile loads, the item view is picked) starts over when it
  // mounts again: until it draws, the thumbnail is back over the box instead of a blank canvas.
  useEffect(() => {
    if (!scene && !failed) setStatus("loading");
  }, [scene, failed]);
  const shownStatus: AvatarStatus = !scene && !failed ? "loading" : status;
  const clip = useShopQuery(isEmote && urn && show3d && !waiting ? ["emote-clip", urn, body] : null, (signal) => fetchEmoteClipUrl(urn!, body, signal), { staleMs: 30 * 60_000 });

  return (
    <div className="shop-preview" data-status={show3d ? shownStatus : "thumbnail"}>
      {scene ? (
        <WearablePreview
          base={base}
          urns={isEmote ? [] : [urn!]}
          body={body}
          outfit={outfit}
          outfitPending={waiting}
          emote={isEmote ? (clip.data ?? "idle") : undefined}
          emoteNonce={replay}
          spin={!isEmote}
          zoom={1.05}
          onStatus={setStatus}
        />
      ) : null}
      {item.thumbnail && (!preview3d || shownStatus !== "ready") ? (
        <img className={preview3d ? "shop-preview__thumb shop-preview__thumb--cover" : "shop-preview__thumb"} src={item.thumbnail} alt={item.name} />
      ) : null}
      {preview3d && shownStatus === "loading" && !item.thumbnail ? (
        <div className="shop-preview__loading" role="status">
          {t("itemDetail.previewLoading")}
        </div>
      ) : null}
      {show3d && failed ? (
        <p className="shop-preview__note" role="status">
          {t("itemDetail.previewUnavailable")}
        </p>
      ) : null}
      {show3d && canPreview && !failed && !isEmote ? <ViewToggle view={view} onView={setView} /> : null}
      {show3d && canPreview && !failed && isEmote && status === "ready" ? <EmotePlaybar replay={replay} onReplay={() => setReplay((n) => n + 1)} /> : null}
      {!show3d && canPreview ? (
        <button type="button" className="shop-preview__show" onClick={() => setShow3d(true)} data-testid="show-3d">
          {t("itemDetail.showPreview")}
        </button>
      ) : null}
      {mismatch ? (
        <p className="shop-preview__note" data-testid="shape-note">
          {t("itemDetail.shownOnBody", { shape: shapeLabel(body) })}
        </p>
      ) : null}
    </div>
  );
}

// The web preview, as decentraland.org's Shop draws it (shop@3a322cf9 ItemPreview): the shopper's own avatar when it
// can wear the item, otherwise a default one of a shape the item fits; the item alone in the item view; an emote on the
// avatar. The Unity renderer where the `unity-wearable-preview` flag and this device allow it, Babylon otherwise and
// when Unity fails to load. Unity ships its own view and emote controls, so the page draws its own only over Babylon.
function WebItemPreview({ item, avatarShape = null, renderer }: ItemPreviewProps) {
  const [show3d, setShow3d] = useState(renderer === "full");
  // Read on the client only: the device decides between Unity and Babylon, and whether to mount at all without WebGL.
  const [fullDevice, setFullDevice] = useState<boolean | null>(renderer ? renderer === "full" : null);
  useEffect(() => {
    const decision = pickRenderer();
    if (!renderer) {
      setFullDevice(decision.renderer === "full");
      if (decision.reason !== "no-webgl") setShow3d(true);
    }
  }, [renderer]);
  const [view, setView] = useState<"avatar" | "item">("avatar");
  // What the preview reported, each for the frame and scene it reported it on: a new frame or scene starts loading.
  const [drawn, setDrawn] = useState<{ scene: string; by: PreviewRendererName } | null>(null);
  const [failedFrame, setFailedFrame] = useState<string | null>(null);
  const [unityFailed, setUnityFailed] = useState(false);
  const [generation, setGeneration] = useState(0);
  const [replay, setReplay] = useState(0);
  const urn = item.urn ?? itemUrn(item);
  const canPreview = !!urn && item.category !== "ens";
  const isEmote = item.category === "emote";
  const { account } = useShopWallet();
  const wanted = canPreview && show3d;
  const profile = useShopQuery(wanted && account ? ["wearer-profile", account] : null, (signal) => fetchProfile(account!, { signal }), { staleMs: 5 * 60_000 });
  const unityFlag = useRemoteFlagState(REMOTE_FLAG.unityWearablePreview, wanted);
  const ownBody = account ? profile.data?.avatar?.bodyShape : null;
  const ownShape = ownBody ? profileShape({ avatar: { bodyShape: ownBody } }) : null;
  const shopperShape = avatarShape ?? ownShape;
  const shapes = itemShapes(item);
  const body = shopperShape && shapes.includes(shopperShape) ? shopperShape : (shapes[0] ?? BASE_MALE);
  const mismatch = !isEmote && !!shopperShape && !shapes.includes(shopperShape);
  const wearsOwn = !!ownShape && (isEmote || shapes.includes(ownShape));
  const itemAlone = !isEmote && view === "item";
  // Mounted once with its final avatar and renderer, so the preview never loads a default and then the shopper's. A
  // lighter device draws with Babylon whatever the flag says, so it does not wait for it.
  const settled = !(account && profile.isLoading) && fullDevice !== null && (fullDevice === false || unityFlag !== null);
  const unity = !unityFailed && unityFlag === true && fullDevice === true;

  // A frame is mounted per renderer, preview origin, shopper and retry; a new item or view reloads its scene in place.
  const frame = [unity ? "unity" : "babylon", previewBase(urn ? [urn] : []), account ?? "", generation].join("|");
  const scene = `${frame}|${urn}|${itemAlone ? "item" : "avatar"}`;
  const failed = failedFrame === frame;
  const status: "loading" | "ready" | "error" = failed ? "error" : drawn?.scene === scene ? "ready" : "loading";
  const drawnBy = drawn?.scene === scene ? drawn.by : undefined;
  const mounted = wanted && settled && !failed;

  useEffect(() => {
    setView("avatar");
    setUnityFailed(false);
    setFailedFrame(null);
  }, [urn]);
  // What a retired or replaced frame drew is not the next one's, even when the next one comes back under the same key.
  useEffect(() => {
    setDrawn(null);
  }, [mounted, frame]);

  const options: PreviewFrameOptions = {
    urns: urn ? [urn] : [],
    profile: itemAlone ? undefined : wearsOwn ? account! : "default",
    bodyShape: itemAlone || wearsOwn ? undefined : body,
    type: isEmote ? undefined : itemAlone ? "wearable" : "avatar",
    emote: isEmote || itemAlone ? undefined : "fashion",
    disableBackground: true,
    wheelZoom: isEmote ? 1.5 : undefined,
    wheelStart: isEmote ? 100 : undefined,
    unity: unity || undefined,
    unityMode: unity ? "marketplace" : undefined,
  };
  const ownControls = status === "ready" && drawnBy === "babylon";

  return (
    <div className="shop-preview" data-status={show3d ? status : "thumbnail"} data-renderer={drawnBy}>
      {mounted ? (
        <PreviewFrame
          key={frame}
          options={options}
          replay={replay}
          loadTimeoutMs={unity ? UNITY_LOAD_TIMEOUT_MS : undefined}
          onLoad={(by) => setDrawn({ scene, by: by ?? (unity ? "unity" : "babylon") })}
          onError={() => {
            // The item alone failed where the avatar had drawn: back to the avatar, in a fresh frame. Before the
            // renderer: a preview that asked for Unity and drew with Babylon got this far on Babylon.
            if (itemAlone) {
              setView("avatar");
              setGeneration((n) => n + 1);
            } else if (unity) setUnityFailed(true);
            else setFailedFrame(frame);
          }}
        />
      ) : null}
      {item.thumbnail && (!mounted || status !== "ready") ? (
        <img className={mounted ? "shop-preview__thumb shop-preview__thumb--cover" : "shop-preview__thumb"} src={item.thumbnail} alt={item.name} />
      ) : null}
      {mounted && status === "loading" && !item.thumbnail ? (
        <div className="shop-preview__loading" role="status">
          {t("itemDetail.previewLoading")}
        </div>
      ) : null}
      {show3d && failed ? (
        <p className="shop-preview__note" role="status">
          {t("itemDetail.previewUnavailable")}
        </p>
      ) : null}
      {ownControls && !isEmote ? <ViewToggle view={view} onView={setView} /> : null}
      {ownControls && isEmote ? <EmotePlaybar replay={replay} onReplay={() => setReplay((n) => n + 1)} /> : null}
      {!show3d && canPreview ? (
        <button type="button" className="shop-preview__show" onClick={() => setShow3d(true)} data-testid="show-3d">
          {t("itemDetail.showPreview")}
        </button>
      ) : null}
      {mismatch && !itemAlone ? (
        <p className="shop-preview__note" data-testid="shape-note">
          {t("itemDetail.shownOnBody", { shape: shapeLabel(body) })}
        </p>
      ) : null}
    </div>
  );
}

export default function ItemPreview(props: ItemPreviewProps) {
  return insideExplorer() ? <ExplorerItemPreview {...props} /> : <WebItemPreview {...props} />;
}
