import assert from 'node:assert/strict'
import { spec, until } from '../driver/index.ts'

const scenario = spec(import.meta.url)
const panel = { role: 'dialog', name: 'Debug panel' } as const
const button = (name: string) => ({ role: 'button', name } as const)

scenario('every registered debug reading resolves or explains what it is waiting for', async ({ explorer }) => {
  const sections = await explorer.json<{ title: string; query: string; fields: [string, string][] }[]>('/debug_panel')
  assert.ok(sections.length > 0)
  const replies = new Map<string, Record<string, unknown>>()
  for (const section of sections) {
    if (!replies.has(section.query)) replies.set(section.query, await explorer.json(section.query))
    const reply = replies.get(section.query)!
    for (const [label, path] of section.fields) {
      let value: unknown = reply
      for (const key of path.split('.')) value = value && typeof value === 'object'
        ? (value as Record<string, unknown>)[key] : undefined
      if (value != null) continue
      const explained = Object.entries(reply.unavailable ?? {}).some(([prefix, reason]) =>
        typeof reason === 'string' && reason.trim() && (prefix === '' || path === prefix || path.startsWith(prefix + '.')))
      assert.ok(explained, `${section.title} → ${label}: ${section.query} returned no value or explanation for ${path}`)
    }
  }
}, { session: { signIn: 'guest' }, closeSession: true })

scenario('the debug panel shows live plugin data and can play an avatar emote', async ({ explorer }) => {
  const filter = { name: 'Find debug controls' }
  const expand = async (title: string) => {
    await explorer.hud.fill(filter, title)
    await explorer.hud.click(button(title))
  }
  const before = await explorer.json<{ overrideFps: number | null }>('/performance_state')
  const developmentBuild = await explorer.json<boolean>('/debug_panel --dev-build')
  const sidebarSettings = button('Sidebar settings')
  const nativeHud = explorer.console.kind === 'native'
  if (nativeHud) {
    await explorer.hud.click(sidebarSettings)
    await explorer.hud.wait(button('Debug panel'))
    const cef = await until(() => explorer.json<{
      paintSize: [number, number] | null
      bitmapMiBPerSecond: number | null
    }>('/cef_hud_perf'), (reading) => (reading.bitmapMiBPerSecond ?? 0) > 0,
    { timeoutMs: 10_000, what: 'a CEF repaint while the debug panel is closed' })
    assert.ok(cef.paintSize?.every(size => size > 0))
    if (developmentBuild) await explorer.hud.click(sidebarSettings)
  }
  const waitTarget = (fps: number) => until(() => explorer.hud.get(panel),
    (read) => read.text.includes('Target frame rate (FPS) ' + fps),
    { timeoutMs: 10_000, what: 'the open panel to show a target of ' + fps + ' FPS' })
  const launcher = button('Toggle debug panel')
  if (developmentBuild) {
    await explorer.hud.click(launcher)
  } else {
    assert.equal(await explorer.hud.isVisible(launcher), false)
    if (!nativeHud) await explorer.hud.click(sidebarSettings)
    await explorer.hud.click(button('Debug panel'))
  }
  try {
    await explorer.hud.wait(panel)
    if (nativeHud) {
      await expand('CEF HUD')
      await until(() => explorer.hud.get(panel), (read) => {
        const text = read.text.replace(/\s+/g, ' ')
        return /Bitmap traffic \(MiB\/s, last 2 s\) \d+(?:\.\d+)?\b/.test(text)
      }, { timeoutMs: 10_000, what: 'the panel to display measured CEF bitmap traffic' })
    }
    await expand('World')
    const world = await explorer.worldState()
    await until(() => explorer.hud.get(panel), (read) => read.text.includes(new URL(world.realm).origin), {
      timeoutMs: 10_000, what: 'the panel to display the current realm'
    })
    await expand('Legacy avatar events')
    await until(async () => ({ state: await explorer.legacySocial(), read: await explorer.hud.get(panel) }), ({ state, read }) => {
      assert.ok(Object.values(state).every(value => Number.isInteger(value) && value >= 0))
      assert.ok(state.queuedEvents <= state.subscribedScenes * 1024)
      assert.ok(state.resyncScenes <= state.subscribedScenes)
      const text = read.text.replace(/\s+/g, ' ')
      return [
        ['Subscribed scenes', state.subscribedScenes], ['Queued events', state.queuedEvents],
        ['Scenes awaiting resync', state.resyncScenes], ['Tracked peers', state.trackedPeers]
      ].every(([label, value]) => new RegExp(`${label} ${value}(?:\\s|$)`).test(text))
    }, { timeoutMs: 10_000, what: 'the panel to display the engine legacy avatar queue counts' })
    await explorer.run('/session_graphics fps 47')
    await expand('Performance')
    await waitTarget(47)
    await explorer.hud.click(button('Limit to 30 FPS'))
    await waitTarget(30)
    assert.equal(await explorer.hud.isVisible(button('Limit to 30 FPS')), false)
    await explorer.hud.click(button('Use configured FPS'))
    await explorer.hud.wait(button('Limit to 30 FPS'))
    assert.equal((await explorer.json<{ overrideFps: number | null }>('/performance_state')).overrideFps, null)
    await expand('Avatar')
    // a scene's movement animation outranks a triggered emote while it is not idle (the scene
    // under the player can drive one): the wave goes out once the avatar would take it
    await until(async () => (await explorer.myAvatar()).emote, (emote) => !(emote?.source === 'sceneMovementAnim' && emote.idle === false),
      { timeoutMs: 20_000, intervalMs: 100, what: "the scene under the player to let its movement animation go (it outranks a triggered emote)" })
    await explorer.hud.click(button('Play wave'))
    await until(async () => (await explorer.myAvatar()).emote, (emote) =>
      emote?.source === 'triggeredEmote' && !emote.finished && /(?:^|:)wave$/i.test(emote.urn),
      { timeoutMs: 20_000, intervalMs: 100, what: 'the debug action to play the wave emote' })
  } finally {
    await explorer.run(before.overrideFps == null ? '/session_graphics fps' : '/session_graphics fps ' + before.overrideFps)
    if (await explorer.hud.isVisible(panel)) await explorer.escape()
    await explorer.hud.waitHidden(panel)
  }
  assert.equal(await explorer.hud.isVisible(button('Play wave')), false)
})

// An unsigned guest, the session-only one the engine's `/login_guest` signs in (as a scene
// preview's `?guest=1` does), has no signed identity to send a client login with. The HUD's
// Continue as guest makes a guest account, which signs and sends them like any account. The
// session's HUD stays on its start page, so the scenario gets no opening Escape.
scenario('an unsigned guest does not produce authenticated client login events', async ({ explorer }) => {
  const world = await explorer.worldState()
  assert.ok(world.signedIn && world.address, 'the unsigned guest should be signed in')
  assert.equal(world.guest, false, 'an unsigned guest is not a guest account')
  const state = await explorer.json<{ active: boolean; inFlight: boolean; pending: number; memoryOnly: boolean; storageFailures: number }>('/client_login_state')
  assert.equal(typeof state.memoryOnly, 'boolean')
  assert.equal(typeof state.storageFailures, 'number')
  assert.equal(state.active, false)
  assert.equal(state.inFlight, false)
  assert.ok(Number.isInteger(state.pending) && state.pending >= 0)
}, { session: { signIn: 'unsigned-guest' }, noEscape: true, closeSession: true })
