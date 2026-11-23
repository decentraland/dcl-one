import { afterEach, describe, expect, it, vi } from 'vitest'
import { choosePageIsolation, pageIsolation, servedByWorker, serviceWorkerUrl, shellMayMount } from './isolation'

function controlledBy(scriptURL: string | null): void {
  vi.stubGlobal('navigator', { ...navigator, serviceWorker: { controller: scriptURL == null ? null : { scriptURL } } })
}

afterEach(() => {
  localStorage.clear()
  vi.unstubAllGlobals()
})

describe('page isolation (a browser without Document-Isolation-Policy)', () => {
  it('is off until chosen: the plain worker URL, and an isolated shell may not mount the app', () => {
    vi.stubGlobal('crossOriginIsolated', true)
    expect(pageIsolation()).toBe(false)
    expect(serviceWorkerUrl('https://play.example/play/').href).toBe('https://play.example/play/service_worker.js')
    expect(shellMayMount()).toBe(false)
  })

  it('is chosen by an unisolated shell with a service worker, then names the worker that gives it', () => {
    vi.stubGlobal('crossOriginIsolated', false)
    controlledBy('https://play.example/play/service_worker.js')
    expect(choosePageIsolation()).toBe(true)
    expect(pageIsolation()).toBe(true)
    expect(serviceWorkerUrl('https://play.example/play/').href).toBe('https://play.example/play/service_worker.js?isolation=page')
    // chosen once: asking again cannot help
    expect(choosePageIsolation()).toBe(false)
  })

  it('is not chosen by an isolated shell or without a service worker', () => {
    vi.stubGlobal('crossOriginIsolated', true)
    controlledBy(null)
    expect(choosePageIsolation()).toBe(false)
    vi.stubGlobal('crossOriginIsolated', false)
    vi.stubGlobal('navigator', {})
    expect(choosePageIsolation()).toBe(false)
    expect(pageIsolation()).toBe(false)
  })

  it('counts a load as served only by a worker of the chosen isolation, with the isolation it gives', () => {
    vi.stubGlobal('crossOriginIsolated', false)
    controlledBy('https://play.example/play/service_worker.js')
    expect(servedByWorker()).toBe(true)
    localStorage.setItem('dcl-shell-isolation', 'page')
    // the plain worker served this load: the page worker must take over first
    expect(servedByWorker()).toBe(false)
    controlledBy('https://play.example/play/service_worker.js?isolation=page')
    vi.stubGlobal('crossOriginIsolated', true)
    expect(servedByWorker()).toBe(true)
    expect(shellMayMount()).toBe(true)
    controlledBy(null)
    expect(servedByWorker()).toBe(false)
  })
})
