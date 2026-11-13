// Event descriptions as the reference shows them (sites: utils/linkifyText.tsx, utils/emojiShortcodes.ts): http(s)
// URLs and markdown links become links, and Slack-style emoji shortcodes become emoji.
import { Fragment, type ReactNode } from "react";

/** The shortcodes event descriptions use, as the reference's curated map has them. */
const EMOJI = new Map<string, string>([
  ["+1", "👍"], ["-1", "👎"], ["100", "💯"], ["alarm_clock", "⏰"], ["alien", "👽"], ["art", "🎨"], ["balloon", "🎈"],
  ["beer", "🍺"], ["bell", "🔔"], ["birthday", "🎂"], ["blue_heart", "💙"], ["bomb", "💣"], ["book", "📖"], ["boom", "💥"],
  ["bow", "🙇"], ["bulb", "💡"], ["cake", "🍰"], ["calendar", "📅"], ["camera", "📷"], ["champagne", "🍾"], ["check", "✅"],
  ["checkered_flag", "🏁"], ["cherry_blossom", "🌸"], ["clap", "👏"], ["clapper", "🎬"], ["clinking_glasses", "🥂"],
  ["clock", "🕐"], ["cloud", "☁️"], ["coffee", "☕"], ["computer", "💻"], ["confetti_ball", "🎊"], ["construction", "🚧"],
  ["controller", "🎮"], ["cool", "🆒"], ["crown", "👑"], ["crystal_ball", "🔮"], ["dancer", "💃"], ["dart", "🎯"],
  ["dizzy", "💫"], ["dog", "🐶"], ["drum", "🥁"], ["earth_americas", "🌎"], ["earth_asia", "🌏"], ["earth_africa", "🌍"],
  ["egg", "🥚"], ["eight_pointed_black_star", "✴️"], ["eyes", "👀"], ["fire", "🔥"], ["fireworks", "🎆"], ["flag", "🚩"],
  ["game_die", "🎲"], ["gear", "⚙️"], ["gem", "💎"], ["ghost", "👻"], ["gift", "🎁"], ["globe_with_meridians", "🌐"],
  ["green_heart", "💚"], ["guitar", "🎸"], ["hammer", "🔨"], ["hammer_and_wrench", "🛠️"], ["handshake", "🤝"],
  ["headphones", "🎧"], ["heart", "❤️"], ["heart_eyes", "😍"], ["heavy_check_mark", "✔️"], ["hourglass", "⌛"],
  ["hourglass_flowing_sand", "⏳"], ["ice_cream", "🍦"], ["ice_cube", "🧊"], ["joystick", "🕹️"], ["key", "🔑"],
  ["laughing", "😆"], ["leaves", "🍃"], ["light_bulb", "💡"], ["lightning", "⚡"], ["link", "🔗"], ["lock", "🔒"],
  ["loudspeaker", "📢"], ["mag", "🔍"], ["magic_wand", "🪄"], ["mailbox", "📬"], ["martini", "🍸"], ["mask", "🎭"],
  ["mega", "📣"], ["memo", "📝"], ["microphone", "🎤"], ["movie_camera", "🎥"], ["musical_note", "🎵"],
  ["musical_score", "🎼"], ["notes", "🎶"], ["ok_hand", "👌"], ["package", "📦"], ["paintbrush", "🖌️"], ["palette", "🎨"],
  ["party_popper", "🎉"], ["partying_face", "🥳"], ["paw_prints", "🐾"], ["peace", "☮️"], ["pencil", "✏️"],
  ["pencil2", "✏️"], ["penguin", "🐧"], ["phone", "📞"], ["pin", "📌"], ["pizza", "🍕"], ["point_right", "👉"],
  ["point_up", "☝️"], ["popcorn", "🍿"], ["pray", "🙏"], ["purple_heart", "💜"], ["question", "❓"], ["rainbow", "🌈"],
  ["red_heart", "❤️"], ["robot", "🤖"], ["rocket", "🚀"], ["rose", "🌹"], ["saxophone", "🎷"], ["scroll", "📜"],
  ["shield", "🛡️"], ["ship", "🚢"], ["shopping_bags", "🛍️"], ["shrug", "🤷"], ["smile", "😄"], ["smiley", "😃"],
  ["soccer", "⚽"], ["sos", "🆘"], ["sparkler", "🎇"], ["sparkles", "✨"], ["speaker", "🔈"], ["speech_balloon", "💬"],
  ["star", "⭐"], ["star2", "🌟"], ["stars", "🌟"], ["sun", "☀️"], ["sunglasses", "😎"], ["tada", "🎉"],
  ["telephone", "☎️"], ["thinking", "🤔"], ["thumbsup", "👍"], ["thumbsdown", "👎"], ["ticket", "🎫"], ["trophy", "🏆"],
  ["tv", "📺"], ["umbrella", "☂️"], ["video_camera", "📹"], ["video_game", "🎮"], ["warning", "⚠️"], ["wave", "👋"],
  ["white_check_mark", "✅"], ["wink", "😉"], ["world_map", "🗺️"], ["wrench", "🔧"], ["yellow_heart", "💛"],
  ["zap", "⚡"]
]);
const SHORTCODE = /:([a-z0-9_+-]{1,32}):/gi;
const URL_RE = /https?:\/\/[^\s<>"]+/gi;
const MARKDOWN_LINK = /\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/g;
const TRAILING_PUNCT = /[\s<>"'.,;:!?]+$/;

export const replaceEmojiShortcodes = (text: string): string =>
  text.replace(SHORTCODE, (match, name: string) => EMOJI.get(name.toLowerCase()) ?? match);

function isSafeUrl(raw: string): boolean {
  try {
    const { protocol } = new URL(raw);
    return protocol === "http:" || protocol === "https:";
  } catch {
    return false;
  }
}

/** Cuts a URL at a second protocol glued to it, then drops trailing punctuation and unbalanced closing parentheses. */
function trimUrl(raw: string): string {
  const inner = raw.slice(1).search(/https?:\/\//i);
  let url = (inner >= 0 ? raw.slice(0, inner + 1) : raw).replace(TRAILING_PUNCT, "");
  const count = (s: string, c: string) => s.split(c).length - 1;
  while (url.endsWith(")") && count(url, ")") > count(url, "(")) url = url.slice(0, -1).replace(TRAILING_PUNCT, "");
  return url;
}

type Link = { start: number; end: number; url: string; label: string };

function nextLink(text: string, from: number): Link | null {
  let md: Link | null = null;
  MARKDOWN_LINK.lastIndex = from;
  for (let m = MARKDOWN_LINK.exec(text); m; m = MARKDOWN_LINK.exec(text)) {
    if (isSafeUrl(m[2]!)) { md = { start: m.index, end: m.index + m[0].length, url: m[2]!, label: replaceEmojiShortcodes(m[1]!) }; break; }
  }
  let bare: Link | null = null;
  URL_RE.lastIndex = from;
  for (let m = URL_RE.exec(text); m; m = URL_RE.exec(text)) {
    const url = trimUrl(m[0]);
    if (url && isSafeUrl(url)) { bare = { start: m.index, end: m.index + url.length, url, label: url }; break; }
    URL_RE.lastIndex = m.index + Math.max(m[0].length, 1);
  }
  if (!md || !bare) return md ?? bare;
  return md.start <= bare.start ? md : bare;
}

export function linkifyText(text: string): ReactNode {
  const parts: ReactNode[] = [];
  let cursor = 0;
  for (let link = nextLink(text, 0); link; link = nextLink(text, cursor)) {
    if (link.start > cursor) parts.push(<Fragment key={`t-${cursor}`}>{replaceEmojiShortcodes(text.slice(cursor, link.start))}</Fragment>);
    parts.push(<a key={`l-${link.start}`} href={link.url} target="_blank" rel="noopener noreferrer">{link.label}</a>);
    cursor = link.end;
  }
  if (parts.length === 0) return replaceEmojiShortcodes(text);
  if (cursor < text.length) parts.push(<Fragment key={`t-${cursor}`}>{replaceEmojiShortcodes(text.slice(cursor))}</Fragment>);
  return parts;
}
