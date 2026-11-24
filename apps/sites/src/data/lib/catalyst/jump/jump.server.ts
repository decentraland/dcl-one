import { z } from "zod";

import { CatalystError, catalystBase, errorFromResponse, getJSON, postPublicJSON, worldsBase, type GetOptions } from "../client";
import { EventRecordSchema } from "../generated-schemas/events";
import { fetchEvent, fetchEvents } from "../places/events";
import type { JumpEventRecord } from "./jump";

export { loadJumpPlace } from "./place-lookup";

const ActiveEntities = z.array(
  z.object({ id: z.string(), metadata: z.object({ owner: z.string().optional() }).passthrough().optional() }).passthrough(),
);
type ActiveEntity = z.infer<typeof ActiveEntities>[number];

async function postActiveEntity(url: string, pointer: string, opts: GetOptions): Promise<ActiveEntity | null> {
  const res = await (opts.fetchImpl ?? fetch)(url, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json" },
    body: JSON.stringify({ pointers: [pointer] }),
    signal: opts.signal,
  });
  if (!res.ok) throw await errorFromResponse(res, url);
  return ActiveEntities.parse(await res.json())[0] ?? null;
}

/**
 * A World's active scene; null when the worlds server answers that the name has none, an outage throws.
 * `opts.base` names a catalyst node, not a worlds server, so it does not move this lookup.
 */
export function loadWorldEntity(realm: string, opts: GetOptions = {}): Promise<ActiveEntity | null> {
  return postActiveEntity(`${worldsBase()}/entities/active`, realm.toLowerCase(), opts);
}

export type JumpCreator = { address: string; name: string; avatar?: string; hasClaimedName?: boolean };

/** The fields a card reads from a profile. */
const ProfileRow = z
  .object({
    avatars: z.array(
      z
        .object({
          name: z.string().nullish(),
          realName: z.string().nullish(),
          hasClaimedName: z.boolean().nullish(),
          ethAddress: z.string().nullish(),
          avatar: z
            .object({ snapshots: z.object({ face256: z.string().nullish() }).passthrough().nullish() })
            .passthrough()
            .nullish(),
        })
        .passthrough(),
    ),
  })
  .passthrough();

/**
 * Null when the wallet has no profile name, so the card keeps the catalogue's name. Asked as decentraland.org
 * asks it: `POST /lambdas/profiles` with the one
 * id, spelled as the deployment or event names it. The profile's own address feeds the avatar colour.
 */
export async function loadCreator(address: string, opts: GetOptions = {}): Promise<JumpCreator | null> {
  const avatar = z.array(ProfileRow).parse(await postPublicJSON("/lambdas/profiles", { ids: [address] }, opts))[0]?.avatars[0];
  const name = avatar?.name || avatar?.realName;
  if (!name) return null;
  return {
    address: avatar.ethAddress || address,
    name,
    avatar: avatar.avatar?.snapshots?.face256 ?? undefined,
    hasClaimedName: avatar.hasClaimedName ?? undefined,
  };
}

const Deployments = z.object({ deployments: z.array(z.object({ deployedBy: z.string() }).passthrough()) });

/** Who deployed the scene at a Genesis City parcel; null when the parcel is empty. */
export async function loadParcelCreator(coordinates: [number, number], opts: GetOptions = {}): Promise<JumpCreator | null> {
  const entity = await postActiveEntity(`${catalystBase(opts.base)}/content/entities/active`, coordinates.join(","), opts);
  if (!entity) return null;
  // The slash is part of the path decentraland.org asks (`fetchPeerDeployment`).
  const raw = await getJSON("/content/deployments/", { ...opts, query: { entityId: entity.id } });
  const deployedBy = Deployments.parse(raw).deployments[0]?.deployedBy;
  return deployedBy ? loadCreator(deployedBy, opts) : null;
}

/** The fields a jump card reads, out of the full event record the events client already validated. */
const JumpEventRow = EventRecordSchema.pick({
  id: true,
  name: true,
  image: true,
  description: true,
  next_start_at: true,
  next_finish_at: true,
  coordinates: true,
  user: true,
  user_name: true,
  scene_name: true,
  total_attendees: true,
  live: true,
  world: true,
  server: true,
  deleted_by_user: true,
  deleted_by_admin: true,
});

/** The events client has already validated the whole record; this keeps only what a card reads. */
const toJumpEvent = (event: unknown): JumpEventRecord => JumpEventRow.parse(event);

/**
 * The event a link names: by id, else the first one at the link's World or Genesis City parcel. A Genesis
 * City lookup asks for no World events, which mostly sit at 0,0 too. The node sends one row: the card shows
 * one event. Null when there is none.
 */
export async function loadJumpEvent(
  target: { id: string | null; lookup: [number, number] | undefined; realm: string | undefined },
  opts: GetOptions = {},
): Promise<JumpEventRecord | null> {
  if (target.id) {
    try {
      return toJumpEvent(await fetchEvent(target.id, opts));
    } catch (error) {
      // 400: the events service rejects an id that is not a UUID; to a visitor it is the same unknown event.
      if (error instanceof CatalystError && (error.status === 400 || error.status === 404)) return null;
      throw error;
    }
  }
  // A World link without a position asks for the World's events wherever they are in it.
  const position = target.lookup?.join(",");
  const params = target.realm ? { position, world_names: target.realm, limit: 1 } : { position, world: false as const, limit: 1 };
  const { data } = await fetchEvents(params, opts);
  return data[0] ? toJumpEvent(data[0]) : null;
}
