import { openCast } from '@dcl/sdk/cast';
import type { Cast, CastError, Code, Key, State, StreamKey } from '@dcl/sdk/cast';

let handle: Cast<'owner'> | undefined;

/**
 * The panel's one handle on the scene's stream and Cast. The owner role reads the stream key; the
 * server still decides what this admin may do. A second handle would take this one's slide packets.
 */
export const cast = (): Cast<'owner'> => (handle ??= openCast('owner'));

const DAY = 86_400_000;
const two = (n: number) => String(Math.floor(n)).padStart(2, '0');

/** The one expiry text of a key: the nearest day from one day up, the clock below. */
export function expiresIn(endsAt: number, now: number): string {
  const left = Math.max(0, endsAt - now);
  const days = Math.round(left / DAY);
  if (left >= DAY) return `${days} ${days === 1 ? 'day' : 'days'}`;
  return `${two(left / 3_600_000)}:${two((left / 60_000) % 60)}:${two((left / 1_000) % 60)}`;
}

// A refusal carries the server's words; these codes have none.
const SAY: Partial<Record<Code, string>> = {
  preview: 'Try again in Genesis/World. LocalPreview not supported.',
  busy: 'Wait for the last request to end.',
  timeout: 'The server did not answer. Try again.',
  unavailable: 'The server is not available. Try again.',
  noKey: 'Get a stream key first.',
};
const NOTICE: Partial<Record<Key['tag'], string>> = {
  revoked: 'The stream key was revoked.',
  expired: 'The stream key expired.',
  replaced: 'Another admin made a new stream key.',
};
const KEY = ['key', 'generate', 'reset', 'revoke', 'link'];

/** The words of the last refusal of one of `of`, while none of them is in flight. */
export function refusal(s: State, of: readonly string[]): string {
  const error: CastError | undefined =
    s.tag === 'unavailable' ? s.error : s.tag === 'following' && !s.write ? s.error : undefined;
  return error && (s.tag === 'unavailable' || of.includes(error.of))
    ? error.message || SAY[error.code] || error.code
    : '';
}

/** What the Stream and Cast sections show of the key; `off` where the realm has no streaming. */
export type KeyView = { notice: string; error: string } & (
  | { tag: 'loading' | 'off' | 'none' }
  | { tag: 'key'; key: StreamKey; expires: string; soon: boolean }
);

export function keyView(s: State = cast().state): KeyView {
  const error = refusal(s, KEY);
  if (s.tag !== 'following')
    return { tag: s.tag === 'unavailable' ? 'off' : 'loading', notice: '', error };
  const notice = NOTICE[s.key.tag] ?? '';
  if ((s.write && KEY.includes(s.write.of)) || (s.key.tag === 'unknown' && !error))
    return { tag: 'loading', notice, error };
  if (!('key' in s.key)) return { tag: 'none', notice, error };
  const { key } = s.key;
  return {
    tag: 'key',
    key,
    expires: expiresIn(key.endsAt, s.now),
    soon: s.key.tag === 'expiring',
    notice,
    error,
  };
}

/** The Cast links are asked for only where a key exists: a revoked key does not come back by itself. */
export const needsLinks = (s: State = cast().state): boolean =>
  s.tag === 'following' && 'key' in s.key && !s.key.key.links && !s.write && s.error?.of !== 'link';

/** What each confirmation says and sends. */
export const ASK = {
  reset: {
    title: 'Are you sure you want to reset your Stream Key?',
    text: 'Active streams using this stream key will be disconnected.',
    label: 'Reset',
    cmd: { do: 'reset' },
  },
  revoke: {
    title: 'Revoke the stream key?',
    text: 'The stream stops, and the key and the Cast links stop working.',
    label: 'Revoke',
    cmd: { do: 'revoke' },
  },
  room: {
    title: 'Reset the Cast room?',
    text: 'The stream key and both links change. Active streams will be disconnected.',
    label: 'Reset',
    cmd: { do: 'reset' },
  },
} as const;
