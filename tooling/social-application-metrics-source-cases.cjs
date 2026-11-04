#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json
// Actual pinned source metrics component, RPC wrapper and pool; injected service results and wall clock.
const { requireNode26, sourceCapture, sha256 } = require('./social-capture-common.cjs')
requireNode26()
const fs = require('node:fs'), path = require('node:path')
const output = process.argv[3]
if (!process.argv[2] || !output) throw Error('Usage: node tooling/social-application-metrics-source-cases.cjs <source-root> <output.json>')
const { upstream: root, revision, req } = sourceCapture(process.argv[2])
const { metricDeclarations } = req(path.join(root, 'src/metrics.ts'))
const { createMetricsComponent } = req('@dcl/metrics')
const { createRpcServerMetricsWrapper, ServiceType } = req(path.join(root, 'src/adapters/rpc-server/metrics-wrapper.ts'))
const { createWsPoolComponent } = req(path.join(root, 'src/logic/ws-pool/component.ts'))
const proto = req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const config = { getString: async key => ({ WKC_METRICS_COLLECT_DEFAULT: 'false', RPC_TRACING_ENABLED: 'false' })[key], getNumber: async key => key === 'WS_MAX_CONCURRENT_CONNECTIONS' ? 2 : undefined }
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
async function main() {
 const metrics = await createMetricsComponent(metricDeclarations, { config })
 const initial = await metrics.registry.metrics()
 const schemaMetrics = await createMetricsComponent(metricDeclarations, { config })
 for (const [name, declaration] of Object.entries(metricDeclarations)) if (declaration.type === 'histogram') schemaMetrics.observe(name, Object.fromEntries((declaration.labelNames || []).map(label => [label, 'fixture'])), 1)
 const histograms = (await schemaMetrics.registry.getMetricsAsJSON()).filter(item => item.type === 'histogram').map(item => ({ name: item.name, buckets: item.values.filter(value => value.metricName.endsWith('_bucket')).map(value => value.labels.le) }))
 const protobuf = req('protobufjs')
 const schema = new protobuf.Root()
 const protoRoot = path.join(root, 'node_modules/@dcl/protocol/proto')
 schema.resolvePath = (_, target) => path.join(protoRoot, target)
 schema.loadSync(['decentraland/social_service/v2/social_service_v2.proto', 'decentraland/common/colors.proto', 'decentraland/social_service/errors.proto']).resolveAll()
 const jsonCases = []
 function seed(type, variant, depth = 0) {
  if (depth > 8) return {}
  const result = {}
  for (const field of type.fieldsArray) {
   if (field.partOf && field.name !== (variant?.[field.partOf.name] || field.partOf.oneof[0])) continue
   let value = field.resolvedType instanceof protobuf.Type ? seed(field.resolvedType, undefined, depth + 1) : field.resolvedType instanceof protobuf.Enum ? Object.values(field.resolvedType.values)[0] : field.type === 'string' ? 'fixture😀' : field.type === 'bool' ? true : ['float', 'double'].includes(field.type) ? 0.1 : 123
   result[field.name] = field.repeated ? [value] : value
  }
  return result
 }
 function visit(namespace) {
  for (const type of namespace.nestedArray || []) {
   if (type instanceof protobuf.Type) {
    const pkg = type.fullName.startsWith('.decentraland.social_service.v2.') ? '.decentraland.social_service.v2.' : type.fullName.startsWith('.decentraland.social_service.') ? '.decentraland.social_service.' : '.decentraland.common.'
    const exportName = type.fullName.slice(pkg.length).replaceAll('.', '_')
    const module = pkg.includes('.v2.') ? proto : pkg.includes('social_service') ? req('@dcl/protocol/out-js/decentraland/social_service/errors.gen') : req('@dcl/protocol/out-js/decentraland/common/colors.gen')
    const codec = module[exportName]
    if (codec) {
     const inputs = [['default', {}], ['seeded', seed(type)]]
     for (const oneof of type.oneofsArray) for (const field of oneof.oneof.slice(1)) inputs.push([oneof.name + '.' + field, seed(type, { [oneof.name]: field })])
     for (const [variant, input] of inputs) {
      const bytes = type.encode(type.fromObject(input)).finish(), decoded = codec.decode(bytes)
      jsonCases.push({ id: exportName + '.' + variant, type: exportName, hex: Buffer.from(bytes).toString('hex'), decoded, jsonBytes: Buffer.byteLength(JSON.stringify(decoded)) })
     }
    }
   }
   visit(type)
  }
 }
 visit(schema)
 const observations = []
 const observer = { ...metrics, increment(name, labels = {}, value = 1) { observations.push({ name, labels, value }); metrics.increment(name, labels, value) }, observe(name, labels = {}, value) { observations.push({ name, labels, value }); metrics.observe(name, labels, value) } }
 const NativeDate = Date; let now = 1000
 global.Date = class extends NativeDate { constructor(...args) { super(...(args.length ? args : [now])) } static now() { return now } }
 const pool = await createWsPoolComponent({ metrics: observer, logs, config })
 const sockets = ['rpc', 'stream'].map((protocol, index) => ({ getUserData: () => ({ wsConnectionId: String(index), protocol, connectionStartTime: 1000 }) }))
 for (const socket of sockets) pool.registerConnection(socket)
 try { pool.registerConnection({ getUserData: () => ({ wsConnectionId: 'full', protocol: 'rpc' }) }) } catch (_) {}
 now = 6000
 for (const socket of sockets) pool.unregisterConnection(socket.getUserData())
 const poolObservations = observations.splice(0)
 const wrapper = await createRpcServerMetricsWrapper({ components: { metrics: observer, logs, config } })
 const { onRequestStart, onRequestEnd } = req('@dcl/uws-http-server/dist/src/metrics.js')
 const { createGenericLogComponent, incrementMetric } = req('@well-known-components/logger/dist/helpers.js')
 const measuredLogs = await createGenericLogComponent({ config, metrics: observer }, (components, level, logger) => incrementMetric(components, logger, level))
 const { createPrivacyHandler } = req(path.join(root, 'src/controllers/handlers/uws/privacy-handler.ts'))
 const { createStatusHandler } = req(path.join(root, 'src/controllers/handlers/uws/status-handler.ts'))
 const privacy = await createPrivacyHandler({ friendsDb: {}, logs: measuredLogs })
 const status = await createStatusHandler({ config })
 for (const [method, handler, run] of [
  ['get', status.path, () => status.f()],
  ['get', privacy.path, () => privacy.f({}, { getParameter: () => 'invalid' })],
  ['post', '/health/live', async () => ({ status: 200 })],
  ['head', '', async () => ({ status: 404 })]
 ]) {
  const { labels, end } = onRequestStart(observer, method, handler)
  const result = await run(); onRequestEnd(observer, labels, result.status || 200, end)
 }
 const httpObservations = observations.splice(0)
 const unary = []
 for (const item of [
  { procedure: 'getFriends', requestType: 'GetFriendsPayload', responseType: 'PaginatedFriendsProfilesResponse', request: { pagination: { limit: 10, offset: 0 } }, response: { friends: [], paginationData: { total: 0 } } },
  { procedure: 'blockUser', requestType: 'BlockUserPayload', responseType: 'BlockUserResponse', request: {}, response: { response: { $case: 'invalidRequest', invalidRequest: { message: 'User is missing' } } } },
  { procedure: 'getSocialSettings', request: {}, responseType: 'GetSocialSettingsResponse', response: { response: { $case: 'ok', ok: { settings: { privateMessagesPrivacy: 0, blockedUsersMessagesVisibility: 0, showSituationReactions: 0 } } } } },
  { procedure: 'getBlockingStatus', request: {}, responseType: 'GetBlockingStatusResponse', response: { blockedUsers: [], blockedByUsers: [] } },
  { procedure: 'getFriends', requestType: 'GetFriendsPayload', request: {}, throws: true }
 ]) {
  const requestBytes = item.requestType ? proto[item.requestType].encode(item.request).finish() : Buffer.alloc(0)
  const params = item.requestType ? proto[item.requestType].decode(requestBytes) : {}
  const responseBytes = item.responseType ? proto[item.responseType].encode(item.response).finish() : Buffer.alloc(0)
  const result = item.responseType ? proto[item.responseType].decode(responseBytes) : null
  const call = wrapper.withMetrics({ [item.procedure]: { type: ServiceType.CALL, creator: async () => { now += 50; if (item.throws) throw Error('fixture'); return result } } })[item.procedure]
  try { await call(params, {}) } catch (_) {}
  unary.push({ ...item, params, result, requestHex: Buffer.from(requestBytes).toString('hex'), responseHex: Buffer.from(responseBytes).toString('hex'), observations: observations.splice(0) })
 }
 const stream = wrapper.withMetrics({ subscribeToBlockUpdates: { type: ServiceType.STREAM, event: 'blockUpdate', creator: async function* () { yield { address: 'fixture', isBlocked: true }; now += 200 } } }).subscribeToBlockUpdates({}, {})
 await stream.next(); now += 300; await stream.next()
 const streamObservations = observations.splice(0)
 global.Date = NativeDate
 const { createSubscribersContext } = req(path.join(root, 'src/adapters/rpc-server/subscribers-context.ts'))
 const { createUpdateHandlerComponent } = req(path.join(root, 'src/logic/updates.ts'))
 const intervals = [], nativeSetInterval = global.setInterval, nativeClearInterval = global.clearInterval
 global.setInterval = (callback, milliseconds) => { const interval = { callback, milliseconds }; intervals.push(interval); return interval }
 global.clearInterval = () => {}
 let subscriberObservations, subscriptionObservations
 try {
  const subscribers = createSubscribersContext({ logs, metrics: observer, config }, { getAuthenticatedAddresses: () => ['fixture', 'other'], getConnectionIds: () => ['one', 'two', 'other'] })
  await subscribers.start()
  if (observations.length) throw Error('Subscriber reporting must not run immediately')
  subscribers.addConnection('fixture', 'one'); subscribers.addConnection('fixture', 'two'); subscribers.addConnection('other', 'other')
  const updates = createUpdateHandlerComponent({ logs, metrics: observer, subscribersContext: subscribers })
  let release
  const initial = new Promise(resolve => { release = resolve })
  const params = { rpcContext: { address: 'fixture', wsConnectionId: 'one', subscribersContext: subscribers }, eventName: 'blockUpdate', getAddressFromUpdate: () => 'fixture', shouldHandleUpdate: () => true, parser: value => value, getInitialUpdates: () => initial, buildStreamClosedUpdate: streamClosed => ({ streamClosed }) }
  const generator = updates.handleSubscriptionUpdates(params), pending = generator.next()
  const duplicate = updates.handleSubscriptionUpdates(params)
  await duplicate.next(); await duplicate.next()
  const emitter = subscribers.getSubscriber('fixture')
  for (let index = 0; index < 1100; index++) emitter.emit('blockUpdate', { index })
  subscriptionObservations = observations.splice(0)
  intervals.find(interval => interval.milliseconds === 30000).callback()
  subscriberObservations = observations.splice(0)
  release([])
  const first = await pending
  if (first.value.index !== 100) throw Error('Source overflow fixture changed')
  await generator.return()
  subscribers.removeConnection('fixture', 'one'); subscribers.removeConnection('fixture', 'two'); subscribers.removeConnection('other', 'other')
  intervals.find(interval => interval.milliseconds === 30000).callback()
  subscriberObservations.push(...observations.splice(0))
  await subscribers.stop()
 } finally { global.setInterval = nativeSetInterval; global.clearInterval = nativeClearInterval }
 const { createServerComponent } = req('@dcl/http-server/dist/server.js')
 const { getUnderlyingServer } = req('@dcl/http-server/dist/injectors.js')
 const { instrumentHttpServerWithPromClientRegistry } = req('@dcl/http-server/dist/metrics.js')
 const { setupHttpRoutes } = req(path.join(root, 'src/controllers/routes/http.routes.ts'))
 const { createSchemaValidatorComponent } = req('@dcl/schema-validator-component')
 const apiMetrics = await createMetricsComponent(metricDeclarations, { config })
 const apiObservations = []
 const apiObserver = { ...apiMetrics, increment(name, labels = {}, value = 1) { apiObservations.push({ name, labels, value }); apiMetrics.increment(name, labels, value) }, observe(name, labels = {}, value) { apiObservations.push({ name, labels, value }); apiMetrics.observe(name, labels, value) } }
 const context = { components: { logs, config: { getString: async key => key === 'API_ADMIN_TOKEN' ? 'fixture-admin' : undefined }, schemaValidator: createSchemaValidatorComponent({ ensureJsonContentType: false }), fetcher: { fetch() { throw Error('no outbound fixture requests') } } } }
 const apiRouter = await setupHttpRoutes(context)
 const httpRoutes = apiRouter.stack.filter(layer => layer.methods.length).map(layer => ({ path: layer.path, methods: layer.methods }))
 const server = await createServerComponent({ config: { requireNumber: async () => 0, requireString: async () => '127.0.0.1' }, logs }, { cors: { methods: ['GET', 'HEAD', 'OPTIONS', 'DELETE', 'POST', 'PUT', 'PATCH'], maxAge: 86400 }, maxBodySize: 64 })
 server.setContext(context)
 await instrumentHttpServerWithPromClientRegistry({ server, metrics: apiObserver, config, registry: apiMetrics.registry })
 server.use(apiRouter.middleware()); server.use(apiRouter.allowedMethods())
 await server.start()
 const apiCases = []
 let listener
 try {
  listener = await getUnderlyingServer(server)
  const port = listener.address().port
  for (const input of [
   { method: 'GET', path: '/unknown' },
   { method: 'OPTIONS', path: '/unknown' },
   { method: 'GET', path: '/v1/social-settings' },
   { method: 'DELETE', path: '/v1/friends/0x1111111111111111111111111111111111111111' },
   { method: 'DELETE', path: '/v1/communities/fixture/members/0x1111111111111111111111111111111111111111' },
   { method: 'GET', path: '/v1/communities/0x1111111111111111111111111111111111111111/managed' },
   { method: 'POST', path: '/v1/social-settings' },
   { method: 'POST', path: '/v1/friendship-requests', body: '{}' },
   { method: 'POST', path: '/unknown', body: 'x'.repeat(65) }
  ]) {
   const status = await new Promise((resolve, reject) => {
    const request = require('node:http').request({ hostname: '127.0.0.1', port, path: input.path, method: input.method, headers: input.body ? { 'content-length': Buffer.byteLength(input.body), 'content-type': 'application/json' } : {} }, response => { response.resume(); response.on('end', () => resolve(response.statusCode)); response.on('error', reject); response.on('aborted', () => reject(Error('Metrics capture response aborted'))) })
    request.on('error', reject); request.setTimeout(5000, () => request.destroy(Error('Metrics capture request timed out'))); request.end(input.body)
   })
   apiCases.push({ ...input, status, observations: apiObservations.splice(0) })
  }
 } finally { listener?.closeAllConnections(); await server.stop() }
 fs.writeFileSync(output, JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision, yarn_lock_sha256: sha256(root, 'yarn.lock') }, node: process.version, protocol: req('@dcl/protocol/package.json').version, promClient: req('prom-client/package.json').version, scope: 'Actual source registry, pool, RPC metrics wrapper, subscription components and Social API loopback HTTP listener/router; deterministic injected dependencies and a fake wall clock for pool/RPC durations. No providers, database, kernel-buffer or deployment equivalence.', initial, histograms, jsonCases, poolObservations, subscriptionObservations, subscriberObservations, httpObservations, httpRoutes, apiCases, unary, streamObservations }, null, 2) + '\n')
}
main().catch(error => { console.error(error); process.exitCode = 1 })
