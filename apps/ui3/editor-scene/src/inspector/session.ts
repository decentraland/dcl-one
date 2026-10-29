import { cmd } from '../cmd'
import { autoLogin } from '../login'
import { getCurrentInspectableScene } from '../current-scene'
import { state } from '../state'
import { decodeCustomComponents } from '../custom-components'
import { freezeAnimatables, reassertAuthoredIntoSnapshot } from '../edit-freeze'
import { stripPickColliders } from '../viewport/pick-layer'
import { sleep } from '../utils'
import { loadComponentNames } from './component-ops'

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

export const SETTLE_MS = 150
export async function reloadAfter(goneIds: string[] = []): Promise<void> {
  if (state.frozen) return
  for (let attempt = 0; attempt < 6; attempt++) {
    await sleep(SETTLE_MS)
    await reloadSnapshot()
    if (goneIds.every((id) => !(id in state.snapshot))) return
  }
}

export async function ensureProjectScenePinned(hash: string): Promise<boolean> {
  for (let attempt = 0; attempt < 8; attempt++) {
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
    await sleep(SETTLE_MS)
  }
  return false
}
