type Corners = { topLeft?: number; topRight?: number; bottomRight?: number; bottomLeft?: number }

const ARC_STEPS = 6

function corner(cx: number, cy: number, radius: number, from: number): string[] {
  if (radius <= 0) return [`${cx}px ${cy}px`]
  const points: string[] = []
  for (let i = 0; i <= ARC_STEPS; i++) {
    const a = from + (i / ARC_STEPS) * (Math.PI / 2)
    points.push(`${(cx + radius * Math.cos(a)).toFixed(2)}px ${(cy + radius * Math.sin(a)).toFixed(2)}px`)
  }
  return points
}

/** Unlike a mask, clipping removes the hole from pointer hit-testing too. `round` rounds the hole's
 *  corners by that many pixels. */
export function holeClip({ x, y, width, height }: { x: number; y: number; width: number; height: number }, round: Corners = {}): React.CSSProperties {
  const r = (v: number | undefined): number => Math.max(0, Math.min(v ?? 0, width / 2, height / 2))
  const [tl, tr, br, bl] = [r(round.topLeft), r(round.topRight), r(round.bottomRight), r(round.bottomLeft)]
  const hole = [
    ...corner(x + tl, y + tl, tl, Math.PI),
    ...corner(x + width - tr, y + tr, tr, -Math.PI / 2),
    ...corner(x + width - br, y + height - br, br, 0),
    ...corner(x + bl, y + height - bl, bl, Math.PI / 2)
  ]
  return {
    clipPath: `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${hole.join(', ')}, ${hole[0]}, 0 0)`
  }
}
