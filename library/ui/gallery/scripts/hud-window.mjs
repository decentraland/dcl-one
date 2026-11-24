// What a HUD canvas window must show, read from the sources that decide it rather than copied: the HUD's scale
// formula from library/explorer-core/lib/hudScale.ts, the window's shape from the iframe's own computed CSS.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export const HUD_SCALE_SOURCE = fileURLToPath(
  new URL('../../../explorer-core/lib/hudScale.ts', import.meta.url)
)
const CONSTANTS = ['REFERENCE_WIDTH', 'REFERENCE_HEIGHT', 'MIN', 'MAX']

/** The reference canvas and bounds hudScale.ts scales by. Throws, naming the constant, when one cannot be read. */
export function readHudScale(source = readFileSync(HUD_SCALE_SOURCE, 'utf8')) {
  const value = (name) => {
    const match = source.match(new RegExp(`^const ${name} = ([0-9.]+)$`, 'm'))
    if (!match) throw new Error(`cannot read ${name} from library/explorer-core/lib/hudScale.ts`)
    return Number(match[1])
  }
  const [width, height, min, max] = CONSTANTS.map(value)
  return { width, height, min, max }
}

/** The `--ui-scale` hudScale.ts installs for a window, as it writes it. */
export function expectedHudScale({ width, height, min, max }, innerWidth, innerHeight) {
  return Math.min(max, Math.max(min, Math.min(innerWidth / width, innerHeight / height))).toFixed(4)
}

/** The height an iframe of this width takes from its computed `aspect-ratio` and `min-block-size`. */
export function expectedWindowHeight(width, aspectRatio, minBlockSize) {
  const [w, h] = aspectRatio.split('/').map((part) => Number(part.trim()))
  if (!w || !h) throw new Error(`the HUD canvas has no aspect ratio: ${aspectRatio}`)
  return Math.max(Math.round((width * h) / w), parseFloat(minBlockSize) || 0)
}
