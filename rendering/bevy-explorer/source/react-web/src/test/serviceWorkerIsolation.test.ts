// The service worker's isolation headers. Without `?isolation=page` the app gets
// Document-Isolation-Policy and the shell stays unisolated; with it (a browser without that policy,
// library/explorer-core/lib/isolation.ts) the shell, the app and their workers get COEP credentialless.
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it, vi } from 'vitest'

function serviceWorker(script: string) {
  const listeners: Record<string, (event: unknown) => void> = {}
  // a same-origin response, as the browser's fetch gives one
  const network = vi.fn(async () => {
    const response = new Response('', { headers: { 'Cross-Origin-Embedder-Policy': 'require-corp' } })
    Object.defineProperty(response, 'type', { value: 'basic' })
    return response
  })
  const self = {
    addEventListener: (type: string, fn: (event: unknown) => void) => (listeners[type] = fn),
    registration: { scope: 'https://play.example/play/' },
    location: new URL(script)
  }
  const source = readFileSync(resolve(__dirname, '../../../deploy/web/service_worker.js'), 'utf8')
  const load = new Function('self', 'caches', 'fetch', 'console', source) as (...args: unknown[]) => void
  load(self, { open: vi.fn() }, network, { log: vi.fn(), warn: vi.fn() })

  // the headers the worker answers a request with, or null when it leaves the request to the browser
  return async (path: string, destination: string): Promise<Headers | null> => {
    const request = { url: 'https://play.example' + path, mode: destination === 'document' ? 'navigate' : 'cors', destination }
    const replied: { response?: Promise<Response> } = {}
    listeners.fetch({ request, clientId: '', respondWith: (r: Promise<Response>) => (replied.response = r) })
    return replied.response ? (await replied.response).headers : null
  }
}

describe('service worker isolation headers', () => {
  it('without page isolation: Document-Isolation-Policy for the app, no COEP for the shell, workers untouched', async () => {
    const headers = serviceWorker('https://play.example/play/service_worker.js')
    const shell = await headers('/play/', 'document')
    expect(shell?.get('Cross-Origin-Embedder-Policy')).toBeNull()
    expect(shell?.get('Document-Isolation-Policy')).toBeNull()
    const app = await headers('/play/app.html', 'document')
    expect(app?.get('Document-Isolation-Policy')).toBe('isolate-and-credentialless')
    expect(app?.get('Cross-Origin-Embedder-Policy')).toBeNull()
    expect(await headers('/play/engine/pkg/worker.js', 'worker')).toBeNull()
  })

  it('with page isolation: COOP and COEP credentialless for the shell, the app and their workers', async () => {
    const headers = serviceWorker('https://play.example/play/service_worker.js?isolation=page')
    for (const [path, destination] of [
      ['/play/', 'document'],
      ['/play/index.html', 'document'],
      ['/play/app.html', 'document'],
      ['/play/engine/headless.html', 'document'],
      ['/play/engine/pkg/worker.js', 'worker']
    ]) {
      const h = await headers(path, destination)
      expect(h?.get('Cross-Origin-Opener-Policy'), path).toBe('same-origin')
      expect(h?.get('Cross-Origin-Embedder-Policy'), path).toBe('credentialless')
    }
  })
})
