// from: shop@94584a5c:app/src/lib/store.ts; marketplace@f3956826:webapp/src/modules/store/{actions,sagas,utils}.ts
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// A creator's "store": the cover, short description and social links on their storefront, an off-chain
// content entity keyed by the store URN (one per owner). The classic Marketplace account banner reads the
// same entity, so both stay in sync. Reads are the content server's active entity; saving deploys a new
// STORE entity signed by the signed-in account's identity (no gas, no transaction). Only the owner edits
// their own store: the entity's pointer is the owner's URN and the content server checks the signer.

import { catalystBase } from "../../../data/catalyst/client";
import type { AuthIdentity } from "../../../data/auth/identity";
import { buildEntity, deployEntity, hashV1, signEntityId } from "./entity";

export type CreatorStore = {
  cover: string;
  coverHash: string;
  description: string;
  links: { website: string; twitter: string; discord: string; facebook: string };
};

export const emptyStore = (): CreatorStore => ({ cover: "", coverHash: "", description: "", links: { website: "", twitter: "", discord: "", facebook: "" } });

export const storeUrn = (address: string) => `urn:decentraland:off-chain:marketplace-stores:${address.toLowerCase()}`;

type StoreEntity = {
  content?: { file: string; hash: string }[];
  metadata?: { description?: string; images?: { name: string; file: string }[]; links?: { name: string; url: string }[] };
};

function linkUrl(links: { name: string; url: string }[] | undefined, name: string): string {
  const url = links?.find((l) => l.name === name)?.url ?? "";
  return url.startsWith("https://") ? url : "";
}

/** Upstream `fetchStore` (marketplace FETCH_STORE): the active store entity; an empty store when there is none or on error. */
export async function fetchStore(address: string, opts: { base?: string; fetchImpl?: typeof fetch } = {}): Promise<CreatorStore> {
  const peer = catalystBase(opts.base);
  try {
    const res = await (opts.fetchImpl ?? fetch)(`${peer}/content/entities/active`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ pointers: [storeUrn(address)] }),
    });
    if (!res.ok) return emptyStore();
    const entity = ((await res.json()) as StoreEntity[])?.[0];
    if (!entity?.metadata) return emptyStore();
    const coverFile = entity.metadata.images?.find((img) => img.name === "cover")?.file;
    const coverHash = coverFile ? entity.content?.find((c) => c.file === coverFile)?.hash : undefined;
    return {
      cover: coverHash ? `${peer}/content/contents/${coverHash}` : "",
      coverHash: coverHash ?? "",
      description: (entity.metadata.description ?? "").trim(),
      links: {
        website: linkUrl(entity.metadata.links, "website"),
        twitter: linkUrl(entity.metadata.links, "twitter"),
        discord: linkUrl(entity.metadata.links, "discord"),
        facebook: linkUrl(entity.metadata.links, "facebook"),
      },
    };
  } catch {
    return emptyStore();
  }
}

export const LINK_TYPES = ["website", "twitter", "discord", "facebook"] as const;
export type LinkType = (typeof LINK_TYPES)[number];

export const LINK_PREFIX: Record<LinkType, string> = {
  website: "https://",
  twitter: "https://www.twitter.com/",
  discord: "https://discord.gg/",
  facebook: "https://www.facebook.com/",
};

/** The editable draft (marketplace UPDATE_LOCAL_STORE edits it; REVERT_LOCAL_STORE resets it to the saved store). */
export type StoreDraft = {
  cover: string;
  coverName: string;
  coverHash: string;
  description: string;
  links: Record<LinkType, string>;
};

function coverNameFromUrl(url: string): string {
  const base = decodeURIComponent(url.split("?")[0]!.split("/").pop() || "");
  return `cover/${base || "cover.jpeg"}`;
}

export function draftFromStore(store: CreatorStore): StoreDraft {
  return {
    cover: store.cover,
    coverName: store.cover ? coverNameFromUrl(store.cover) : "",
    coverHash: store.coverHash,
    description: store.description,
    links: { ...store.links },
  };
}

const templateHashCache = new Map<string, Promise<string>>();

/** Upstream `templateHash`: a bundled cover template's content hash, to re-select it after a reload. */
export function templateHash(url: string, fetchImpl: typeof fetch = fetch): Promise<string> {
  let cached = templateHashCache.get(url);
  if (!cached) {
    cached = (async () => hashV1(new Uint8Array(await (await fetchImpl(url)).arrayBuffer())))();
    templateHashCache.set(url, cached);
  }
  return cached;
}

/** Empty is valid (the link is omitted); otherwise it must carry its prefix. */
export function isValidLink(type: LinkType, value: string): boolean {
  return value === "" || value.startsWith(LINK_PREFIX[type]);
}

/** The STORE entity metadata (@dcl/schemas `Store`), shaped as the classic Marketplace deploys it. */
export function buildStoreMetadata(address: string, draft: StoreDraft) {
  const owner = address.toLowerCase();
  const links = LINK_TYPES.filter((name) => draft.links[name]).map((name) => ({ name, url: draft.links[name] }));
  const images = draft.cover && draft.coverName ? [{ name: "cover", file: draft.coverName }] : [];
  return { id: storeUrn(owner), owner, description: draft.description, images, links, version: 1 };
}

export class StoreSaveError extends Error {
  override name = "StoreSaveError";
  constructor(readonly original: unknown) {
    super("save-failed");
  }
}

/**
 * Upstream `saveStore` (marketplace UPDATE_STORE): deploy the store entity, cover bytes included, signed
 * with the account's identity. Refuses an identity for another account: a store is only its owner's.
 */
export async function saveStore(
  address: string,
  draft: StoreDraft,
  identity: AuthIdentity,
  opts: { base?: string; fetchImpl?: typeof fetch; now?: () => number } = {},
): Promise<void> {
  if (identity.signer.toLowerCase() !== address.toLowerCase()) throw new StoreSaveError(new Error("A store can only be edited by its owner"));
  const fetchImpl = opts.fetchImpl ?? fetch;
  try {
    const files = new Map<string, Uint8Array>();
    if (draft.cover && draft.coverName) files.set(draft.coverName, new Uint8Array(await (await fetchImpl(draft.cover)).arrayBuffer()));
    const built = await buildEntity({
      type: "store",
      pointers: [storeUrn(address)],
      files,
      metadata: buildStoreMetadata(address, draft),
      timestamp: (opts.now ?? Date.now)(),
    });
    await deployEntity({ contentUrl: `${catalystBase(opts.base)}/content`, built, authChain: await signEntityId(identity, built.entityId), fetchImpl });
  } catch (error) {
    throw new StoreSaveError(error);
  }
}
