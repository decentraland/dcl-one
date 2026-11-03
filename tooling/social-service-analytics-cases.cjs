#!/usr/bin/env node
// Runs the installed source analytics/fetch SDKs with controlled responses and
// a local redirect server. No request reaches a real analytics endpoint.
const fs = require('node:fs')
const http = require('node:http')
const { req, revision } = require('./social-service-voice-source-cases.cjs')
const { createAnalyticsComponent } = req('@dcl/analytics-component')
const { createFetchComponent } = req('@dcl/fetch-component')
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const base = { ANALYTICS_CONTEXT: 'social-fixture', ANALYTICS_API_URL: 'https://analytics.example/events', ANALYTICS_API_TOKEN: 'fixture-token', ENV: 'fixture' }
const encoded = value => Number.isFinite(value) ? value : String(value)
const quiet = entries => ({ getLogger: () => ({ warn: message => entries.push({ level: 'warn', message }), error: (message, data) => entries.push({ level: 'error', message, data }) }) })
async function main() {
  const configuration = []
  for (const omitted of [null, ...Object.keys(base), 'all-empty']) {
    const values = omitted === 'all-empty' ? Object.fromEntries(Object.keys(base).map(key => [key, ''])) : { ...base }
    if (omitted && omitted !== 'all-empty') delete values[omitted]
    let request
    try {
      const analytics = await createAnalyticsComponent({ config: createRecordConfigComponent(values), logs: quiet([]), fetcher: { fetch: async (url, options) => { request = { url, timeout: options.timeout }; return new Response(null) } } })
      await analytics.sendEvent('FIXTURE', {})
      configuration.push({ omitted, configured: true, request })
    } catch (error) { configuration.push({ omitted, error: error.message }) }
  }
  const timeouts = []
  for (const raw of [null, '0', '-1', '2.75ms', '0.25', 'Infinity', '-Infinity', '2147483647', '2147483648', '1e300', 'NaN', '']) {
    const logs = []
    let timeout
    try {
      const config = createRecordConfigComponent({ ...base, ...(raw === null ? {} : { ANALYTICS_REQUEST_TIMEOUT: raw }) })
      const configured = await config.getNumber('ANALYTICS_REQUEST_TIMEOUT')
      const analytics = await createAnalyticsComponent({ config, logs: quiet(logs),
        fetcher: { fetch: async (_url, options) => { timeout = options.timeout; return new Response(null) } } })
      await analytics.sendEvent('FIXTURE', {})
      const timer = setTimeout(() => {}, timeout)
      const timer_bucket_ms = Math.trunc(timer._idleTimeout)
      clearTimeout(timer)
      timeouts.push({ raw, configured_number: configured === undefined ? null : encoded(configured), timeout: encoded(timeout), timer_bucket_ms, logs })
    } catch (error) { timeouts.push({ raw, error: error.message, logs }) }
  }
  const transport = []
  const realFetch = global.fetch
  try {
    for (const scenario of ['success', '503', '429', 'network-failure', 'cancel-failure', 'body-absent', 'timeout']) {
      const calls = [], logs = [], order = []
      global.fetch = async (url, options) => {
        calls.push({ url, method: options.method, headers: options.headers, payload: JSON.parse(options.body), redirect: options.redirect ?? 'follow' })
        order.push('fetch')
        if (scenario === 'network-failure') throw Error('synthetic network failure')
        if (scenario === 'timeout') return new Promise((_resolve, reject) => options.signal.addEventListener('abort', () => { order.push('abort'); reject(Error('synthetic abort')) }, { once: true }))
        const status = scenario === '503' ? 503 : scenario === '429' ? 429 : 200
        return new Response(scenario === 'body-absent' ? null : new ReadableStream({ cancel() { order.push('cancel'); if (scenario === 'cancel-failure') throw Error('synthetic cancel failure') } }), { status })
      }
      const analytics = await createAnalyticsComponent({ config: createRecordConfigComponent({ ...base, ...(scenario === 'timeout' ? { ANALYTICS_REQUEST_TIMEOUT: '5' } : {}) }), logs: quiet(logs), fetcher: createFetchComponent({ defaultFetcherOptions: { attempts: 5, timeout: 30000 } }) })
      const result = await analytics.sendEvent('START_CALL', { call_id: 'fixture-call', env: 'overridden', user_id: 'fixture-caller' })
      order.push('resolved')
      transport.push({ scenario, calls, order, logs, result: result === undefined ? 'undefined' : result })
    }
    let release
    const pending = new Promise(resolve => { release = resolve })
    const order = []
    const analytics = await createAnalyticsComponent({ config: createRecordConfigComponent(base), logs: quiet([]), fetcher: { fetch: async () => { order.push('fetch'); await pending; return new Response(new ReadableStream({ cancel() { order.push('cancel') } })) } } })
    const result = analytics.fireEvent('ACCEPT_CALL', { call_id: 'fixture-call' })
    order.push('caller-returned')
    const before_release = [...order]
    release()
    await new Promise(setImmediate)
    transport.push({ scenario: 'fire-and-forget', result: result === undefined ? 'undefined' : result, before_release, order })
  } finally { global.fetch = realFetch }
  const redirects = []
  const server = http.createServer(async (request, response) => {
    let body = ''
    for await (const chunk of request) body += chunk
    const [kind, code, remaining] = request.url.slice(1).split('/')
    const trace = redirects.at(-1).requests
    trace.push({ path: request.url, method: request.method, content_type: request.headers['content-type'] ?? null, content_length: request.headers['content-length'] ?? null, token: request.headers['x-token'] ?? null, body })
    if (kind === 'jump' && +remaining > 0) { response.writeHead(+code, { location: `/jump/${code}/${+remaining - 1}` }); response.end() }
    else { response.writeHead(200); response.end('unused response body') }
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  try {
    for (const [status, hops] of [[301, 3], [302, 3], [303, 3], [307, 3], [308, 3], [307, 20], [307, 21]]) {
      const logs = []
      const entry = { status, hops, requests: [], logs }
      redirects.push(entry)
      const analytics = await createAnalyticsComponent({ config: createRecordConfigComponent({ ...base, ANALYTICS_API_URL: `http://127.0.0.1:${server.address().port}/jump/${status}/${hops}` }), logs: quiet(logs), fetcher: createFetchComponent() })
      await analytics.sendEvent('REDIRECT_FIXTURE', { call_id: 'fixture-call' })
    }
  } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)) }
  fs.writeFileSync(process.argv[3], JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision }, packages: { analytics: req('@dcl/analytics-component/package.json').version, fetch: req('@dcl/fetch-component/package.json').version }, scope: 'Installed upstream analytics SDK, config provider, fetch wrapper and Node timers; controlled fetch/Response streams plus real loopback redirect server. No real analytics endpoint or mutation hooks in this capture.', configuration, timeouts, transport, redirects }, null, 2) + '\n')
  process.stdout.write(`Captured analytics configuration${configuration.length}, timeouts${timeouts.length}, transport${transport.length}, redirects${redirects.length}\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
