import { backoffDelay } from '../../../../../library/async'
import { cmd } from '../cmd'
import { autoLogin } from '../login'
import { getCurrentInspectableScene } from '../current-scene'
import { state } from '../state'
import { decodeCustomComponents } from '../custom-components'
import { freezeAnimatables, reassertAuthoredIntoSnapshot } from '../edit-freeze'
import { stripPickColliders } from '../viewport/pick-layer'
import { sleep } from '../utils'
import { loadComponentNames } from './component-ops'

const SNAPSHOT_TIMEOUT_MS = 1200
const PROJECT_PIN_TIMEOUT_MS = 1200
const SNAPSHOT_BACKOFF = { initialMs: 25, maxMs: 200, factor: 2 }

export async function startInspector(): Promise<void> {
  state.status = 'logging-in'
  await autoLogin()
  await refresh()
  loadComponentNames().catch(console.error)
}

export async function refresh(): Promise<void> {
  state.status = 'loading-snapshot'
  state.error = ''

  console.log('[boot] resolving current scene…')
  const scene = await getCurrentInspectableScene()
  console.log(`[boot] current scene: ${scene?.hash ?? 'none'}`)
  if (scene === undefined) {
    state.scene = undefined
    state.status = 'no-scene'
    return
  }
  state.scene = scene

  try {
    await cmd.setScene(scene.hash)
  } catch (e) {
    console.error('set_scene failed:', e)
  }

  console.log('[boot] syncing frozen state…')
  await syncFrozenState()
  console.log('[boot] loading snapshot…')
  if (await reloadSnapshot()) console.log('[boot] snapshot loaded; editor ready')
  else console.log(`[boot] snapshot unavailable; waiting for project runtime: ${state.error}`)
}

async function syncFrozenState(): Promise<void> {
  try {
    const stats = await cmd.sceneStats()
    state.frozen = /status:\s*blocked/i.test(stats)
  } catch {
  }
}

export async function reloadSnapshot(): Promise<boolean> {
  state.status = 'loading-snapshot'
  try {
    state.snapshot = await cmd.crdtSnapshot()
    stripPickColliders(state.snapshot)
    decodeCustomComponents(state.snapshot)
    reassertAuthoredIntoSnapshot(state.snapshot)
    await freezeAnimatables()
    state.error = ''
    state.status = 'ready'
    return true
  } catch (e) {
    state.error = String(e)
    state.status = 'error'
    return false
  }
}

/**
 * Reloads the snapshot until `ready` holds or the budget ends, and says which. Best effort, as the
 * fixed six reloads were: a snapshot that lags the edit is not an error for a write that succeeded.
 */
export async function reloadUntil(ready: () => boolean, timeoutMs = SNAPSHOT_TIMEOUT_MS): Promise<boolean> {
  const deadline = Date.now() + timeoutMs
  let attempt = 0
  for (;;) {
    if (await reloadSnapshot() && ready()) return true
    const remaining = deadline - Date.now()
    if (remaining <= 0) return false
    await sleep(backoffDelay(attempt++, SNAPSHOT_BACKOFF, remaining))
  }
}

export async function reloadAfter(goneIds: string[] = []): Promise<void> {
  if (state.frozen) return
  await reloadUntil(() => goneIds.every((id) => !(id in state.snapshot)))
}

export async function ensureProjectScenePinned(hash: string): Promise<boolean> {
  const deadline = Date.now() + PROJECT_PIN_TIMEOUT_MS
  let attempt = 0
  for (;;) {
    try {
      await cmd.setScene(hash)
    } catch (e) {
    }
    try {
      const stats = await cmd.sceneStats()
      const esc = hash.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
      if (new RegExp(`hash:\\s*${esc}(?:\\s|$)`, 'm').test(stats)) return true
    } catch {
    }
    const remaining = deadline - Date.now()
    if (remaining <= 0) return false
    await sleep(backoffDelay(attempt++, SNAPSHOT_BACKOFF, remaining))
  }
}
