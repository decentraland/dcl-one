type RGB = readonly [number, number, number]
function opaqueRGB(value: string): RGB {
  const match = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)$/.exec(
    value
  )
  if (!match || (match[4] !== undefined && Number(match[4]) !== 1))
    throw new Error('Contrast requires opaque computed RGB colors')
  const rgb = match.slice(1, 4).map(Number)
  if (rgb.some((channel) => !Number.isFinite(channel) || channel < 0 || channel > 255))
    throw new Error('Invalid computed RGB color')
  return rgb as unknown as RGB
}
function luminance(rgb: RGB): number {
  const [r, g, b] = rgb.map((channel) => {
    const value = channel / 255
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
}
export function colorContrast(foreground: string, background: string): number {
  const a = luminance(opaqueRGB(foreground)),
    b = luminance(opaqueRGB(background))
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
}
export function requiredTextContrast(fontSize: string, fontWeight: string): number {
  const size = Number.parseFloat(fontSize),
    weight = fontWeight === 'bold' ? 700 : fontWeight === 'normal' ? 400 : Number(fontWeight)
  if (!Number.isFinite(size) || size <= 0 || !Number.isFinite(weight) || weight <= 0)
    throw new Error('Invalid computed font metrics')
  // WCAG uses points; 14pt bold is 18.666... CSS pixels, not 14 CSS pixels.
  return size >= 24 || (weight >= 700 && size >= (14 * 96) / 72) ? 3 : 4.5
}
