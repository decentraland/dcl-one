// Deterministic example data: every picture is an inline SVG, so the canvas makes no network request.

/** A gradient picture with a caption, as a `data:` URL. */
export function artwork(caption: string, hue: number, width = 640, height = 360): string {
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="hsl(${hue} 70% 50%)"/><stop offset="1" stop-color="hsl(${(hue + 70) % 360} 65% 22%)"/>` +
    `</linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/>` +
    `<text x="50%" y="50%" fill="#fff" font-family="sans-serif" font-size="${Math.round(height / 7)}" ` +
    `text-anchor="middle" dominant-baseline="middle">${caption}</text></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

/** A square face picture. */
export const face = (initial: string, hue: number) => artwork(initial, hue, 128, 128)

export const covers = {
  plaza: artwork('Genesis Plaza', 280),
  festival: artwork('Music Festival', 330),
  gallery: artwork('Art Gallery', 190),
  wearable: artwork('Hoodie', 210, 360, 360),
  emote: artwork('Emote', 40, 360, 360),
  blog: artwork('Blog post', 250)
}

export const people = {
  ada: { name: 'Ada', picture: face('A', 300), color: '#ff7439' },
  grace: { name: 'Grace', picture: face('G', 160), color: '#5ab7ff' }
}
