// Draws what the bridge scene shows behind the passport's avatar: the part of the passport panel's
// background under its avatar column, for each of the panel's three backgrounds, with the glow behind the
// avatar and the shadow under its feet over it. Chrome draws them from the panel's own CSS, so the
// scene's column and the page's panel beside it show the same gradient.
//
//   node scripts/passport-backgrounds.mjs <RadialGlow.png> [chrome binary]
//
// RadialGlow.png is unity-explorer's Explorer/Assets/Textures/Common/RadialGlow.png (Apache-2.0); it is
// drawn into the pictures, not copied to this tree. Run it again when the gradients in
// library/ui/apps/hud/features/profile/ProfilePassport.module.css change.
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const css = readFileSync(resolve(here, '../../../../../library/ui/apps/hud/features/profile/ProfilePassport.module.css'), 'utf8')
const glow = process.argv[2]
const chrome = process.argv[3] ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
if (glow == null) throw new Error('usage: passport-backgrounds.mjs <RadialGlow.png> [chrome binary]')

// The panel and its avatar column, in the pixels of the 1920x1080 canvas.
const PANEL = { width: 1650, height: 928 }
const COLUMN = { width: 514, height: 928 }

function background(selector) {
  const rule = css.slice(css.indexOf(`${selector} {`))
  const found = /--passport-bg:\s*([^;]+);/.exec(rule.slice(0, rule.indexOf('}')))
  if (found == null) throw new Error(`no --passport-bg in ${selector}`)
  return found[1]
}

const BACKGROUNDS = {
  overview: background('.panel'),
  photos: background(".panel[data-tab='photos']"),
  creations: background(".panel[data-tab='creations']")
}

const work = mkdtempSync(join(tmpdir(), 'passport-backgrounds-'))
const mask = `mask: url('${pathToFileURL(resolve(glow)).href}') center / 100% 100% no-repeat`
for (const [name, value] of Object.entries(BACKGROUNDS)) {
  const page = join(work, `${name}.html`)
  writeFileSync(
    page,
    `<!doctype html><style>
      html, body { margin: 0; overflow: hidden; background: #000 }
      .panel { position: absolute; left: 0; top: 0; width: ${PANEL.width}px; height: ${PANEL.height}px; background: ${value} }
      .column { position: absolute; left: 0; top: 0; width: ${COLUMN.width}px; height: ${COLUMN.height}px }
      /* Shadow: 484x130, 374 below the column's centre, black at 0.69 */
      .shadow { position: absolute; left: 50%; top: 50%; width: 484px; height: 130px; margin: ${374 - 65}px 0 0 -242px; background: rgba(0, 0, 0, 0.69); ${mask} }
      /* Glow: 640x1090, 130 above the column's centre, (0.790, 0.032, 0.981) at 0.149 */
      .glow { position: absolute; left: 50%; top: 50%; width: 640px; height: 1090px; margin: ${-130 - 545}px 0 0 -320px; background: rgba(201, 8, 250, 0.149); ${mask} }
    </style><div class="panel"></div><div class="column"><div class="shadow"></div><div class="glow"></div></div>`
  )
  const out = resolve(here, `../bridge-scene/images/passport-${name}.png`)
  execFileSync(chrome, [
    '--headless=new',
    '--hide-scrollbars',
    '--force-device-scale-factor=1',
    '--allow-file-access-from-files',
    `--window-size=${COLUMN.width},${COLUMN.height}`,
    `--screenshot=${out}`,
    pathToFileURL(page).href
  ], { stdio: 'ignore' })
  console.log(out)
}
