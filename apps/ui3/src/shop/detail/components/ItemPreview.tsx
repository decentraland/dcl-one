// Shop item preview with avatar, wearable, and emote states. On the web the avatar is the wearable-preview iframe that
// decentraland.org's Shop embeds; inside the Explorer it is the Explorer's own preview host, fed the worn outfit.

import { useEffect, useState } from "react";
import { PreviewSurface, WebItemPreview } from "@dcl/one-ui/apps/shop/preview";

import { fetchProfile } from "../../../data/catalyst/profile";
import type { AvatarStatus } from "../../../wearable-preview/avatar";
import WearablePreview from "../../../wearable-preview/WearablePreview";
import type { OutfitData } from "../../../wearable-preview/outfit";
import type { CatalogItem } from "@shop/discovery/lib/catalogItem";
import { useShopWallet } from "../../wallet/context";
import { avatarShape as profileShape, BASE_MALE, itemShapes, type BodyShapeUrn } from "@shop/detail/lib/bodyShape";
import { pickRenderer } from "../lib/pickRenderer";
import { decidePreviewDevice } from "../lib/previewDevice";
import { itemUrn } from "@shop/detail/lib/urn";
import { fetchWearableRules, keepEquipped, sharedEntityBase } from "../lib/wearable-rules";
import { fetchEmoteClipUrl } from "../lib/emoteBase64";
import { useShopQuery } from "../../discovery/query";
import { REMOTE_FLAG, useRemoteFlagState } from "../../discovery/lib/remoteFlags";

// The Explorer draws the preview itself and announces it before it is ready (bevy-host.ts).
const insideExplorer = () => typeof window !== "undefined" && (!!window.dclAvatarPreview || !!window.dclAvatarPreviewExpected);

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
    <PreviewSurface
      name={item.name}
      status={show3d ? shownStatus : "thumbnail"}
      thumbnail={!preview3d || shownStatus !== "ready" ? item.thumbnail : undefined}
      overlay={preview3d}
      loading={preview3d && shownStatus === "loading" && !item.thumbnail}
      unavailable={show3d && failed}
      view={show3d && canPreview && !failed && !isEmote ? { current: view, select: setView } : undefined}
      emote={show3d && canPreview && !failed && isEmote && status === "ready" ? { replay, play: () => setReplay((n) => n + 1) } : undefined}
      show={!show3d && canPreview ? () => setShow3d(true) : undefined}
      shapeNote={mismatch ? body : undefined}
    >
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
    </PreviewSurface>
  );
}

function WebPreviewHost({ item, avatarShape = null, renderer }: ItemPreviewProps) {
  const urn = item.urn ?? itemUrn(item);
  const canPreview = !!urn && item.category !== "ens";
  const [show3d, setShow3d] = useState(renderer === "full");
  // Read on the client only: the device decides between Unity and Babylon, and whether to mount at all without WebGL.
  // An item with no preview (a NAME) never asks, so it never pays for the link probe.
  const [fullDevice, setFullDevice] = useState<boolean | null>(renderer ? renderer === "full" : null);
  useEffect(() => {
    if (renderer || !canPreview) return;
    return decidePreviewDevice((decision) => {
      setFullDevice(decision.renderer === "full");
      if (decision.reason !== "no-webgl") setShow3d(true);
    });
  }, [renderer, canPreview]);
  const { account } = useShopWallet();
  const wanted = canPreview && show3d;
  const profile = useShopQuery(wanted && account ? ["wearer-profile", account] : null, (signal) => fetchProfile(account!, { signal }), { staleMs: 5 * 60_000 });
  const unityFlag = useRemoteFlagState(REMOTE_FLAG.unityWearablePreview, wanted);
  const bevyFlag = useRemoteFlagState(REMOTE_FLAG.bevyAvatarPreview, wanted);
  const ownBody = account ? profile.data?.avatar?.bodyShape : null;
  return (
    <WebItemPreview
      item={item}
      urn={urn}
      avatarShape={avatarShape}
      fullDevice={fullDevice}
      show3d={show3d}
      onShow={() => setShow3d(true)}
      account={account}
      ownBody={ownBody}
      profileLoading={profile.isLoading}
      unityFlag={unityFlag}
      bevyFlag={bevyFlag}
    />
  );
}
export default function ItemPreview(props: ItemPreviewProps) {
  return insideExplorer() ? <ExplorerItemPreview {...props} /> : <WebPreviewHost {...props} />;
}
