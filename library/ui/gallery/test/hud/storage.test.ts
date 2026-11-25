import { readFileSync } from 'node:fs'
import path from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'

// The origin's own storage, as the Explorer's /play HUD on the same origin would read it.
const origin = {
  localStorage: window.localStorage,
  sessionStorage: window.sessionStorage,
  indexedDB: { open: vi.fn(), deleteDatabase: vi.fn(), databases: vi.fn() },
  cookie: Object.getOwnPropertyDescriptor(Document.prototype, 'cookie')!
}
vi.stubGlobal('indexedDB', origin.indexedDB)

afterEach(() => {
  vi.restoreAllMocks()
  vi.resetModules()
})

describe('when the gallery HUD canvas loads on the origin /play serves', () => {
  it('should load the storage isolation before any HUD module', () => {
    const main = readFileSync(path.resolve(import.meta.dirname, '../../src/hud/main.tsx'), 'utf8')
    const imports = [...main.matchAll(/^import .*'([^']+)'$/gm)].map((match) => match[1])
    expect(imports[0]).toBe('./isolateStorage')
  })

  it('should keep a HUD preference in this page alone, outside a shell', async () => {
    await import('../../src/hud/isolateStorage')
    const prefs = await import('@explorer-core/lib/prefs')
    prefs.setPref(prefs.PREF.minimapZoom, '3')
    expect(prefs.getPref(prefs.PREF.minimapZoom)).toBe('3')
    expect(origin.localStorage.getItem(prefs.PREF.minimapZoom)).toBeNull()
    expect(window.localStorage).not.toBe(origin.localStorage)
  })

  it('should keep a HUD preference in memory and never open the origin IndexedDB inside the canvas iframe', async () => {
    vi.spyOn(window, 'parent', 'get').mockReturnValue({} as Window)
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    await import('../../src/hud/isolateStorage')
    const prefs = await import('@explorer-core/lib/prefs')
    await prefs.loadPrefs()
    prefs.setPref(prefs.PREF.voiceVolume, '0.2')
    await vi.waitFor(() =>
      expect(warn).toHaveBeenCalledWith(
        '[prefs] could not store',
        prefs.PREF.voiceVolume,
        expect.anything()
      )
    )
    expect(prefs.getPref(prefs.PREF.voiceVolume)).toBe('0.2')
    expect(origin.indexedDB.open).not.toHaveBeenCalled()
  })

  it('should keep session storage and cookies in this page alone', async () => {
    await import('../../src/hud/isolateStorage')
    window.sessionStorage.setItem('dcl-analytics-queue', '[]')
    document.cookie = 'hud=1; path=/'
    expect(origin.sessionStorage.getItem('dcl-analytics-queue')).toBeNull()
    expect(origin.cookie.get!.call(document)).not.toContain('hud=1')
    expect(document.cookie).toBe('hud=1')
  })

  it('should refuse Cache Storage instead of writing the origin cache', async () => {
    await import('../../src/hud/isolateStorage')
    expect(() => (window as unknown as { caches: CacheStorage }).caches.open('hud')).toThrow(
      'keeps no Cache Storage'
    )
  })
})
