// DOMAIN: the benchmark's switches (`autopilot`, `perfStream`, `perfCsv`; docs/guidelines/benchmark.md)
// belong to the harness. A page acts on them on a loopback host and in the native client, and
// anywhere else ignores them outright (lib/bootMode.ts `benchmarkSwitchesHonoured`): no autopilot
// boot mode, nothing handed to the engine, no stream, and no gate to click through.

import { render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import { EngineRpc } from '../engine/engineRpc'
import { EngineHost, entryEngineOptions } from '../features/engine/EngineHost'
import { benchmarkSwitchesHonoured, bootMode } from '../lib/bootMode'
import { untrustedLaunchParams } from '../lib/launchGate'
import { isLoopbackHost } from '../lib/loopbackHost'
import { leave, visit } from './entryUrl'

const HARNESS_URL = 'http://localhost:8080/?autopilot&portables=a;b&perfStream=/__perf/change_build_1&perfCsv'
const LINK_URL = 'https://decentraland.org/?autopilot&portables=a;b&perfStream=/__perf/change_build_1&perfCsv'

afterEach(leave)

describe('isLoopbackHost', () => {
  it('takes only loopback hosts as the harness', () => {
    for (const host of ['localhost', 'perf.localhost', '127.0.0.1', '[::1]']) expect(isLoopbackHost(host)).toBe(true)
    for (const host of ['decentraland.org', 'localhost.example', '127.0.0.1.example', '10.0.0.1']) expect(isLoopbackHost(host)).toBe(false)
  })
})

describe('benchmarkSwitchesHonoured', () => {
  it('is true on loopback hosts and for the native client, false anywhere else', () => {
    for (const host of ['localhost', 'perf.localhost', '127.0.0.1', '[::1]']) {
      expect(benchmarkSwitchesHonoured({ host, native: false })).toBe(true)
    }
    for (const host of ['decentraland.org', 'play.decentraland.zone', 'localhost.example', '127.0.0.1.example', '10.0.0.1', '0.0.0.0', '']) {
      expect(benchmarkSwitchesHonoured({ host, native: false })).toBe(false)
      // the native client's `--autopilot` is the user's own flag
      expect(benchmarkSwitchesHonoured({ host, native: true })).toBe(true)
    }
  })

  it('reads the page it runs on', () => {
    visit(HARNESS_URL)
    expect(benchmarkSwitchesHonoured()).toBe(true)
    visit(LINK_URL)
    expect(benchmarkSwitchesHonoured()).toBe(false)
  })
})

describe('the autopilot boot mode', () => {
  it('is entered on a loopback host', () => {
    visit(HARNESS_URL)
    expect(bootMode().autoLogin).toBe('autopilot')
    visit('http://127.0.0.1:8080/?autopilot=')
    expect(bootMode().autoLogin).toBe('autopilot')
  })

  it('is never entered off loopback: the page boots as an ordinary visit', () => {
    visit(LINK_URL)
    expect(bootMode()).toEqual({ systemScene: null, hideHud: false, autoLogin: null })
    visit('https://decentraland.org/?autopilot=')
    expect(bootMode().autoLogin).toBeNull()
  })

  it('leaves the other boot modes as they were', () => {
    visit('https://decentraland.org/?guest=1&autopilot')
    expect(bootMode().autoLogin).toBe('guest')
    visit('http://localhost:8080/?guest=1&autopilot')
    expect(bootMode().autoLogin).toBe('guest')
  })
})

describe('what the engine is handed', () => {
  const entry = (search: string) => new URLSearchParams(search)

  it('is the autopilot flag and the stream where the page honours the switches', () => {
    const { launch, perfStream } = entryEngineOptions(entry(HARNESS_URL.split('?')[1]), true)
    expect(launch.autopilot).toBe(true)
    expect(perfStream).toEqual({ path: '/__perf/change_build_1', csv: true })
    expect(entryEngineOptions(entry('?perfStream=/__perf/b'), true).perfStream).toEqual({ path: '/__perf/b', csv: false })
    // the flag stays false (not absent) when the link does not set it
    expect(entryEngineOptions(entry('?portables=a'), true).launch.autopilot).toBe(false)
  })

  it('has no autopilot key and no stream anywhere else', () => {
    const { launch, perfStream } = entryEngineOptions(entry(LINK_URL.split('?')[1]), false)
    expect('autopilot' in launch).toBe(false)
    expect(perfStream).toBeNull()
    // the rest of the entry url is handed over as it always was
    expect(Object.keys(launch)).toEqual(Object.keys(entryEngineOptions(entry('?portables=a'), true).launch).filter((name) => name !== 'autopilot'))
    expect(launch.portables).toBe('a;b')
  })

  it('has nothing to approve off loopback: no gate offers to let the switches through', () => {
    visit(LINK_URL)
    expect(untrustedLaunchParams({ native: false })).toEqual([])
  })
})

// The one place the boot config is really written: EngineHost injects the engine once per page, so
// this is the file's only render. It stands on a link to a public deployment.
describe('EngineHost on a link', () => {
  it('writes a boot config with no autopilot key and sets up no stream', () => {
    visit(LINK_URL)
    const w = window as unknown as { __bevyBootConfig?: Record<string, unknown>; __bevyPerfStream?: unknown }
    try {
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
    }
  })
})
