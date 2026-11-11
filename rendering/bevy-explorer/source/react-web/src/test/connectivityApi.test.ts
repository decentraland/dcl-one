import { runInNewContext } from 'node:vm'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, it, vi } from 'vitest'
import type { ConnectivitySnapshot } from '../engine/generated'

it('releases the engine stream when its JS consumer stops and sends Retry through its op', async () => {
  const snapshot: ConnectivitySnapshot = { connectionLost: true, probing: false, everConnected: true, suppressed: false, rooms: [] }
  const close = vi.fn()
  const retry = vi.fn()
  const read = vi.fn(async (id: number) => {
    expect(id).toBe(42)
    return snapshot
  })
  const module = { exports: {} as {
    getConnectivityStream: () => Promise<AsyncGenerator<ConnectivitySnapshot>>
    retryConnectivity: () => void
  } }
  const source = readFileSync(resolve(process.cwd(), '../crates/dcl_js/src/modules/SystemApi.js'), 'utf8')
  runInNewContext(source, { module, Deno: { core: { ops: {
    op_get_connectivity_stream: async () => 42,
    op_read_connectivity_stream: read,
    op_close_connectivity_stream: close,
    op_retry_connectivity: retry
  } } } })
  const stream = await module.exports.getConnectivityStream()
  expect(await stream.next()).toEqual({ value: snapshot, done: false })
  await stream.return(undefined)
  expect(close).toHaveBeenCalledWith(42)
  expect(read).toHaveBeenCalledOnce()
  module.exports.retryConnectivity()
  expect(retry).toHaveBeenCalledOnce()
})
