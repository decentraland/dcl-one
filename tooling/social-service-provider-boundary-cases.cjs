#!/usr/bin/env node
// Execute pinned provider factories/SDKs with controlled fetch streams and timers.
// No provider endpoint is contacted. The stream probe covers SDK timer scope;
// A separate child exercises native Node body inactivity over local HTTP.
const fs = require('node:fs')
const crypto = require('node:crypto')
const http = require('node:http')
const { execFileSync } = require('node:child_process')
const { req, load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const { createAIComplianceComponent } = load('adapters/ai-compliance.ts')
const compliant = { isCompliant: true, issues: { name: [], description: [], image: [] }, confidence: 0.9 }
const config = key => createRecordConfigComponent({ ENV: 'prd', ...(key === undefined ? {} : { OPEN_AI_API_KEY: key }) })
const dependencies = key => ({ config: config(key), logs, featureFlags: { isEnabled: () => true }, metrics: { increment() {}, observe() {} }, redis: { get: async () => null, put: async () => {} } })
const realFetch = global.fetch, realSetTimeout = global.setTimeout, realClearTimeout = global.clearTimeout
const response = padding => JSON.stringify({ object: 'response', padding: 'x'.repeat(padding), output: [{ type: 'message', content: [{ type: 'output_text', text: JSON.stringify(compliant) }] }] })
const turn = () => new Promise(setImmediate)
async function main() {
  const native_socket_body = JSON.parse(execFileSync(process.execPath, [__filename, upstream, process.argv[3], 'native'], { encoding: 'utf8', timeout: 10000 }))
  const native_analytics = Object.fromEntries(['headers', 'redirect-chain', 'connect'].map(kind => [kind, JSON.parse(execFileSync(process.execPath, [__filename, upstream, process.argv[3], 'native-analytics', kind], { encoding: 'utf8', timeout: 10000 }))]))
  const bootstrap = []
  for (const key of [undefined, '', 'fixture-key']) {
    try {
      await createAIComplianceComponent(dependencies(key))
      bootstrap.push({ key: key === undefined ? 'absent' : key === '' ? 'empty' : 'fixture', configured: true })
    } catch (error) { bootstrap.push({ key: 'absent', configured: false, error: error.message }) }
  }
  const { createAnalyticsComponent } = req('@dcl/analytics-component')
  const { createSlackComponent } = req('@dcl/slack-component')
  const required = ['ANALYTICS_CONTEXT', 'ANALYTICS_API_URL', 'ANALYTICS_API_TOKEN', 'ENV', 'OPEN_AI_API_KEY', 'SLACK_BOT_TOKEN']
  const initialization = fs.readFileSync(upstream + '/src/components.ts', 'utf8')
  const provider_initialization_order = ['const analytics = await createAnalyticsComponent', 'const aiCompliance = await createAIComplianceComponent', "const slackToken = await config.requireString('SLACK_BOT_TOKEN')"].map(text => ({ statement: text, offset: initialization.indexOf(text) }))
  if (provider_initialization_order.some(entry => entry.offset < 0) || provider_initialization_order.some((entry, index) => index && entry.offset <= provider_initialization_order[index - 1].offset)) throw Error('Source provider initialization order changed')
  const active_bootstrap = []
  for (const omitted of [null, ...required, 'all-empty', 'all-missing']) {
    const values = Object.fromEntries(required.map(key => [key, omitted === 'all-empty' ? '' : 'fixture']))
    if (omitted === 'all-missing') for (const key of required) delete values[key]
    else if (omitted) delete values[omitted]
    const configured = createRecordConfigComponent(values)
    try {
      // Execute the actual factories in their source initialization order; no
      // event is sent and unrelated initComponents dependencies are excluded.
      await createAnalyticsComponent({ config: configured, logs, fetcher: { fetch() { throw Error('bootstrap must not fetch') } } })
      await createAIComplianceComponent({ ...dependencies('fixture'), config: configured })
      createSlackComponent({ logs }, { token: await configured.requireString('SLACK_BOT_TOKEN') })
      active_bootstrap.push({ omitted, configured: true })
    } catch (error) { active_bootstrap.push({ omitted, configured: false, error: error.message }) }
  }
  let requests = 0, signal, abortTimer, stream, completed = false
  const timers = []
  global.setTimeout = (callback, delay, ...args) => {
    if (delay !== 600000 && delay !== 60000) return realSetTimeout(callback, delay, ...args)
    const timer = { delay, cleared: false, fire() { if (!this.cleared) callback(...args) } }
    timers.push(timer)
    abortTimer = timer
    return timer
  }
  global.clearTimeout = timer => {
    if (timers.includes(timer)) timer.cleared = true
    else realClearTimeout(timer)
  }
  global.fetch = async (_url, init) => {
    requests++
    signal = init.signal
    return new Response(new ReadableStream({ start(controller) { stream = controller } }), { headers: { 'content-type': 'application/json' } })
  }
  let sdk_body_phase
  try {
    const component = await createAIComplianceComponent(dependencies('fixture-key'))
    const pending = component.validateCommunityContent({ name: 'Fixture' }).then(value => { completed = true; return value })
    await turn()
    const before = { completed, signal_aborted: signal.aborted, timer_ms: abortTimer.delay, timer_cleared: abortTimer.cleared }
    // Cross the SDK's nominal deadline after headers while body is pending.
    abortTimer.fire()
    await turn()
    const after = { completed, signal_aborted: signal.aborted }
    stream.enqueue(new TextEncoder().encode(response(0)))
    stream.close()
    const result = await pending
    sdk_body_phase = { before, after, result, requests }
    requests = 0
    const large = response((4 << 20) + 1)
    global.fetch = async () => { requests++; return new Response(large, { headers: { 'content-type': 'application/json' } }) }
    const large_response = { bytes: Buffer.byteLength(large), padding_bytes: (4 << 20) + 1, result: await component.validateCommunityContent({ name: 'Large fixture' }), requests }
    const ai_metrics = []
    for (const scenario of ['live-compliant', 'live-non-compliant', 'cached-compliant', 'cached-non-compliant', 'provider-invalid', 'cache-read-failure', 'cache-write-failure']) {
      const events = []
      const result = { ...compliant, isCompliant: !scenario.endsWith('non-compliant') }
      global.fetch = async () => new Response(JSON.stringify({ object: 'response', output: [{ type: 'message', content: [{ type: 'output_text', text: JSON.stringify(scenario === 'provider-invalid' ? {} : result) }] }] }), { headers: { 'content-type': 'application/json' } })
      const component = await createAIComplianceComponent({ ...dependencies('fixture-key'),
        metrics: { increment: (name, labels) => events.push({ operation: 'increment', name, labels }), observe: (name, labels, value) => { if (!Number.isFinite(value) || value < 0) throw Error('Invalid duration'); events.push({ operation: 'observe', name, labels }) } },
        redis: { get: async () => { if (scenario === 'cache-read-failure') throw Error('fixture cache read failure'); return scenario.startsWith('cached-') ? result : null }, put: async () => { if (scenario === 'cache-write-failure') throw Error('fixture cache write failure') } }
      })
      let error
      try { await component.validateCommunityContent({ name: 'Metrics fixture' }) } catch (failure) { error = failure.message }
      ai_metrics.push({ scenario, events, ...(error ? { error } : {}) })
    }
    const analyticsLogs = []
    const { createFetchComponent } = req('@dcl/fetch-component')
    let analyticsCompleted = false
    global.fetch = async (_url, init) => new Promise((_resolve, reject) => {
      init.signal.addEventListener('abort', () => reject(Error('fixture abort')), { once: true })
    })
    const analyticsComponent = await createAnalyticsComponent({
      config: createRecordConfigComponent({ ANALYTICS_CONTEXT: 'fixture', ANALYTICS_API_URL: 'http://fixture.localhost/events', ANALYTICS_API_TOKEN: 'fixture-token', ENV: 'fixture', ANALYTICS_REQUEST_TIMEOUT: '60000' }),
      logs: { getLogger: () => ({ warn() {}, error: (message, data) => analyticsLogs.push({ message, data }) }) },
      fetcher: createFetchComponent()
    })
    const analyticsPending = analyticsComponent.sendEvent('SLOW_HEADERS_FIXTURE', {}).then(() => { analyticsCompleted = true })
    await turn()
    const analyticsTimer = abortTimer
    const analytics = { configured_header_timer_ms: analyticsTimer.delay, before_timer: { completed: analyticsCompleted } }
    analyticsTimer.fire()
    await analyticsPending
    analytics.after_timer = { completed: analyticsCompleted, logs: analyticsLogs }
    const { WebClient } = req('@slack/web-api')
    const client = new WebClient('fixture-token')
    const slack = { axios_timeout_ms: client.axios.defaults.timeout, max_redirects: client.axios.defaults.maxRedirects }
    let emptyRequests = 0, emptyAuthorization = null
    const originalMakeRequest = WebClient.prototype.makeRequest
    WebClient.prototype.makeRequest = function (...args) {
      this.axios.defaults.adapter = async request => {
        emptyRequests++
        emptyAuthorization = request.headers.Authorization ?? null
        return { status: 200, headers: {}, data: { ok: true }, config: request, request: { path: '/api/chat.postMessage' } }
      }
      return originalMakeRequest.apply(this, args)
    }
    try {
      await createSlackComponent({ logs }, { token: '' }).sendMessage({ channel: 'fixture', text: 'hello' })
      slack.empty_token = { requests: emptyRequests, authorization: emptyAuthorization, result: 'ok' }
    } finally { WebClient.prototype.makeRequest = originalMakeRequest }

    const sources = ['src/components.ts', 'src/adapters/ai-compliance.ts', 'node_modules/@dcl/slack-component/dist/src/component.js', 'node_modules/openai/client.js', 'node_modules/openai/internal/parse.js', 'node_modules/@slack/web-api/dist/WebClient.js', 'node_modules/@dcl/analytics-component/dist/src/component.js', 'node_modules/@dcl/fetch-component/dist/src/fetcher.js']
    fs.writeFileSync(process.argv[3], JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision, sources: sources.map(path => ({ path, sha256: crypto.createHash('sha256').update(fs.readFileSync(upstream + '/' + path)).digest('hex') })) }, runtime: { node: process.version, openai: JSON.parse(fs.readFileSync(upstream + '/node_modules/openai/package.json')).version, slack: req('@slack/web-api/package.json').version, analytics: req('@dcl/analytics-component/package.json').version, fetch: req('@dcl/fetch-component/package.json').version }, scope: 'Actual source provider factory sequence (analytics, AI, Slack) and installed OpenAI SDK; controlled fetch ReadableStream and SDK 600000ms timer. Captures missing/empty bootstrap key, timer clearance at headers, pending body across nominal SDK deadline, and accepted response above 4MiB. Slack installed SDK axios defaults and the analytics configured 60000ms header-abort timer are observed. Native Node body inactivity, headers and stalled TLS connect deadlines are exercised over loopback with the native 499ms fast timer tick controlled; analytics redirect hops execute independently within the SDK timer. Seven actual AI factory metric boundaries use controlled provider responses and cache mocks. Does not establish deployed credentials, full initComponents startup, live provider success, or delayed-DNS/combined connection-and-header timing equivalence.', native_socket_body, native_analytics, bootstrap, provider_initialization_order, active_bootstrap, sdk_body_phase, large_response, slack, analytics, ai_metrics }, null, 2) + '\n')
  } finally { global.fetch = realFetch; global.setTimeout = realSetTimeout; global.clearTimeout = realClearTimeout }
}
async function nativeMain() {
  let response, requests = 0, ticker, completed = false, sdkTimer
  const timerHandles = new Set()
  const server = http.createServer((request, reply) => {
    requests++
    request.resume()
    response = reply
    reply.writeHead(200, { 'content-type': 'application/json' })
    reply.write('{"padding":"')
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  process.env.OPENAI_BASE_URL = `http://127.0.0.1:${server.address().port}/v1`
  let headersArrived
  const headers = new Promise(resolve => { headersArrived = resolve })
  global.fetch = async (...args) => { const value = await realFetch(...args); headersArrived(); return value }
  global.setTimeout = (callback, delay, ...args) => {
    if ((delay === 499 && callback.name === 'onTick') || delay === 600000) {
      const timer = { delay, cleared: false, refresh() { return this }, unref() { return this }, fire() { if (!this.cleared) callback(...args) } }
      timerHandles.add(timer)
      if (delay === 499) ticker = timer
      else sdkTimer = timer
      return timer
    }
    return realSetTimeout(callback, delay, ...args)
  }
  global.clearTimeout = timer => { if (timerHandles.has(timer)) timer.cleared = true; else realClearTimeout(timer) }
  try {
    const component = await createAIComplianceComponent(dependencies('fixture-key'))
    const pending = component.validateCommunityContent({ name: 'Native body fixture' }).then(value => ({ result: value }), error => ({ error: error.message })).then(value => { completed = true; return value })
    await headers
    await turn()
    if (!ticker || !sdkTimer.cleared) throw Error('Expected native fast timer and cleared SDK header timer')
    // Initialize the native fast-timer state, then allow 239.5 seconds of
    // inactivity. A body byte resets that timer before its 300-second limit.
    ticker.fire()
    for (let n = 0; n < 480; n++) ticker.fire()
    await turn()
    const before_progress = { completed, elapsed_tick_ms: 481 * 499 }
    response.write('x')
    // A local I/O turn delivers the byte and refreshes the real parser timer.
    await new Promise(resolve => realSetTimeout(resolve, 20))
    for (let n = 0; n < 480; n++) ticker.fire()
    await turn()
    const after_progress = { completed, elapsed_tick_ms: 961 * 499 }
    response.write('x')
    await new Promise(resolve => realSetTimeout(resolve, 20))
    for (let n = 0; n < 480; n++) ticker.fire()
    await turn()
    const after_sdk_deadline = { completed, elapsed_tick_ms: 1441 * 499 }
    // No further body bytes: cross 300 seconds since the refresh.
    for (let n = 0; n < 123; n++) ticker.fire()
    let watchdog
    const result = await Promise.race([pending, new Promise((_resolve, reject) => { watchdog = realSetTimeout(() => reject(Error('native body inactivity probe stalled')), 2000) })]).finally(() => realClearTimeout(watchdog))
    process.stdout.write(JSON.stringify({ transport: 'Actual source AI component and OpenAI SDK over native Node fetch to a loopback HTTP server; native 499ms fast timer ticks are invoked under controlled time', node: process.version, undici: process.versions.undici, native_bundle_sha256: crypto.createHash('sha256').update(process.binding('natives')['internal/deps/undici/undici']).digest('hex'), tick_ms: 499, before_progress, after_progress, after_sdk_deadline, at_inactivity_timeout: { elapsed_tick_ms: 1564 * 499, ...result }, sdk_header_timer_cleared: sdkTimer.cleared, requests }))
  } finally {
    global.fetch = realFetch; global.setTimeout = realSetTimeout; global.clearTimeout = realClearTimeout
    server.closeAllConnections()
    await new Promise(resolve => server.close(resolve))
  }
}
async function nativeAnalyticsMain(kind) {
  const net = require('node:net')
  const sockets = new Set(), requests = [], logs = []
  let response, ticker, sdkTimer, completed = false, entered
  const entry = () => new Promise(resolve => { entered = resolve })
  let arrived = entry()
  const server = kind === 'connect' ? net.createServer(socket => { sockets.add(socket); entered() }) : http.createServer((request, reply) => {
    requests.push(request.url)
    request.resume()
    response = reply
    entered()
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  const handles = new Set()
  global.setTimeout = (callback, delay, ...args) => {
    if ((delay === 499 && callback.name === 'onTick') || delay === 600000) {
      const timer = { cleared: false, refresh() { return this }, unref() { return this }, fire() { if (!this.cleared) callback(...args) } }
      handles.add(timer)
      if (delay === 499) ticker = timer; else sdkTimer = timer
      return timer
    }
    return realSetTimeout(callback, delay, ...args)
  }
  global.clearTimeout = timer => { if (handles.has(timer)) timer.cleared = true; else realClearTimeout(timer) }
  try {
    const { createAnalyticsComponent } = req('@dcl/analytics-component')
    const { createFetchComponent } = req('@dcl/fetch-component')
    const analytics = await createAnalyticsComponent({ config: createRecordConfigComponent({ ANALYTICS_CONTEXT: 'fixture', ANALYTICS_API_URL: `${kind === 'connect' ? 'https' : 'http'}://127.0.0.1:${server.address().port}/start`, ANALYTICS_API_TOKEN: 'fixture-token', ENV: 'fixture', ANALYTICS_REQUEST_TIMEOUT: '600000' }), logs: { getLogger: () => ({ warn() {}, error: (message, data) => logs.push({ message, data }) }) }, fetcher: createFetchComponent() })
    const pending = analytics.sendEvent('NATIVE_HEADERS_FIXTURE', {}).then(() => { completed = true })
    await arrived
    await turn()
    if (!ticker || !sdkTimer) throw Error('Expected native fast timer and analytics SDK timer')
    const ticks = count => { for (let n = 0; n < count; n++) ticker.fire() }
    let before_deadline, after_first_hop, after_second_hop
    if (kind === 'connect') {
      ticks(20)
      await turn()
      before_deadline = { completed, elapsed_tick_ms: 20 * 499 }
      ticks(2)
    } else if (kind === 'headers') {
      ticks(601)
      await turn()
      before_deadline = { completed, elapsed_tick_ms: 601 * 499 }
      ticks(2)
    } else {
      ticks(481)
      await turn()
      after_first_hop = { completed, elapsed_tick_ms: 481 * 499 }
      arrived = entry()
      response.writeHead(307, { location: '/end' }); response.end()
      await arrived
      await turn()
      ticks(480)
      await turn()
      after_second_hop = { completed, elapsed_tick_ms: 961 * 499 }
      response.writeHead(200); response.end('unused body')
    }
    let watchdog
    await Promise.race([pending, new Promise((_resolve, reject) => { watchdog = realSetTimeout(() => reject(Error('native analytics probe stalled')), 2000) })]).finally(() => realClearTimeout(watchdog))
    process.stdout.write(JSON.stringify({ node: process.version, undici: process.versions.undici, sdk_timer_ms: 600000, native_budget_ms: kind === 'connect' ? 10000 : 300000, tick_ms: 499, ...(before_deadline ? { before_deadline } : {}), ...(after_first_hop ? { after_first_hop, after_second_hop } : {}), completed, logs, requests, stalled_tls_connections: sockets.size }))
  } finally {
    global.setTimeout = realSetTimeout; global.clearTimeout = realClearTimeout
    for (const socket of sockets) socket.destroy()
    server.closeAllConnections?.()
    await new Promise(resolve => server.close(resolve))
  }
}
(process.argv[4] === 'native' ? nativeMain() : process.argv[4] === 'native-analytics' ? nativeAnalyticsMain(process.argv[5]) : main()).catch(error => { console.error(error); process.exitCode = 1 })
