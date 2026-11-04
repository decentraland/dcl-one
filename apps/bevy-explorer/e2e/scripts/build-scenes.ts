// Builds the visual scenes with dcl-scene-cli (apps/dcl-scene-cli): the vendored SDK toolchain
// into each scene once, then the bundle. `DCL_SCENE_CLI` names the binary, else it is on the PATH;
// a binary named by path has its folder put on the PATH of what it runs, since a scene's own
// `package.json` scripts call `dcl-scene-cli` by name.
//
//   node scripts/build-scenes.ts [scene id ...]
import { spawn } from 'node:child_process'
import { access, readFile } from 'node:fs/promises'
import path from 'node:path'
import { SCENES_DIR } from '../driver/paths.ts'

export async function workspaceFolders(scenesDir: string = SCENES_DIR): Promise<string[]> {
  const workspace = JSON.parse(await readFile(path.join(scenesDir, 'dcl-workspace.json'), 'utf8')) as { folders: { path: string }[] }
  return workspace.folders.map((folder) => folder.path)
}

/** The environment the CLI runs with: its own folder first on the PATH when it was named by path. */
export function environment(bin: string, env: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const dir = path.dirname(bin)
  if (dir === '.') return env
  const pathKey = Object.keys(env).find((key) => key.toUpperCase() === 'PATH') ?? 'PATH'
  return { ...env, [pathKey]: [path.resolve(dir), env[pathKey]].filter(Boolean).join(path.delimiter) }
}

function run(bin: string, args: string[], env: NodeJS.ProcessEnv): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, args, { stdio: 'inherit', env })
    child.on('error', reject)
    child.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`${bin} ${args.join(' ')} exited with ${code}`))))
  })
}

async function exists(file: string): Promise<boolean> {
  try {
    await access(file)
    return true
  } catch {
    return false
  }
}

export async function buildScenes(ids: string[], scenesDir: string = SCENES_DIR, bin = process.env.DCL_SCENE_CLI || 'dcl-scene-cli'): Promise<void> {
  const folders = await workspaceFolders(scenesDir)
  const wanted = ids.length === 0 ? folders : folders.filter((folder) => ids.includes(folder))
  const unknown = ids.filter((id) => !folders.includes(id))
  if (unknown.length > 0) throw new Error(`not in dcl-workspace.json: ${unknown.join(', ')}`)
  const env = environment(bin)
  for (const folder of wanted) {
    const dir = path.join(scenesDir, folder)
    if (!(await exists(path.join(dir, 'node_modules', '@dcl', 'sdk')))) {
      await run(bin, ['init', '--dir', dir, '--node-modules-only', '--yes'], env)
    }
    await run(bin, ['build', '--dir', dir, '--skip-install'], env)
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(SCENES_DIR, '..', 'scripts', 'build-scenes.ts')) {
  await buildScenes(process.argv.slice(2))
}
