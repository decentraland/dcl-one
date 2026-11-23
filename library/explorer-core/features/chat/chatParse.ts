// Rich chat text: a raw message as URLs, worlds, locations, @mentions and bold/italic tags, as
// tokens <MessageText> renders (library/ui/apps/hud/features/chat/chatText.tsx). Pure, so it is unit-testable.
import { MENTION_PATTERN, STYLE_TAG_PATTERN, mentionName, mentionsName } from '../../engine/mention'
import type { NearbyMember } from '../../engine/protocol'

export type Token =
  | { type: 'text'; value: string }
  | { type: 'url'; value: string }
  | { type: 'world'; value: string }
  | { type: 'location'; value: string; x: number; y: number }
  | { type: 'mention'; value: string; name: string; tag?: string }
  /** `<b>`, `</b>`, `<i>` or `</i>`: styles the tokens after it and is not shown itself. Only a
   *  tag that styles text is one; see `styling`. */
  | { type: 'tag'; value: string; style: 'b' | 'i'; close: boolean }

// Tag → URL → world name → location (x,y) → @mention, scanned in one pass to keep original order.
// Bold and italic are the only tags; any other tag is text. Worlds are ENS names (e.g.
// boedo.dcl.eth) → clickable "jump to realm". Coords require both signs/commas so we don't linkify
// every number; mentions use the shared MENTION_PATTERN. The scan is case-sensitive, for the tags
// and the mentions' own boundaries: the URL scheme and the world suffix spell out either case.
const TOKEN_RE = new RegExp(
  String.raw`(?<tag>${STYLE_TAG_PATTERN})|(?<url>[hH][tT][tT][pP][sS]?:\/\/[^\s<>"']+)|(?<world>[A-Za-z0-9][\w-]*\.(?:[dD][cC][lL]\.)?[eE][tT][hH]\b)|(?<loc>-?\d{1,3}\s*,\s*-?\d{1,3})|` + MENTION_PATTERN,
  'g'
)

/**
 * The tokens with every tag that styles no text turned back into text, so a hidden tag always
 * shows as a change of style: an opening tag is one when its style is off and text follows before
 * that style's next closing tag, and a closing tag when it ends such a one. `<b></b>`, a closing
 * tag with nothing open and a second `<b>` inside bold are shown as written; otherwise they would
 * part a link from the text beside it (`boe<b></b>do.dcl.eth`) with nothing to show for it. The
 * engine's bubbles read the same (`styling` in `nametag/bubble.rs`).
 */
function styling(tokens: Token[]): Token[] {
  // from the end: whether text stands between each opening tag and its style's next closing tag
  const styles = new Array<boolean>(tokens.length).fill(false)
  const ahead = { b: false, i: false }
  for (let at = tokens.length - 1; at >= 0; at--) {
    const t = tokens[at]
    if (t.type !== 'tag') ahead.b = ahead.i = true
    else if (t.close) ahead[t.style] = false
    else styles[at] = ahead[t.style]
  }
  const on = { b: false, i: false }
  const read: Token[] = []
  tokens.forEach((t, at) => {
    if (t.type === 'tag' && (t.close ? on[t.style] : !on[t.style] && styles[at])) {
      on[t.style] = !t.close
      read.push(t)
      return
    }
    const before = read[read.length - 1]
    const shown = t.type === 'tag' || t.type === 'text'
    if (shown && before?.type === 'text') before.value += t.value
    else read.push(t.type === 'tag' ? { type: 'text', value: t.value } : t)
  })
  return read
}

export function parseMessage(text: string): Token[] {
  const tokens: Token[] = []
  let last = 0
  for (const m of text.matchAll(TOKEN_RE)) {
    const i = m.index ?? 0
    const g = m.groups ?? {}
    if (i > last) tokens.push({ type: 'text', value: text.slice(last, i) })
    if (g.tag) {
      tokens.push({ type: 'tag', value: g.tag, style: g.style as 'b' | 'i', close: g.close != null })
    } else if (g.url) {
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
  return styling(tokens)
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
