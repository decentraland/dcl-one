// A capture file is named after its Figma page, and a page name can hold characters that no Windows file
// name may contain, so a Windows checkout of the repository would fail. Every capture file name is formed and
// read back through this one pair. It is injective and reversible: `%` is always encoded, so an encoded
// sequence can never be confused with a literal one.
const WINDOWS_RESERVED = new Set('<>:"/\\|?*')
// A reserved device stem: the part before the first dot, in any case. COM and LPT also take the superscript
// digits ¹ ² ³ (Microsoft, "Naming Files, Paths, and Namespaces").
const DEVICE_STEM = /^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(\.|$)/i
const TRAILING = /[ .]$/

const isControl = (character: string) => {
  const code = character.charCodeAt(0)
  return code < 0x20 || code === 0x7f
}
const rejected = (character: string) => WINDOWS_RESERVED.has(character) || isControl(character)
const encode = (character: string) =>
  `%${character.charCodeAt(0).toString(16).toUpperCase().padStart(2, '0')}`

// True when Windows accepts `segment` as one file or folder name: no reserved or control character, no
// trailing dot or space, and no reserved device stem.
function isWindowsSafeName(segment: string): boolean {
  return (
    segment.length > 0 &&
    ![...segment].some(rejected) &&
    !TRAILING.test(segment) &&
    !DEVICE_STEM.test(segment)
  )
}

function encodePageFileName(pageName: string): string {
  let base = [...pageName]
    .map((character) => (rejected(character) || character === '%' ? encode(character) : character))
    .join('')
    .replace(TRAILING, encode)
  // A reserved device stem is encoded in its first character, which keeps the mapping reversible.
  if (DEVICE_STEM.test(base)) base = encode(base[0]!) + base.slice(1)
  return `${base}.json`
}

function decodePageFileName(fileName: string): string {
  return decodeURIComponent(fileName.replace(/\.json$/, ''))
}

export { decodePageFileName, encodePageFileName, isWindowsSafeName }
