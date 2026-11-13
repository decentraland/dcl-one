// from: sites@17b3535a, features/places/places.helpers.ts, places.mappers.ts and places.types.ts (the deep-link card's
// data and the rules that read the jump URL).

export const DEFAULT_POSITION = "0,0";
export const DEFAULT_REALM = "main";

export type JumpCardData = {
  type: "place" | "event";
  title: string;
  userName: string;
  /** The creator's wallet, when there is one: it drives the profile link and the avatar colour. */
  user?: string;
  avatar?: string;
  avatarColor?: string;
  /** Credited to the Decentraland Foundation: the card shows its logo when the scene has no deployer face. */
  foundation?: boolean;
  coordinates: [number, number];
  image?: string;
  description?: string;
  userCount: number;
  realm?: string;
  /** Places: the explorer link the Jump In button opens. */
  playHref: string;
  /** Events only. */
  startAt?: string;
  finishAt?: string;
  live?: boolean;
  totalAttendees?: number;
};

export type ParsedPosition = { coordinates: [number, number]; isValid: boolean };

/** "x,y" or "x.y" (a separator, not a decimal); anything that is not two integers is invalid. */
export function parsePosition(value: string): ParsedPosition {
  const tokens = value.split(/[,.]/);
  if (tokens.length !== 2) return { coordinates: [0, 0], isValid: false };
  const x = Number.parseInt(tokens[0]!, 10);
  const y = Number.parseInt(tokens[1]!, 10);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return { coordinates: [0, 0], isValid: false };
  return { coordinates: [x, y], isValid: true };
}

export function isEns(value: string | undefined): value is `${string}.eth` {
  return !!value && /^[a-zA-Z0-9._-]+\.eth$/.test(value);
}

export type JumpTarget = {
  /** The position the page shows, 0,0 when the URL has none. */
  position: ParsedPosition;
  /** The position the places lookup asks for: none for a World jump without one, so it reads the World's record. */
  placesPosition?: [number, number];
  realm?: string;
  id?: string;
};

/** What a jump URL asks for: `?position=`, `?realm=` (or its alias `?world=`) and, for events, `?id=`. */
export function readJumpTarget(search: URLSearchParams): JumpTarget {
  const rawPosition = search.get("position");
  const position = parsePosition(rawPosition ?? DEFAULT_POSITION);
  const realmParam = search.get("realm") ?? search.get("world") ?? DEFAULT_REALM;
  const realm = realmParam === DEFAULT_REALM ? undefined : realmParam;
  const placesPosition = rawPosition !== null || !realm ? position.coordinates : undefined;
  return { position, placesPosition, realm, id: search.get("id") ?? undefined };
}

export function eventHasEnded(finishAt: string | undefined, now = Date.now()): boolean {
  if (!finishAt) return false;
  const t = new Date(finishAt).getTime();
  return !Number.isNaN(t) && now > t;
}

export function formatLocation([x, y]: [number, number]): string {
  return `${x}, ${y}`;
}

/** The event's start as decentraland.org prints it: "Nov 10, 2026, 04:00 PM UTC" in the viewer's zone. */
export function formatEventDate(iso: string, timeZone?: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZoneName: "short", timeZone });
}

export const DEFAULT_DESCRIPTION =
  "You're about to jump into Decentraland! Explore a virtual world built by its community and make new friends from anywhere.";
