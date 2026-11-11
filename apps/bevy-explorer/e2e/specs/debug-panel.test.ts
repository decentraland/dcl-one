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
}, { session: { signIn: 'guest' } })

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
    await explorer.run('/session_graphics fps 47')
    await expand('Performance')
    await waitTarget(47)
    await explorer.hud.click(button('Limit to 30 FPS'))
    await waitTarget(30)
    assert.equal(await explorer.hud.isVisible(button('Limit to 30 FPS')), false)
    await explorer.hud.click(button('Use configured FPS'))
    await explorer.hud.wait(button('Limit to 30 FPS'))
    assert.equal((await explorer.json<{ overrideFps: number | null }>('/performance_state')).overrideFps, null)
    await expand('Profiler')
    await explorer.hud.click(button('Enable profiler'))
    await until(() => explorer.hud.get(panel), (read) => read.text.includes('Disable profiler'), {
      timeoutMs: 10_000, what: 'the profiler to report itself enabled'
    })
    const top = await until(() => explorer.json<ProfileTop>('/profile_top 5'),
      (reading) => reading.enabled && reading.scopeSummaries.length === 5,
      { timeoutMs: 10_000, what: 'five profiled scopes' })
    for (const row of top.scopeSummaries) {
      assert.ok(['ecs_system', 'schedule', 'render_graph_node', 'bevy_internal', 'explorer_span'].includes(row.scopeKind), row.scopeKind)
      assert.ok(row.callCount > 0 && row.selfTimeMs >= 0 && row.inclusiveTimeMs >= row.selfTimeMs, JSON.stringify(row))
    }
    await explorer.hud.click(button('Disable profiler'))
    await explorer.hud.wait(button('Enable profiler'))
    const off = await explorer.json<ProfileTop>('/profile_top')
    assert.equal(off.enabled, false)
    assert.match(String(off.unavailable.scopeSummary), /disabled/)
    await expand('Avatar')
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

type ProfileTop = {
  enabled: boolean
  scopeSummaries: { scopeName: string; scopeKind: string; callCount: number; selfTimeMs: number; inclusiveTimeMs: number }[]
  unavailable: Record<string, string>
}

type PerformanceState = {
  sampling: { status: 'sampling' | 'paused'; pausedReason: string | null }
  inProgressSample: { sampledSeconds: number }
  cpuFrameTimes: { hiccupThresholdMs: number; sampleCount: number; p99Ms: number } | null
  frameTimeBreakdown: (number | null)[]
  bottleneck: string | null
  entityCount: number | null
  unavailable: Record<string, string>
}

scenario('the performance sampler samples in world and scales the hiccup threshold to the target', async ({ explorer }) => {
  const state = await until(() => explorer.json<PerformanceState>('/performance_state'),
    (reading) => reading.sampling.status === 'sampling' && reading.inProgressSample.sampledSeconds > 0,
    { timeoutMs: 20_000, what: 'the sampler to run in world' })
  assert.equal(state.sampling.pausedReason, null)
  assert.equal(state.frameTimeBreakdown.length, 6)
  assert.ok(state.frameTimeBreakdown.slice(0, 4).every((ms) => typeof ms === 'number' && ms >= 0), JSON.stringify(state.frameTimeBreakdown))
  assert.ok(state.bottleneck === null || ['main_thread_cpu', 'gpu', 'present_wait', 'balanced'].includes(state.bottleneck), String(state.bottleneck))
  assert.ok((state.entityCount ?? 0) > 0)
  if (state.cpuFrameTimes == null) assert.match(state.unavailable.cpuFrameTimes, /sampled time/)
  const before = await explorer.json<{ overrideFps: number | null }>('/performance_state')
  try {
    await explorer.run('/session_graphics fps 30')
    const capped = await until(() => explorer.json<PerformanceState>('/performance_state'),
      (reading) => reading.cpuFrameTimes != null && Math.abs(reading.cpuFrameTimes.hiccupThresholdMs - 66.7) < 0.1,
      { timeoutMs: 40_000, what: 'a snapshot taken at the 30 FPS cap, with its 66.7 ms threshold' })
    assert.ok(capped.cpuFrameTimes!.sampleCount > 0)
    assert.ok(capped.cpuFrameTimes!.p99Ms > 0)
  } finally {
    await explorer.run(before.overrideFps == null ? '/session_graphics fps' : '/session_graphics fps ' + before.overrideFps)
  }
}, { session: { signIn: 'guest' } })

scenario('an unsigned guest does not produce authenticated client login events', async ({ explorer }) => {
  const state = await explorer.json<{ active: boolean; inFlight: boolean; pending: number; memoryOnly: boolean; storageFailures: number }>('/client_login_state')
  assert.equal(typeof state.memoryOnly, 'boolean')
  assert.equal(typeof state.storageFailures, 'number')
  assert.equal(state.active, false)
  assert.equal(state.inFlight, false)
  assert.ok(Number.isInteger(state.pending) && state.pending >= 0)
}, { session: { signIn: 'guest' } })
