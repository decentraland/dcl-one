// The place page's read-only scene chat: the room credentials the node's gatekeeper mints for a
// scene and the chat packets its participants publish.
import { z } from "zod";

import type { AuthIdentity } from "../auth/types";
import { catalystBase, getJSON } from "../catalyst/client";

export type SceneRoomCredentials = { url: string; token: string };
/** A Genesis City parcel ("1,95"). A world has no target: see `fetchSceneRoom`. */
export type SceneRoomTarget = { location: string };
export type SceneRoom = { mode: "scene" | "cast"; credentials: SceneRoomCredentials };

const GATEKEEPER = "/comms-gatekeeper";
const GUEST_LIFESPAN_MS = 24 * 60 * 60 * 1000;
const PARCEL = /^-?\d+,-?\d+$/;

const ActiveEntities = z.array(z.object({ id: z.string().min(1) }));
const AboutRealm = z.object({ configurations: z.object({ realmName: z.string().trim().min(1) }) });
const SceneAdapter = z.object({ adapter: z.string().min(1) });
const WatcherToken = z.object({ url: z.string().min(1), token: z.string().min(1) });

/** An answer with an error status has no room. Its body is not read, so it is dropped: the connection is free at once. */
async function unread(response: Response): Promise<null> {
  await response.body?.cancel().catch(() => undefined);
  return null;
}

async function resolveSceneIdForParcel(parcel: string, signal?: AbortSignal): Promise<string | null> {
  try {
    const response = await fetch(`${catalystBase()}/content/entities/active`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "*/*" },
      body: JSON.stringify({ pointers: [parcel] }),
      signal,
    });
    if (!response.ok) return unread(response);
    return ActiveEntities.parse(await response.json())[0]?.id ?? null;
  } catch {
    return null;
  }
}

/** The gatekeeper's adapter is `livekit:<url>?access_token=<token>`. */
export function parseAdapterUrl(adapter: string): SceneRoomCredentials | null {
  try {
    const parsed = new URL(adapter.startsWith("livekit:") ? adapter.slice("livekit:".length) : adapter);
    const token = parsed.searchParams.get("access_token");
    if (!token) return null;
    parsed.searchParams.delete("access_token");
    return { url: parsed.toString(), token };
  } catch {
    return null;
  }
}

let guest: Promise<AuthIdentity> | null = null;

/** A throwaway wallet with its ephemeral key, made once per page: the gatekeeper admits any signer. */
export function getGuestIdentity(): Promise<AuthIdentity> {
  guest ??= (async () => {
    const [{ createIdentityFromPrivateKey }, { generatePrivateKey }] = await Promise.all([import("../auth/identity"), import("viem/accounts")]);
    return createIdentityFromPrivateKey(generatePrivateKey(), { expirationMs: GUEST_LIFESPAN_MS });
  })();
  return guest;
}

/** Genesis City rooms carry the realm name the node publishes in `/about`, which the Explorer signs too. */
async function nodeRealmName(signal?: AbortSignal): Promise<string> {
  return AboutRealm.parse(await getJSON("/about", { signal })).configurations.realmName;
}

/**
 * The room of a Genesis City parcel's scene, always signed as the guest. The gatekeeper names the room participant
 * after the signer, and a room holds one participant per name: the visitor's wallet here would remove that wallet's
 * Explorer session from the room. The panel only reads, so it never needs the wallet's name.
 */
async function fetchSceneAdapter(parcel: string, signal?: AbortSignal): Promise<SceneRoomCredentials | null> {
  try {
    const sceneId = await resolveSceneIdForParcel(parcel, signal);
    if (!sceneId) return null;
    const realmName = await nodeRealmName(signal);
    const signer = await getGuestIdentity();
    const { signedFetch } = await import("../auth/signer");
    const response = await signedFetch(signer, `${catalystBase()}${GATEKEEPER}/get-scene-adapter`, {
      method: "POST",
      metadata: { sceneId, parcel, realmName, signer: "dcl:explorer" },
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
      signal,
    });
    if (!response.ok) return unread(response);
    return parseAdapterUrl(SceneAdapter.parse(await response.json()).adapter);
  } catch {
    return null;
  }
}

const ANON_ADJECTIVES = ["happy", "brave", "clever", "bright", "swift", "gentle", "mighty", "calm", "wise", "bold", "eager", "noble", "proud", "quiet", "agile", "fancy", "jolly", "keen", "lively", "merry", "sharp", "warm", "wild", "cosmic", "crystal", "golden", "silver", "royal", "mystic", "ancient"];
const ANON_NOUNS = ["rabbit", "falcon", "dolphin", "tiger", "phoenix", "dragon", "wolf", "eagle", "panda", "lion", "fox", "hawk", "owl", "bear", "deer", "raven", "snake", "shark", "whale", "panther", "jaguar", "cheetah", "lynx", "otter", "beaver", "badger", "koala", "sloth", "penguin", "octopus"];

/** The cast watcher's anonymous name, drawn as the reference draws it (two `Math.random` calls). */
export function generateRandomName(): string {
  const adjective = ANON_ADJECTIVES[Math.floor(Math.random() * ANON_ADJECTIVES.length)];
  const noun = ANON_NOUNS[Math.floor(Math.random() * ANON_NOUNS.length)];
  return `${adjective}-${noun}`;
}

/** The fallback when the scene has no room: a cast watcher token. The node answers 501, which is no room. */
async function fetchCastWatcherToken(args: { location: string; identity: string }, signal?: AbortSignal): Promise<SceneRoomCredentials | null> {
  try {
    const response = await fetch(`${catalystBase()}${GATEKEEPER}/cast/watcher-token`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ location: args.location, identity: args.identity }),
      signal,
    });
    if (!response.ok) return unread(response);
    return WatcherToken.parse(await response.json());
  } catch {
    return null;
  }
}

/**
 * The room to watch: the scene's own, else a cast broadcast; null when there is neither. Only a Genesis City parcel
 * has one. A world's rooms are never asked for: the node counts every participant of a world's scene rooms as a
 * player against the world's player limit, so a page view there would take a player's place.
 */
export async function fetchSceneRoom(target: SceneRoomTarget, signal?: AbortSignal): Promise<SceneRoom | null> {
  if (!PARCEL.test(target.location)) return null;
  const scene = await fetchSceneAdapter(target.location, signal);
  if (scene) return { mode: "scene", credentials: scene };
  const cast = await fetchCastWatcherToken({ location: target.location, identity: generateRandomName() }, signal);
  return cast ? { mode: "cast", credentials: cast } : null;
}

/** Messages that carry a control prefix are the explorer's own signalling, not chat. */
const CONTROL_PREFIXES = ["␆", "␑", "␐"];

function readVarint(bytes: Uint8Array, at: number): [value: number, next: number] {
  let value = 0;
  let shift = 0;
  for (let i = at; i < bytes.length && shift <= 49; i++) {
    const byte = bytes[i];
    value += (byte & 0x7f) * 2 ** shift;
    if (byte < 0x80) return [value, i + 1];
    shift += 7;
  }
  throw new Error("truncated varint");
}

/** The length-delimited fields of one protobuf message, by field number (the last occurrence of each). */
function lengthDelimitedFields(bytes: Uint8Array): Map<number, Uint8Array> {
  const fields = new Map<number, Uint8Array>();
  let at = 0;
  while (at < bytes.length) {
    const [tag, afterTag] = readVarint(bytes, at);
    const field = Math.floor(tag / 8);
    const wire = tag % 8;
    at = afterTag;
    if (wire === 0) [, at] = readVarint(bytes, at);
    else if (wire === 1) at += 8;
    else if (wire === 5) at += 4;
    else if (wire === 2) {
      const [length, start] = readVarint(bytes, at);
      fields.set(field, bytes.subarray(start, start + length));
      at = start + length;
    } else throw new Error(`unsupported wire type ${wire}`);
  }
  return fields;
}

/**
 * The text of a comms `Packet` whose message is a `Chat` (library/protocol/proto/decentraland/kernel/comms/rfc4/comms.proto:
 * `Chat chat = 5`, `string message = 1`); null for any other packet, a control message or bytes that are not one.
 */
export function decodeChatPacket(payload: Uint8Array): string | null {
  try {
    const chat = lengthDelimitedFields(payload).get(5);
    if (!chat) return null;
    const text = lengthDelimitedFields(chat).get(1);
    if (!text) return null;
    const message = new TextDecoder().decode(text);
    return CONTROL_PREFIXES.some((prefix) => message.startsWith(prefix)) ? null : message;
  } catch {
    return null;
  }
}
