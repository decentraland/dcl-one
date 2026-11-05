// The one loopback realm of the specs under this folder: every scene of `scenes/dcl-workspace.json`,
// built and served at its own parcel. `visualRealm()` starts it once per process, on the realm
// port, and registers its close for the end of the run; the spec files share the process
// (`--test-isolation=none`), so a second server on that port would fail.
import path from 'node:path'
import { after } from 'node:test'
import { buildScene, readTarget, serveRealm } from '../../driver/index.ts'
import { SCENES_DIR } from '../../driver/paths.ts'
import type { BuiltScene, RealmServer } from '../../driver/realm.ts'
import { workspaceFolders } from '../../scripts/build-scenes.ts'

let server: Promise<RealmServer> | undefined

async function build(id: string): Promise<BuiltScene> {
  try {
    return await buildScene(path.join(SCENES_DIR, id))
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    if (message.includes('build the scene first')) {
      throw new Error(`the scene "${id}" is not built (${message}); run \`npm run build-scenes\` in apps/bevy-explorer/e2e`, { cause: error })
    }
    throw error
  }
}

async function start(): Promise<RealmServer> {
  const ids = await workspaceFolders()
  const realm = await serveRealm({ scenes: await Promise.all(ids.map(build)), port: readTarget().ports.realm })
  after(() => realm.close())
  return realm
}

/** The realm, serving every workspace scene; started once per process. */
export function visualRealm(): Promise<RealmServer> {
  server ??= start()
  return server
}
