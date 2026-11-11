// Rich chat text: a raw message as URLs, worlds, locations and @mentions, as tokens <MessageText>
// renders (library/ui/apps/hud/features/chat/chatText.tsx). Pure, so it is unit-testable.
import { MENTION_PATTERN, mentionName, mentionsName } from '../../engine/mention'
import type { NearbyMember } from '../../engine/protocol'

export type Token =
  | { type: 'text'; value: string }
  | { type: 'url'; value: string }
  | { type: 'world'; value: string }
  | { type: 'location'; value: string; x: number; y: number }
  | { type: 'mention'; value: string; name: string; tag?: string }

// URL → world name → location (x,y) → @mention, scanned in one pass to keep original order.
// Worlds are ENS names (e.g. boedo.dcl.eth) → clickable "jump to realm". Coords require both
// signs/commas so we don't linkify every number; mentions use the shared MENTION_PATTERN.
const TOKEN_RE = new RegExp(
  String.raw`(?<url>https?:\/\/[^\s<>"']+)|(?<world>[a-z0-9][\w-]*\.(?:dcl\.)?eth\b)|(?<loc>-?\d{1,3}\s*,\s*-?\d{1,3})|` + MENTION_PATTERN,
  'gi'
)

export function parseMessage(text: string): Token[] {
  const tokens: Token[] = []
  let last = 0
  for (const m of text.matchAll(TOKEN_RE)) {
    const i = m.index ?? 0
    if (i > last) tokens.push({ type: 'text', value: text.slice(last, i) })
    const g = m.groups ?? {}
    if (g.url) {
      tokens.push({ type: 'url', value: g.url })
    } else if (g.world) {
      tokens.push({ type: 'world', value: g.world })
    } else if (g.loc) {
      const [x, y] = g.loc.split(',').map((s) => parseInt(s.trim(), 10))
      tokens.push({ type: 'location', value: g.loc, x, y })
    } else if (g.mention) {
      const [name, tag] = g.mention.split('#')
      tokens.push({ type: 'mention', value: `@${g.mention}`, name, tag })
    }
    last = i + m[0].length
  }
  if (last < text.length) tokens.push({ type: 'text', value: text.slice(last) })
  return tokens
}

/** Lowercased mention name (Name, or Name#1a2b when unclaimed) → address, from the nearby roster. */
export function buildNameIndex(members: NearbyMember[]): Map<string, string> {
  const idx = new Map<string, string>()
  for (const m of members) {
    if (!m.name.trim()) continue
    const mention = mentionName(m.name, m.address, m.claimed).toLowerCase()
    idx.set(mention, m.address)
    // A bare @Name still resolves to an unclaimed Name#1a2b when it's the only one nearby.
    const base = mention.split('#')[0]
    if (!idx.has(base)) idx.set(base, m.address)
  }
  return idx
}

export function resolveMention(t: Extract<Token, { type: 'mention' }>, index: Map<string, string>): string | undefined {
  return t.tag ? index.get(`${t.name}#${t.tag}`.toLowerCase()) : index.get(t.name.toLowerCase())
}

/** Does this message @-mention me? Exactly my mention name, so a different Name#ffff doesn't count. */
export function mentionsMe(text: string, me: { address?: string; name?: string; hasClaimedName?: boolean } | null): boolean {
  if (!me?.name || !me.address) return false
  return mentionsName(text, mentionName(me.name, me.address, me.hasClaimedName))
}
