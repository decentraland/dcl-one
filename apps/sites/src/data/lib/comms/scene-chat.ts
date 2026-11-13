// The place page's read-only scene chat: the room credentials the node's gatekeeper mints for a
// scene and the chat packets its participants publish.
// from sites@17b3535a: discover/sceneAdapter.ts, discover/guestIdentity.ts,
// cast2/cast2.utils.ts (generateRandomName) and hooks/useChat.ts
import type { AuthIdentity } from "../auth/types";
import { catalystBase } from "../catalyst/client";

export type SceneRoomCredentials = { url: string; token: string };
/** A parcel ("1,95"), or a world with the parcel and scene the page shows. */
export type SceneRoomTarget = { location: string; parcel?: string; sceneId?: string };
export type SceneRoom = { mode: "scene" | "cast"; credentials: SceneRoomCredentials };

const GATEKEEPER = "/comms-gatekeeper";
const GUEST_LIFESPAN_MS = 24 * 60 * 60 * 1000;

async function resolveSceneIdForParcel(parcel: string, signal?: AbortSignal): Promise<string | null> {
  try {
    const response = await fetch(`${catalystBase()}/content/entities/active`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "*/*" },
      body: JSON.stringify({ pointers: [parcel] }),
      signal,
    });
    if (!response.ok) return null;
    const entities = (await response.json()) as Array<{ id: string }> | null;
    return entities?.[0]?.id ?? null;
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

/** The scene's room, signed as the wallet when there is one so messages would publish under it, else as a guest. */
export async function fetchSceneAdapter(target: SceneRoomTarget, identity: AuthIdentity | null, signal?: AbortSignal): Promise<SceneRoomCredentials | null> {
  try {
    const world = target.location.endsWith(".eth");
    const sceneId = world ? (target.sceneId ?? target.location.toLowerCase()) : await resolveSceneIdForParcel(target.location, signal);
    if (!sceneId) return null;
    const realmName = world ? target.location.toLowerCase() : "main";
    const parcel = world ? (target.parcel ?? "0,0") : target.location;
    const signer = identity ?? (await getGuestIdentity());
    const { signedFetch } = await import("../auth/signer");
    const response = await signedFetch(signer, `${catalystBase()}${GATEKEEPER}/get-scene-adapter`, {
      method: "POST",
      metadata: { sceneId, parcel, realmName, signer: "decentraland-kernel-scene" },
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
      signal,
    });
    if (!response.ok) return null;
    const body = (await response.json()) as { adapter?: string };
    return body.adapter ? parseAdapterUrl(body.adapter) : null;
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

/** The fallback when the scene has no room: a cast watcher token, which the node answers 501 until cast is ported. */
export async function fetchCastWatcherToken(args: { location: string; identity: string; parcel?: string }, signal?: AbortSignal): Promise<SceneRoomCredentials | null> {
  try {
    const body = args.parcel ? { location: args.location, identity: args.identity, parcel: args.parcel } : { location: args.location, identity: args.identity };
    const response = await fetch(`${catalystBase()}${GATEKEEPER}/cast/watcher-token`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal,
    });
    if (!response.ok) return null;
    const json = (await response.json()) as { url?: string; token?: string };
    return json.url && json.token ? { url: json.url, token: json.token } : null;
  } catch {
    return null;
  }
}

/** The room to watch: the scene's own, else a cast broadcast; null when there is neither. */
export async function fetchSceneRoom(target: SceneRoomTarget, identity: AuthIdentity | null, signal?: AbortSignal): Promise<SceneRoom | null> {
  const scene = await fetchSceneAdapter(target, identity, signal);
  if (scene) return { mode: "scene", credentials: scene };
  const cast = await fetchCastWatcherToken({ location: target.location, identity: generateRandomName(), parcel: target.parcel }, signal);
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
