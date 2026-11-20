// Shop item preview with avatar, wearable, and emote states.

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
import { fetchWearableRules, keepEquipped } from "../lib/wearable-rules";
import { fetchEmoteClipUrl } from "../lib/emoteBase64";
import { useShopQuery } from "../../discovery/query";
import "./itempreview.css";

const VIEW_TEXT = { previewMode: "Preview mode", onAvatar: "On avatar", itemView: "Item", playEmote: "Play emote" };

// The three.js scene starts loading with the page, beside the outfit reads, not after them; only where this page
// draws it (WebGL, and no Explorer preview host).
if (typeof window !== "undefined" && pickRenderer().reason !== "no-webgl" && !window.dclAvatarPreviewExpected && !window.dclAvatarPreview) {
  void import("../../../wearable-preview/avatar").catch(() => {});
}

const wornItemUrn = (urn: string) => {
  const p = urn.split(":");
  return p.length === 7 && /^collections-v[12]$/.test(p[3] ?? "") ? p.slice(0, 6).join(":") : urn;
};

type WearerProfile = Awaited<ReturnType<typeof fetchProfile>>;

async function fetchWornOutfit(profile: WearerProfile, tryOn: { urn: string; category: string } | null): Promise<OutfitData | null> {
  const avatar = profile?.avatar;
  if (!avatar?.bodyShape) return null;
  const worn = [...new Set((avatar.wearables ?? []).map(wornItemUrn))];
  // The body and the item join this read; the preview mounts them next from the same entity cache.
  const rules = await fetchWearableRules(worn, [avatar.bodyShape, ...(tryOn ? [tryOn.urn] : [])]);
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

export default function ItemPreview({ item, avatarShape = null, renderer }: { item: CatalogItem; avatarShape?: BodyShapeUrn | null; renderer?: "full" | "lite" }) {
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
  const worn = useShopQuery(profile.data !== undefined ? ["worn-outfit", wearer, urn, tryOn?.category ?? null] : null, () => fetchWornOutfit(profile.data!, tryOn), { staleMs: 5 * 60_000 });
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
      {show3d && canPreview && !failed && !isEmote ? (
        <div className="shop-preview__toggle" role="group" aria-label={VIEW_TEXT.previewMode} data-testid="preview-toggle">
          <button type="button" data-active={view === "avatar" || undefined} aria-pressed={view === "avatar"} aria-label={VIEW_TEXT.onAvatar} onClick={() => setView("avatar")}>
            <span className="shop-preview__toggle-label">{VIEW_TEXT.onAvatar}</span>
            <span className="shop-preview__toggle-ico" aria-hidden>
              <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor"><circle cx="10" cy="5" r="3" /><path d="M4 18c0-4 2.7-7 6-7s6 3 6 7z" /></svg>
            </span>
          </button>
          <button type="button" data-active={view === "item" || undefined} aria-pressed={view === "item"} aria-label={VIEW_TEXT.itemView} onClick={() => setView("item")}>
            <span className="shop-preview__toggle-label">{VIEW_TEXT.itemView}</span>
            <span className="shop-preview__toggle-ico" aria-hidden>
              <svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor"><path d="M7 3 3 5.5 4.5 9 6 8.3V17h8V8.3l1.5.7L17 5.5 13 3c-.5 1.3-1.6 2-3 2s-2.5-.7-3-2z" /></svg>
            </span>
          </button>
        </div>
      ) : null}
      {show3d && canPreview && !failed && isEmote && status === "ready" ? (
        <div className="shop-preview__playbar" data-testid="emote-playbar">
          <button type="button" className="shop-preview__play" onClick={() => setReplay((n) => n + 1)} aria-label={VIEW_TEXT.playEmote}>
            <svg viewBox="0 0 20 20" width="16" height="16" fill="currentColor" aria-hidden><path d="M6 4v12l10-6z" /></svg>
          </button>
          <span className="shop-preview__track" aria-hidden>
            <span key={replay} className="shop-preview__knob" />
          </span>
        </div>
      ) : null}
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
