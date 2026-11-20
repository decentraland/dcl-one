// @mentions: how they're written and matched. Shared by the HUD and the bridge scene (pure).

/** A claimed name has no #suffix and isn't a raw address — the fallback when no profile says. */
export function looksClaimed(name: string): boolean {
  return name.trim().length > 0 && !name.includes('#') && !/^0x[0-9a-f]+$/i.test(name)
}

/** What follows "@": the name's letters and digits, plus "#" and the wallet's last 4 when the
 *  name isn't claimed (Name#1a2b). */
export function mentionName(name: string, address: string, claimed?: boolean): string {
  const base = name.split('#')[0].replace(/[^A-Za-z0-9]/g, '')
  return (claimed ?? looksClaimed(name)) || address.length < 4 ? base : `${base}#${address.slice(-4)}`
}

/** A bold or italic tag of a chat message: lowercase b or i, opening or closing, with up to 32
 *  characters after whitespace before the `>`, none of them `<` (so a tag never takes in the text
 *  up to a later tag's end). Its groups are `close` and `style`; the pattern is case-sensitive. The
 *  chat's parser and the mention pattern below share it, and the engine's bubbles read the same
 *  (`style_tag_len` in `nametag/bubble.rs`). */
export const STYLE_TAG_PATTERN = String.raw`<(?<close>\/)?(?<style>[bi])(?:\s[^<>]{0,32})?>`
// the same, without its named groups: it stands twice in the mention pattern
const STYLE_TAG = String.raw`<\/?[bi](?:\s[^<>]{0,32})?>`
const STYLE_TAG_RE = new RegExp(STYLE_TAG, 'g')

/** A written @mention: on its own word, up to 15 letters/digits, an optional 4-char #tag, then a
 *  space, end or punctuation; a bold or italic tag on either side counts as a space. Its
 *  `mention` group is the name after "@". The chat's link parser embeds this same pattern, with no
 *  flag that changes its reading, so what renders as a mention is exactly what counts as one. */
export const MENTION_PATTERN = String.raw`(?<=^|\s|${STYLE_TAG})@(?<mention>[A-Za-z0-9]{1,15}(?:#[A-Za-z0-9]{4})?)(?=\s|!|\?|\.|,|$|${STYLE_TAG})`
const MENTION_RE = new RegExp(MENTION_PATTERN, 'g')

/** Does the message @-mention exactly this mention name (case-insensitive)? A tag is read as the
 *  space it counts as, so text inside a tag's brackets (`<b @Name x>`) is no mention: the chat
 *  hides it, and the engine's bubbles read the same (`mentions` in `nametag/bubble.rs`). */
export function mentionsName(message: string, mention: string): boolean {
  const want = mention.toLowerCase()
  for (const m of message.replace(STYLE_TAG_RE, ' ').matchAll(MENTION_RE)) if (m.groups?.mention.toLowerCase() === want) return true
  return false
}

/** A name as shown: an unclaimed name carries its wallet tag (Name#1a2b), added when the
 *  profile stored it bare. */
export function displayName(name: string, address: string, claimed: boolean | undefined): string {
  return claimed === false && !name.includes('#') && address.length >= 4 ? `${name}#${address.slice(-4)}` : name
}
