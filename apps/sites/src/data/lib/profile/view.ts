import type { OwnedItem, OwnedList, OwnedName } from "../catalyst/marketplace/account";
import type { GalleryImage } from "../catalyst/social/photos";

export type PhotoVM = {
  id: string;
  grad: string;
  metadata: {
    userName: string;
    userAddress: string;
    dateTime: string;
    realm: string;
    scene: { name: string; location: { x: string; y: string } };
    visiblePeople: {
      userName: string;
      userAddress: string;
      isGuest: boolean;
      wearables: string[];
    }[];
  };
};

function photoGrad(img: GalleryImage, i: number): string {
  const src = img.thumbnailUrl || img.url;
  if (src) return `#000 url("${src.replace(/"/g, "%22")}") center / cover no-repeat`;
  const h = (i * 47 + 196) % 360;
  return `linear-gradient(150deg, hsl(${h} 62% 48%) 0%, hsl(${(h + 38) % 360} 55% 26%) 100%)`;
}

export function mapPhotos(images: GalleryImage[]): PhotoVM[] {
  return images.map((img, i) => {
    const meta = ((img as { metadata?: Record<string, unknown> }).metadata ??
      {}) as Record<string, unknown>;
    const scene = (meta.scene ?? {}) as {
      name?: unknown;
      location?: { x?: unknown; y?: unknown };
    };
    const loc = (scene.location ?? {}) as { x?: unknown; y?: unknown };
    const people = Array.isArray(meta.visiblePeople)
      ? (meta.visiblePeople as Record<string, unknown>[])
      : [];
    return {
      id: img.id,
      grad: photoGrad(img, i),
      metadata: {
        userName: String(meta.userName ?? ""),
        userAddress: String(meta.userAddress ?? ""),
        dateTime: String(meta.dateTime ?? img.dateTime ?? ""),
        realm: String(meta.realm ?? ""),
        scene: {
          name: String(scene.name ?? "Decentraland"),
          location: { x: String(loc.x ?? "0"), y: String(loc.y ?? "0") },
        },
        visiblePeople: people.map((p) => ({
          userName: String(p.userName ?? ""),
          userAddress: String(p.userAddress ?? ""),
          isGuest: Boolean(p.isGuest ?? false),
          wearables: Array.isArray(p.wearables)
            ? (p.wearables as unknown[]).map(String)
            : [],
        })),
      },
    };
  });
}

export type AssetWearableVM = {
  id: string;
  name: string;
  rarity: string;
  price: string;
  network: "MATIC" | "ETHEREUM";
  category: string;
  bodyShape: "unisex";
  isSmart: false;
};
export type AssetNameVM = { id: string; stem: string };
export type ProfileAssets = { wearables: AssetWearableVM[]; names: AssetNameVM[] };

export const EMPTY_ASSETS: ProfileAssets = { wearables: [], names: [] };

export function mapAssets(
  wearables: OwnedList<OwnedItem>,
  names: OwnedList<OwnedName>,
): ProfileAssets {
  return {
    wearables: wearables.elements.map((w) => ({
      id: w.id,
      name: w.name ?? "Unnamed",
      rarity: w.rarity ?? "common",
      price: w.price ?? "",
      network: w.urn?.includes(":matic:")
        ? "MATIC"
        : w.urn?.includes(":ethereum:")
          ? "ETHEREUM"
          : "MATIC",
      category: w.category ?? "wearable",
      bodyShape: "unisex",
      isSmart: false,
    })),
    names: names.elements.map((n) => ({ id: n.tokenId ?? n.name, stem: n.name })),
  };
}
