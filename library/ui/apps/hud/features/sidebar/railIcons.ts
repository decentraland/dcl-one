// The navigation glyph the design and the shared icon set both lack, drawn here as a 32x32 SVG
// and handed to IconButton as `art` (a data URI): the portable-experiences bolt.

const svg = (body: string): string =>
  `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none">${body}</svg>`)}`

const INK = '#fcfcfc'

export const boltArt = svg(
  `<path d="M17.6 3.2 7.4 16.9h6.3v10.2l10.9-13.9h-7V3.2Z" stroke="${INK}" stroke-width="2" stroke-linejoin="round"/>`
)
