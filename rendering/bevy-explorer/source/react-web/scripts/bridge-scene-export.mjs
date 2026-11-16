// The bridge scene's `export-static` and `export-web` npm scripts, in node so they run under any host
// shell: npm runs package scripts with cmd.exe on Windows, which has no `rm -rf`, `mkdir -p`,
// `cp -R` or `${SCENE_BASE_URL:-../}`. Lives outside bridge-scene so the scene export never packs it.
//
//   node ../scripts/bridge-scene-export.mjs        static/ for SCENE_BASE_URL (unset or empty: ../)
//   node ../scripts/bridge-scene-export.mjs --web  then copies it to deploy/web/bridge-scene/static
import { spawnSync } from 'node:child_process'
import { cpSync, readFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const scene = fileURLToPath(new URL('../bridge-scene/', import.meta.url))
const output = join(scene, 'static')

// the locally installed sdk-commands bin, with this node: npx is npx.cmd on Windows
const sdk = join(scene, 'node_modules/@dcl/sdk-commands')
const { bin } = JSON.parse(readFileSync(join(sdk, 'package.json'), 'utf8'))
const script = join(sdk, typeof bin === 'string' ? bin : bin['sdk-commands'])

rmSync(output, { recursive: true, force: true })
const exported = spawnSync(
  process.execPath,
  [
    script,
    'export-static',
    '--destination',
    'static/',
    '--realmName',
    'BevyExplorerUI',
    '--baseUrl',
    process.env.SCENE_BASE_URL || '../',
    '--commsAdapter',
    'ws-room:ws-room-service.decentraland.org/rooms/bevy-explorer-ui'
  ],
  { cwd: scene, stdio: 'inherit' }
)
if (exported.error) throw exported.error
if (exported.status !== 0) process.exit(exported.status ?? 1)

if (process.argv.includes('--web')) {
  const web = join(scene, '../../deploy/web/bridge-scene')
  rmSync(web, { recursive: true, force: true })
  cpSync(output, join(web, 'static'), { recursive: true })
}
