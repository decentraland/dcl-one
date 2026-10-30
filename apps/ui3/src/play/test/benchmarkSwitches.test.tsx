// DOMAIN: /play never runs the benchmark (README "The HUD"). The unattended run and the log stream
// are the web explorer's on loopback hosts, and Sites carries neither: whatever the link or the
// host, `autopilot` stays out of the engine's launch options (the key is absent, not `false`, so an
// engine that predates it still launches), no stream is set up, no boot mode takes over the
// sign-in, and there is no gate to click through.

import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { EngineRpc } from '../engine/engineRpc'
import { EngineHost } from '../features/engine/EngineHost'
import { bootMode } from '../lib/bootMode'
import { untrustedLaunchParams } from '../lib/launchGate'

const SEARCH = '?autopilot&portables=a;b&perfStream=/__perf/change_build_1&perfCsv'

describe('/play and the benchmark switches', () => {
  it('takes no autopilot boot mode, and sets up no gate for the switches', () => {
    window.history.replaceState(null, '', '/' + SEARCH)
    try {
      expect(bootMode().autoLogin).toBeNull()
      expect(untrustedLaunchParams({ native: false })).toEqual([])
    } finally {
      window.history.replaceState(null, '', '/')
    }
  })

  // EngineHost injects the engine once per page, so this is the file's only render. The page is on
  // localhost, the one host where the web explorer's own HUD would honour the switches.
  it('hands the engine no autopilot key and sets up no stream, even on loopback', () => {
    window.history.replaceState(null, '', '/' + SEARCH)
    const w = window as unknown as { __bevyBootConfig?: Record<string, unknown>; __bevyPerfStream?: unknown }
    try {
      expect(location.hostname).toBe('localhost')
      expect(w.__bevyBootConfig).toBeUndefined()
      render(<EngineHost rpc={new EngineRpc()} />)
      const config = w.__bevyBootConfig
      expect(config).toBeDefined()
      expect('autopilot' in config!).toBe(false)
      expect(config!.portables).toBe('a;b')
      expect(w.__bevyPerfStream).toBeUndefined()
    } finally {
      delete w.__bevyBootConfig
      document.querySelectorAll('head script').forEach((script) => script.remove())
      window.history.replaceState(null, '', '/')
    }
  })
})
