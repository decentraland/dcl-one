import {
  AUTH_CHAIN_HEADER_PREFIX,
  AUTH_METADATA_HEADER,
  AUTH_TIMESTAMP_HEADER,
  buildRequestPayload,
} from "./signer";
import type { AuthLink } from "./types";
import { ETH_ADDRESS_RE } from "../catalyst/format/address";

type AuthResult =
  | { ok: true; wallet: string }
  | { ok: false; status: number; error: string };

const DEFAULT_MAX_AGE_MS = 10 * 60_000;

const LOGIN_PURPOSE = "Decentraland Login";
const EPHEMERAL_ADDRESS_PREFIX = "Ephemeral address: ";
const EXPIRATION_PREFIX = "Expiration: ";

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const WEEKDAYS = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const RFC2822_ZONES: Record<string, number> = {
  ut: 0, gmt: 0, est: -300, edt: -240, cst: -360, cdt: -300, mst: -420, mdt: -360, pst: -480, pdt: -420,
};
const ISO_RE = /^(\d{4})-(\d{2})-(\d{2})([Tt ])(\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?([Zz]|[+-]\d{2}:\d{2})?$/;
const RFC2822_RE =
  /^(?:([A-Za-z]{3})\s*,\s*)?(\d{1,2})\s+([A-Za-z]{3})\s+(\d{2,4})\s+(\d{2}):(\d{2})(?::(\d{2}))?\s+([A-Za-z]{1,3}|[+-]\d{4})(?:\s*\([^()]*\))?$/;
const DATE_STRING_RE = /^([A-Za-z]{3})\s+(\d{1,2})\s+(\d{4})\s+(\d{1,2}):(\d{2}):(\d{2})\s+(?:GMT\s*)?([+-]\d{2}):?(\d{2})$/;

/**
 * Milliseconds for a calendar-valid date-time at `offsetMinutes` from UTC, or null. A
 * second of 60 is the leap second and reads as the next minute.
 */
function utcMillis(
  year: number, month: number, day: number, hour: number, minute: number, second: number, ms: number, offsetMinutes: number,
): number | null {
  if (month < 1 || month > 12 || hour > 23 || minute > 59 || second > 60) return null;
  if (day < 1 || day > utcDate(year, month, 0).getUTCDate()) return null;
  const date = utcDate(year, month - 1, day);
  date.setUTCHours(hour, minute, second, ms);
  return date.getTime() - offsetMinutes * 60_000;
}

/** Midnight UTC of a day; unlike `Date.UTC`, years 0-99 stay as given. */
function utcDate(year: number, monthIndex: number, day: number): Date {
  const date = new Date(0);
  date.setUTCFullYear(year, monthIndex, day);
  return date;
}

/** `±HH` and `MM` as minutes east of UTC, or null past 23:59. */
function offsetOf(signedHours: string, minutes: string): number | null {
  const hours = Number(signedHours.slice(1));
  if (hours > 23 || Number(minutes) > 59) return null;
  const value = hours * 60 + Number(minutes);
  return signedHours.startsWith("-") ? -value : value;
}

/** RFC 2822 zones: `±HHMM`, `UT`, `GMT`, the US zones, or a military letter (read as UTC). */
function rfc2822Offset(zone: string): number | null {
  if (/^[+-]/.test(zone)) return offsetOf(zone.slice(0, 3), zone.slice(3));
  const lower = zone.toLowerCase();
  if (lower in RFC2822_ZONES) return RFC2822_ZONES[lower];
  return lower.length === 1 && lower !== "j" ? 0 : null;
}

/**
 * An ephemeral payload's expiration in the shapes `dcl-auth`'s `parse_expiration` accepts:
 * ISO 8601 with or without an offset or fractional seconds (no offset reads as UTC),
 * RFC 2822 (`Date.toUTCString()`) and `Date.toString()`. Null for anything else.
 */
export function parseExpiration(raw: string): number | null {
  const s = raw.trim();
  let m = ISO_RE.exec(s);
  if (m && (m[9] !== undefined || m[4] === "T")) {
    const zone = m[9];
    const offset = !zone || zone === "Z" || zone === "z" ? 0 : offsetOf(zone.slice(0, 3), zone.slice(4));
    if (offset === null) return null;
    const ms = Number((m[8] ?? "").slice(0, 3).padEnd(3, "0"));
    return utcMillis(+m[1], +m[2], +m[3], +m[5], +m[6], +m[7], ms, offset);
  }
  m = RFC2822_RE.exec(s);
  if (m) {
    const month = MONTHS.indexOf(m[3].toLowerCase()) + 1;
    const offset = rfc2822Offset(m[8]);
    if (offset === null) return null;
    // Obsolete short years: two digits are 2000-2049 or 1950-1999, three count from 1900.
    const short = Number(m[4]);
    const year = m[4].length === 2 ? (short < 50 ? 2000 : 1900) + short : m[4].length === 3 ? 1900 + short : short;
    const millis = utcMillis(year, month, +m[2], +m[5], +m[6], +(m[7] ?? 0), 0, offset);
    if (millis === null) return null;
    // A weekday, when present, must be the date's.
    const weekday = utcDate(year, month - 1, +m[2]).getUTCDay();
    if (m[1] !== undefined && m[1].toLowerCase() !== WEEKDAYS[weekday]) return null;
    return millis;
  }
  // `Date.toString()`: the parenthesised zone name and a leading weekday are dropped.
  const withoutZoneName = s.endsWith(")") && s.includes(" (") ? s.slice(0, s.lastIndexOf(" (")).trimEnd() : s;
  const space = withoutZoneName.indexOf(" ");
  const withoutWeekday =
    space === 3 && /^[A-Za-z]{3}$/.test(withoutZoneName.slice(0, 3)) ? withoutZoneName.slice(4) : withoutZoneName;
  m = DATE_STRING_RE.exec(withoutWeekday);
  if (m) {
    const month = MONTHS.indexOf(m[1].toLowerCase()) + 1;
    const offset = offsetOf(m[7], m[8]);
    if (offset === null) return null;
    return utcMillis(+m[3], month, +m[2], +m[4], +m[5], +m[6], 0, offset);
  }
  return null;
}

/**
 * A login delegation, line by line as `dcl-auth`'s `verify_login_chain` reads it. `message`
 * is the payload without carriage returns: the text the delegation's signature covers.
 */
function parseEphemeral(
  payload: string,
): { message: string; address: string; expiration: number } | null {
  const message = payload.replace(/\r/g, "");
  const [purpose, addressLine, expirationLine] = message.split("\n");
  if (
    purpose !== LOGIN_PURPOSE ||
    !addressLine?.startsWith(EPHEMERAL_ADDRESS_PREFIX) ||
    !expirationLine?.startsWith(EXPIRATION_PREFIX)
  ) {
    return null;
  }
  const address = addressLine.slice(EPHEMERAL_ADDRESS_PREFIX.length);
  if (!ETH_ADDRESS_RE.test(address)) return null;
  const expiration = parseExpiration(expirationLine.slice(EXPIRATION_PREFIX.length));
  if (expiration === null) return null;
  return { message, address: address.toLowerCase(), expiration };
}

export async function verifyAuthChainRequest(
  request: Request,
  opts: { now?: number; maxAgeMs?: number } = {},
): Promise<AuthResult> {
  const now = opts.now ?? Date.now();
  const maxAge = opts.maxAgeMs ?? DEFAULT_MAX_AGE_MS;

  const timestampRaw = request.headers.get(AUTH_TIMESTAMP_HEADER);
  const metadataStr = request.headers.get(AUTH_METADATA_HEADER) ?? "";
  if (!timestampRaw) {
    return { ok: false, status: 401, error: "missing identity timestamp" };
  }
  const timestamp = Number(timestampRaw);
  if (!Number.isFinite(timestamp)) {
    return { ok: false, status: 401, error: "invalid identity timestamp" };
  }
  if (Math.abs(now - timestamp) > maxAge) {
    return { ok: false, status: 401, error: "stale identity timestamp" };
  }

  const chain: AuthLink[] = [];
  for (let i = 0; ; i++) {
    const header = request.headers.get(`${AUTH_CHAIN_HEADER_PREFIX}${i}`);
    if (header === null) break;
    let link: AuthLink;
    try {
      link = JSON.parse(header) as AuthLink;
    } catch {
      return { ok: false, status: 401, error: `malformed auth link ${i}` };
    }
    if (
      !link ||
      typeof link.type !== "string" ||
      typeof link.payload !== "string" ||
      typeof link.signature !== "string"
    ) {
      return { ok: false, status: 401, error: `malformed auth link ${i}` };
    }
    chain.push(link);
    if (i > 8) return { ok: false, status: 401, error: "auth chain too long" };
  }
  if (chain.length < 2) {
    return { ok: false, status: 401, error: "empty auth chain" };
  }

  const signerLink = chain[0];
  if (signerLink.type !== "SIGNER" || !ETH_ADDRESS_RE.test(signerLink.payload)) {
    return { ok: false, status: 401, error: "missing SIGNER link" };
  }
  const owner = signerLink.payload.toLowerCase();

  const { recoverMessageAddress } = await import("viem");
  let authority = owner;
  let finalPayload: string | null = null;

  for (let i = 1; i < chain.length; i++) {
    const link = chain[i];
    const linkType: string = link.type;
    const isEphemeral = linkType === "ECDSA_EPHEMERAL" || linkType === "ECDSA_PERSONAL_EPHEMERAL";
    const isSignedEntity = linkType === "ECDSA_SIGNED_ENTITY" || linkType === "ECDSA_PERSONAL_SIGNED_ENTITY";
    // As in `dcl-auth`: only delegations precede the signed entity, which ends the chain.
    if (isSignedEntity && i !== chain.length - 1) {
      return { ok: false, status: 401, error: `${linkType} at link ${i} must be the last link` };
    }
    const eph = isEphemeral ? parseEphemeral(link.payload) : null;
    if (isEphemeral) {
      if (!eph) {
        return { ok: false, status: 401, error: "malformed ephemeral payload" };
      }
      if (eph.expiration <= now) {
        return { ok: false, status: 401, error: "identity expired" };
      }
    }
    let recovered: string;
    try {
      recovered = (
        await recoverMessageAddress({
          message: eph ? eph.message : link.payload,
          signature: link.signature as `0x${string}`,
        })
      ).toLowerCase();
    } catch {
      return {
        ok: false,
        status: 401,
        error: `unrecoverable signature at link ${i}`,
      };
    }
    if (recovered !== authority) {
      return { ok: false, status: 401, error: `broken auth chain at link ${i}` };
    }

    if (eph) {
      authority = eph.address;
    } else if (isSignedEntity) {
      finalPayload = link.payload;
    } else {
      return {
        ok: false,
        status: 401,
        error: `unsupported auth link type ${linkType}`,
      };
    }
  }

  if (finalPayload === null) {
    return { ok: false, status: 401, error: "missing signed request link" };
  }

  const rawMethod = request.method.toUpperCase();
  const method = rawMethod === "HEAD" ? "GET" : rawMethod;
  const path = new URL(request.url).pathname;
  const expected = buildRequestPayload(method, path, timestampRaw, metadataStr);
  if (finalPayload.toLowerCase() !== expected) {
    return {
      ok: false,
      status: 401,
      error: "request signature does not match request",
    };
  }

  return { ok: true, wallet: owner };
}
