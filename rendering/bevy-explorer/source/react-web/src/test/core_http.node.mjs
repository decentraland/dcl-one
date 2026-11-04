import assert from 'node:assert/strict'
import test from 'node:test'
import * as http from '../../../deploy/web/engine/core_http.js'

test('starvation requires sustained pressure without a terminal request', () => {
  let now = 0
  const host = http.createCoreHttpHost(() => now)
  for (let id = 0; id < 14; id++) host.begin(id)
  now = 6000
  assert.equal(host.snapshot().suspectedStarvation, false)
  host.begin(14)
  now = 10999
  host.begin(15)
  assert.equal(host.snapshot().suspectedStarvation, false)
  now++
  assert.deepEqual(host.snapshot(), {active: 16, highWater: 16, oldestActiveMs: 11000, suspectedStarvation: true})
  host.end(99)
  assert.equal(host.snapshot().suspectedStarvation, true)
  host.end(0)
  assert.equal(host.snapshot().suspectedStarvation, false)
  now += 5000
  assert.equal(host.snapshot().suspectedStarvation, true)
  host.end(1)
  now += 5000
  assert.equal(host.snapshot().suspectedStarvation, false)
  host.dispose()
  assert.deepEqual(host.snapshot(), {active: 0, highWater: 16, oldestActiveMs: 0, suspectedStarvation: false})
})

test('the 16th request dispatches synchronously with the original input, returns at headers, and survives a broken reporter', async t => {
  const host = http.installCoreHttp()
  for (let id = 0; id < 15; id++) http.beginCoreHttp(id)
  t.after(() => http.disposeCoreHttp())
  const request = new Request('https://example.test/')
  const options = { referrerPolicy: 'no-referrer' }
  const response = new Response(new ReadableStream({}))
  const headers = Promise.withResolvers()
  const fetch = t.mock.method(globalThis, 'fetch', () => headers.promise)
  const pending = http.coreFetch(request, options)
  assert.deepEqual(fetch.mock.calls[0].arguments, [request, options])
  assert.equal(fetch.mock.calls[0].arguments[0], request)
  assert.equal(host.snapshot().active, 16)
  headers.resolve(response)
  assert.equal(await pending, response)
  assert.equal(response.bodyUsed, false)
  assert.equal(host.snapshot().active, 15)
  http.disposeCoreHttp()
  globalThis.__dclCoreHttp = { nextId() { throw Error('offline') } }
  assert.equal(await http.coreFetch(request, options), response)
  fetch.mock.mockImplementation(() => { throw Error('network failure') })
  await assert.rejects(http.coreFetch(request), /network failure/)
})

test('reader completion, malformed JSON and abort retire the observed transfer', async t => {
  const host = http.installCoreHttp()
  t.after(() => http.disposeCoreHttp())
  let stream
  const response = new Response(new ReadableStream({ start(controller) { stream = controller } }))
  const fetch = t.mock.method(globalThis, 'fetch', () => Promise.resolve(response))
  const reading = http.coreFetchJson('https://example.test/')
  await new Promise(resolve => setImmediate(resolve))
  assert.equal(host.snapshot().active, 1)
  stream.enqueue(new TextEncoder().encode('{'))
  stream.close()
  await assert.rejects(reading, SyntaxError)
  assert.equal(host.snapshot().active, 0)
  const controller = new AbortController()
  fetch.mock.mockImplementation((_, {signal}) => new Promise((_, reject) => {
    signal.addEventListener('abort', () => reject(signal.reason))
  }))
  const cancelled = http.coreFetch('https://example.test/', {signal: controller.signal})
  controller.abort()
  await assert.rejects(cancelled, {name: 'AbortError'})
  assert.equal(host.snapshot().active, 0)
})

test('owned status and error readers cancel an unused response before the observation ends', async t => {
  const host = http.installCoreHttp()
  t.after(() => http.disposeCoreHttp())
  const duringCancel = []
  const fetch = t.mock.method(globalThis, 'fetch', () => Promise.resolve(new Response(new ReadableStream({
    cancel() {
      duringCancel.push(host.snapshot().active)
      return Promise.reject(Error('underlying cancellation failed'))
    },
  }), {status: 503})))
  await assert.rejects(http.coreFetchJson('https://example.test/'), /503/)
  assert.equal(await http.coreFetch('https://example.test/', undefined, r => r.status), 503)
  assert.deepEqual(duringCancel, [1, 1])
  assert.equal(host.snapshot().active, 0)

  const callerOwned = new Response('stream')
  fetch.mock.mockImplementation(() => Promise.resolve(callerOwned))
  assert.equal(await http.coreFetch('https://example.test/'), callerOwned)
  assert.equal(await callerOwned.text(), 'stream')
})
