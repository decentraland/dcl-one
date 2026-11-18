// Generates fixtures/request-vectors.json: signed requests made with the
// reference signers and the answer @dcl/crypto-middleware 6.5.0's
// `wellKnownComponents` gives each one at a fixed `now`.
//
//   pnpm add @dcl/crypto@3.9.0 @dcl/crypto-middleware@6.5.0 \
//     @dcl/test-helpers@0.3.2 decentraland-crypto-fetch@3.0.2
//   TZ=UTC node gen-request-vectors.js > request-vectors.json
//
// Keys are generated per run and never written out. A row whose `rust` field
// is set records where the Rust verifier deliberately answers differently.
'use strict'
const { Authenticator } = require('@dcl/crypto')
const { createUnsafeIdentity } = require('@dcl/crypto/dist/crypto')
const mw = require('@dcl/crypto-middleware')
const { signedFetchFactory } = require('decentraland-crypto-fetch')
const { getIdentity, getSignedAuthHeaders, createLocalFetchComponent } = require('@dcl/test-helpers')

if (new Date(0).getTimezoneOffset() !== 0) throw new Error('run with TZ=UTC')

const BASE = 'https://signcheck.example.invalid'
const NOW = Date.now()
Date.now = () => NOW

const SECP256K1_N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141n
const CONTRACT = '0x1234567890123456789012345678901234567890'
const LOW_ADDRESS = '0x00000000000000000000000000000000000000ff'

const root = createUnsafeIdentity()
const eph = createUnsafeIdentity()
const eph2 = createUnsafeIdentity()

const link = (type, payload, signature = '') => ({ type, payload, signature })
const sign = (identity, message) => Authenticator.createSignature(identity, message)
const iso = (ms) => new Date(ms).toISOString()

function delegation(text = iso(NOW + 600_000), from = root, to = eph, type = 'ECDSA_EPHEMERAL') {
  const message = `Decentraland Login\nEphemeral address: ${to.address}\nExpiration: ${text}`
  return link(type, message, sign(from, message))
}

function identity(expirationText) {
  return [link('SIGNER', root.address), delegation(expirationText)]
}

function signed(prefix, payload, key = eph) {
  return [...prefix, link('ECDSA_SIGNED_ENTITY', payload, sign(key, payload))]
}

function scopedLink(grants, expirationMs = NOW + 600_000, from = root, to = eph) {
  const message = Authenticator.getScopedEphemeralMessage(to.address, new Date(expirationMs), grants)
  return link('ECDSA_SCOPED_EPHEMERAL', message, sign(from, message))
}

function chainHeaders(chain, ts, meta) {
  const pairs = chain.map((l, i) => [`x-identity-auth-chain-${i}`, JSON.stringify(l)])
  if (ts !== undefined) pairs.push(['x-identity-timestamp', ts])
  if (meta !== undefined) pairs.push(['x-identity-metadata', meta])
  return pairs
}

/** A request signed over the current payload by `prefix`'s last key. */
function request(method, path, opts = {}) {
  const { prefix = identity(), key = eph, signPath } = opts
  const ts = 'ts' in opts ? opts.ts : String(NOW)
  const meta = 'meta' in opts ? opts.meta : '{}'
  const signTs = 'signTs' in opts ? opts.signTs : ts
  const signMeta = 'signMeta' in opts ? opts.signMeta : meta
  const payload = mw.createPayload(method, signPath ?? path, signTs, signMeta)
  return { method, path, headers: chainHeaders(signed(prefix, payload, key), ts, meta) }
}

function highS(signature) {
  const hex = signature.slice(2)
  const s = BigInt('0x' + hex.slice(64, 128))
  const v = parseInt(hex.slice(128, 130), 16)
  const flipped = (SECP256K1_N - s).toString(16).padStart(64, '0')
  return '0x' + hex.slice(0, 64) + flipped + (v === 27 ? '1c' : '1b')
}

function fakeProvider(mode) {
  return {
    sendAsync(payload, callback) {
      setImmediate(() => {
        if (mode === 'down') return callback(new Error('down'))
        const reply = (result) => callback(null, { jsonrpc: '2.0', id: payload.id, result })
        switch (payload.method) {
          case 'eth_call':
            if (mode === 'accept') return reply('0x1626ba7e' + '0'.repeat(56))
            return callback(null, { jsonrpc: '2.0', id: payload.id, error: { code: 3, message: 'execution reverted' } })
          case 'eth_blockNumber':
            return reply('0x64')
          case 'eth_getBlockByNumber': {
            const tag = payload.params[0]
            const n = tag === 'latest' ? 100 : parseInt(tag, 16)
            return reply({ timestamp: '0x' + Math.floor(NOW / 1000 - (100 - n) * 12).toString(16) })
          }
          default:
            return callback(new Error('unexpected ' + payload.method))
        }
      })
    }
  }
}

function resolver(spec) {
  if (spec === undefined || typeof spec === 'string') return spec
  if (spec.fn === 'none') return () => undefined
  if (spec.fn === 'throw') return () => {
    throw new Error('unresolvable')
  }
  throw new Error('bad resolver')
}

function middlewareOptions(o) {
  const out = {}
  for (const key of ['canonicalMetadataKeys', 'service', 'optional', 'expiration', 'maxChainLength', 'maxScopedLoginLifetime']) {
    if (o[key] !== undefined) out[key] = o[key]
  }
  if (o.operation !== undefined) out.operation = resolver(o.operation)
  if (o.resource !== undefined) out.resource = resolver(o.resource)
  if (o.metadataValidator) {
    const [name, ...values] = o.metadataValidator.split(':')
    out.metadataValidator = mw[name](...values.join(':').split(','))
  }
  if (o.provider) out.provider = fakeProvider(o.provider)
  return out
}

async function verdict(v) {
  const handler = mw.wellKnownComponents(middlewareOptions(v.options))
  const headers = new Headers()
  for (const [name, value] of v.headers) headers.append(name, value)
  const ctx = { request: new Request(BASE + v.path, { method: v.method, headers }), url: new URL(BASE + v.path) }
  const res = await handler(ctx, async () => ({ status: 200 }))
  if (res.status !== 200) return { status: res.status, ...(res.body?.code ? { code: res.body.code } : {}) }
  const out = { status: 200 }
  if (ctx.verification) {
    out.auth = ctx.verification.auth
    out.metadata = ctx.verification.authMetadata
    if (ctx.verification.scope) out.scope = ctx.verification.scope
    if (ctx.verification.resource !== undefined) out.resource = ctx.verification.resource
  } else {
    out.anonymous = true
  }
  return out
}

async function main() {
  const rows = []
  const add = (name, req, options = {}, extra = {}) => rows.push({ name, ...req, local: extra.local ?? req.path, options, ...extra })

  // Reference clients, as captured on the wire.
  const cfIdentity = await Authenticator.initializeAuthChain(root.address, eph, 10, async (m) => sign(root, m))
  let captured
  const signedFetch = signedFetchFactory({
    fetch: async (req) => {
      captured = { method: req.method, url: new URL(req.url), headers: [...req.headers] }
      return new Response('ok')
    }
  })
  const cf = async (name, url, init, options = {}, extra = {}) => {
    await signedFetch(url, { identity: cfIdentity, ...init })
    add(name, { method: captured.method, path: extra.path ?? captured.url.pathname, headers: [...captured.headers, ...(extra.headers ?? [])] }, options, extra)
  }
  const thIdentity = await getIdentity(10)
  const origFetch = globalThis.fetch
  let lf
  globalThis.fetch = async (url, init) => {
    lf = { url: new URL(url), method: init?.method ?? 'GET', headers: Object.entries(init?.headers ?? {}) }
    return new Response('ok')
  }
  const localFetch = await createLocalFetchComponent({
    requireString: async () => 'signcheck.example.invalid',
    requireNumber: async () => 443
  })
  const th = async (name, path, init, options = {}) => {
    await localFetch.fetch(path, { identity: thIdentity, ...init })
    add(name, { method: lf.method, path: lf.url.pathname, headers: lf.headers.map(([k, v]) => [k.toLowerCase(), v]) }, options)
  }
  const META = { signer: 'dcl:explorer', intent: 'x' }
  const LEGACY = { canonicalMetadataKeys: ['signer'] }
  for (const [suffix, options] of [['', {}], [' +canonicalMetadataKeys', LEGACY]]) {
    await cf('cf: GET' + suffix, BASE + '/v1/ping', { method: 'GET' }, options)
    await th('th: GET' + suffix, '/v1/ping', { method: 'GET' }, options)
    await cf('cf: POST metadata' + suffix, BASE + '/v1/action', { method: 'POST', metadata: META }, options)
    await th('th: POST metadata' + suffix, '/v1/action', { method: 'POST', metadata: META }, options)
    await cf('cf: reordered keys' + suffix, BASE + '/v1/action', { method: 'POST', metadata: { intent: 'x', signer: 'dcl:explorer' } }, options)
    await th('th: mixed-case metadata' + suffix, '/v1/action', {
      method: 'POST',
      metadata: { Signer: 'DCL:Explorer', realmName: 'Example.dcl.eth', sceneId: 'QmAbCdEf' }
    }, options)
    await th('th: uppercase path' + suffix, '/V1/Users/0xAbC/Profile', { method: 'GET' }, options)
    add('th-direct: GET' + suffix, {
      method: 'GET',
      path: '/v1/ping',
      headers: Object.entries(getSignedAuthHeaders('GET', '/v1/ping', {}, thIdentity)).map(([k, v]) => [k.toLowerCase(), v])
    }, options)
    await cf('cf: mixed-case metadata' + suffix, BASE + '/v1/action', {
      method: 'POST',
      metadata: { Signer: 'DCL:Explorer', realmName: 'Example.dcl.eth', sceneId: 'QmAbCdEf' }
    }, options)
    await cf('cf: Latin-1 metadata' + suffix, BASE + '/v1/action', { method: 'POST', metadata: { intent: 'x', realm: { serverName: 'niño' } } }, options)
    await cf('cf: path with query' + suffix, BASE + '/v1/items?limit=10&Cursor=AbC', { method: 'GET' }, options)
    await th('th: path with query, signed with it' + suffix, '/v1/items?limit=10', { method: 'GET' }, options)
    await cf('cf: uppercase path' + suffix, BASE + '/V1/Users/0xAbC/Profile', { method: 'GET' }, options)
    await cf('cf: lowercase method' + suffix, BASE + '/v1/action', { method: 'post', metadata: META }, options)
  }
  {
    await signedFetch(BASE + '/social-api/v1/x', { identity: cfIdentity, method: 'POST', metadata: META })
    add('x-original-path: gateway prefix', { method: 'POST', path: '/v1/x', headers: [...captured.headers, ['x-original-path', '/social-api/v1/x?foo=1']] }, {}, {
      rust: { status: 200, reason: 'x-original-path names the path the client signed' }
    })
    await signedFetch(BASE + '/other/v1/y', { identity: cfIdentity, method: 'POST', metadata: META })
    add('x-original-path: another route', { method: 'POST', path: '/v1/x', headers: [...captured.headers, ['x-original-path', '/other/v1/y']] })
  }
  globalThis.fetch = origFetch

  // The formats the Rust signers produce.
  {
    const sceneMeta = JSON.stringify({
      origin: BASE, sceneId: 'QmFakeSceneIdAbC', parcel: '0,0', tld: 'org', network: 'mainnet', isGuest: false,
      realm: { hostname: 'signcheck.example.invalid', protocol: 'v3', serverName: 'Example.dcl.eth' },
      signer: 'decentraland-kernel-scene'
    })
    add('explorer: scene metadata', request('POST', '/v1/action', { meta: sceneMeta }))
    add('explorer: empty metadata header', request('GET', '/v1/ping', { meta: '' }))
    add('explorer: uppercase path', request('GET', '/V1/Ping'))
    const simple = (method, path, meta, payload = mw.createPayload(method, path, String(NOW), meta)) => ({
      method,
      path,
      headers: chainHeaders([link('SIGNER', root.address), link('ECDSA_SIGNED_ENTITY', payload, sign(root, payload))], String(NOW), meta)
    })
    add('scene-cli: two-link chain', simple('PUT', '/world/example.dcl.eth/settings', '{"access":{"type":"allow-list","wallets":["0xabc"]}}'))
    const fold = (method, path, meta) => simple(method, path, meta, [method, path, String(NOW), meta].join(':').toLowerCase())
    const worldMeta = '{"realm":{"serverName":"example.dcl.eth"},"realmName":"example.dcl.eth","parcel":"0,0"}'
    add('scene-cli storage: folded payload', fold('PUT', '/values/high-score', worldMeta))
    add('scene-cli storage: folded payload +canonicalMetadataKeys', fold('PUT', '/values/high-score', worldMeta), LEGACY)
    add('scene-cli storage: folded lowercase payload', fold('PUT', '/values/high-score', '{"parcel":"0,0"}'))
  }

  // Metadata bytes.
  add('raw: whitespace in metadata', request('POST', '/v1/action', { meta: '{ "intent" :\t"x" ,  "signer":"dcl:explorer" }' }))
  add('raw: re-spaced after signing', request('POST', '/v1/action', { meta: '{ "intent": "x" }', signMeta: '{"intent":"x"}' }))
  add('raw: re-cased after signing', request('POST', '/v1/action', { meta: '{"Intent":"x"}', signMeta: '{"intent":"x"}' }))
  add('raw: re-cased after signing +canonicalMetadataKeys', request('POST', '/v1/action', { meta: '{"Intent":"x"}', signMeta: '{"intent":"x"}' }), LEGACY)
  add('metadata null', request('POST', '/v1/action', { meta: 'null' }))
  add('metadata absent', request('GET', '/v1/ping', { meta: undefined, signMeta: '' }))
  add('metadata absent, signed as {}', request('GET', '/v1/ping', { meta: undefined, signMeta: '{}' }))
  add('metadata empty string', request('GET', '/v1/ping', { meta: '' }))
  add('metadata array', request('GET', '/v1/ping', { meta: '[]' }))
  {
    const r = request('POST', '/v1/action', { meta: '{"a":1}' })
    r.headers.push(['x-identity-metadata', '{"a":1}'])
    add('metadata header repeated', r)
  }
  add('validator: scene refused', request('POST', '/v1/action', { meta: '{"signer":"decentraland-kernel-scene"}' }), {
    metadataValidator: 'rejectIfSigner:decentraland-kernel-scene'
  })
  add('validator: explorer passes', request('POST', '/v1/action', { meta: '{"signer":"dcl:explorer"}' }), {
    metadataValidator: 'rejectIfSigner:decentraland-kernel-scene'
  })
  add('validator: required signer', request('POST', '/v1/action', { meta: '{"signer":"decentraland-kernel-scene"}' }), {
    metadataValidator: 'requireSigner:decentraland-kernel-scene'
  })

  // Legacy (pre-6.0.0) payload.
  {
    const meta = JSON.stringify({ signer: 'dcl:explorer', realmName: 'Example.dcl.eth' })
    const legacy = (m = meta) => ['POST', '/v1/action', String(NOW), m].join(':').toLowerCase()
    const legacyRequest = (m = meta) => ({ method: 'POST', path: '/v1/action', headers: chainHeaders(signed(identity(), legacy(m)), String(NOW), m) })
    add('legacy payload', legacyRequest())
    add('legacy payload +canonicalMetadataKeys', legacyRequest(), LEGACY)
    add('legacy payload, folded key', legacyRequest('{"Signer":"dcl:explorer"}'), LEGACY)
    add('legacy payload, nested path', legacyRequest('{"realm":{"serverName":"a"}}'), { canonicalMetadataKeys: ['realm.serverName'] })
    add('legacy payload, folded nested key', legacyRequest('{"realm":{"servername":"a"}}'), { canonicalMetadataKeys: ['realm.serverName'] })
    const expired = identity(iso(NOW - 1))
    const meta2 = '{"Signer":"x"}'
    add('expired delegation, folded key +canonicalMetadataKeys', { method: 'POST', path: '/v1/action', headers: chainHeaders(signed(expired, mw.createPayload('POST', '/v1/action', String(NOW), meta2)), String(NOW), meta2) }, LEGACY)
  }

  // Chain headers.
  {
    const r = request('GET', '/v1/ping')
    add('chain: link 1 empty', { ...r, headers: r.headers.map(([k, v]) => [k, k === 'x-identity-auth-chain-1' ? '' : v]) })
    add('chain: link 2 empty, link 3 present', {
      ...r,
      headers: [...r.headers.map(([k, v]) => [k, k === 'x-identity-auth-chain-2' ? '' : v]), ['x-identity-auth-chain-3', r.headers[2][1]]]
    })
    add('chain: link at index 10', { ...r, headers: [...r.headers, ['x-identity-auth-chain-10', '{}']] })
    add('chain: empty link at index 10', { ...r, headers: [...r.headers, ['x-identity-auth-chain-10', '']] })
    add('chain: link at index 3, max 3', { ...r, headers: [...r.headers, ['x-identity-auth-chain-3', '{}']] }, { maxChainLength: 3 })
    add('chain: three links, max 3', r, { maxChainLength: 3 })
    add('chain: link repeated', { ...r, headers: [...r.headers, ['x-identity-auth-chain-1', r.headers[1][1]]] })
    add('chain: not JSON', { ...r, headers: r.headers.map(([k, v]) => [k, k === 'x-identity-auth-chain-2' ? '{' : v]) })
    add('chain: SIGNER without signature', {
      ...r,
      headers: r.headers.map(([k, v]) => [k, k === 'x-identity-auth-chain-0' ? JSON.stringify({ type: 'SIGNER', payload: root.address }) : v])
    })
    add('chain: unknown link type', {
      ...r,
      headers: r.headers.map(([k, v]) => [k, k === 'x-identity-auth-chain-2' ? v.replace('ECDSA_SIGNED_ENTITY', 'ECDSA_SIGNED_THING') : v])
    })
    add('chain: one link', { ...r, headers: r.headers.filter(([k]) => k !== 'x-identity-auth-chain-1' && k !== 'x-identity-auth-chain-2') })
    const unprefixed = r.headers.map(([k, v]) => {
      if (k !== 'x-identity-auth-chain-2') return [k, v]
      const l = JSON.parse(v)
      return [k, JSON.stringify({ ...l, signature: l.signature.slice(2) })]
    })
    add('chain: signature without 0x', { ...r, headers: unprefixed })
    const malleated = r.headers.map(([k, v]) => {
      if (k !== 'x-identity-auth-chain-2') return [k, v]
      const l = JSON.parse(v)
      return [k, JSON.stringify({ ...l, signature: highS(l.signature) })]
    })
    add('chain: high-s signature', { ...r, headers: malleated }, {}, { rust: { status: 401, reason: 'high-s signatures are refused' } })
    const relay = createUnsafeIdentity()
    const payload = mw.createPayload('GET', '/v1/ping', String(NOW), '{}')
    const midEntity = [
      link('SIGNER', root.address),
      link('ECDSA_SIGNED_ENTITY', relay.address, sign(root, relay.address)),
      link('ECDSA_SIGNED_ENTITY', payload, sign(relay, payload))
    ]
    add('chain: signed entity mid-chain', { method: 'GET', path: '/v1/ping', headers: chainHeaders(midEntity, String(NOW), '{}') }, {}, {
      rust: { status: 401, reason: 'only delegations sit between the SIGNER and the last link' }
    })
    add('chain: identity only', { method: 'GET', path: '/v1/ping', headers: chainHeaders(identity(), String(NOW), '{}') })
    add('chain: delegation expired', request('GET', '/v1/ping', { prefix: identity(iso(NOW)) }))
    add('chain: delegation expires 1 ms later', request('GET', '/v1/ping', { prefix: identity(iso(NOW + 1)) }))
    add('chain: two delegations', request('GET', '/v1/ping', {
      prefix: [link('SIGNER', root.address), delegation(iso(NOW + 600_000)), delegation(iso(NOW + 600_000), eph, eph2)],
      key: eph2
    }))
    add('chain: wrong key', request('GET', '/v1/ping', { key: eph2 }))
    for (const text of [
      new Date(NOW + 86_400_000).toString(),
      new Date(NOW + 86_400_000).toUTCString(),
      iso(NOW + 86_400_000).slice(0, 10),
      iso(NOW + 86_400_000).slice(0, 19),
      'Feb 13, 2099 3:31 PM',
      'next Tuesday'
    ]) {
      add(`expiration: ${text}`, request('GET', '/v1/ping', { prefix: identity(text) }))
    }
  }

  // Timestamp.
  for (const [label, ts, rust] of [
    ['absent', undefined],
    ['empty', ''],
    ['not a number', 'soon'],
    ['exponent', '1e3', { status: 400, reason: 'digits only' }],
    ['leading space', ' ' + NOW, { status: 400, reason: 'digits only' }],
    ['negative', '-5', { status: 400, reason: 'digits only' }],
    ['decimal', NOW + '.0', { status: 400, reason: 'digits only' }],
    ['leading zero', '0' + NOW],
    ['window start', String(NOW - 60_000)],
    ['expired by 1 ms', String(NOW - 60_001)],
    ['window end', String(NOW + 60_000)],
    ['future by 1 ms', String(NOW + 60_001)],
    ['huge', '9'.repeat(30)],
    ['repeated', `${NOW}, ${NOW}`]
  ]) {
    const signTs = ts ?? ''
    const r = request('GET', '/v1/ping', { ts, signTs })
    add(`timestamp: ${label}`, r, {}, rust ? { rust } : {})
  }
  add('timestamp: 5 min window', request('GET', '/v1/ping', { ts: String(NOW - 299_000) }), { expiration: 300_000 })
  add('timestamp: 5 min window, 61 s ahead', request('GET', '/v1/ping', { ts: String(NOW + 61_000) }), { expiration: 300_000 }, {
    rust: { status: 401, reason: 'the future side stops at 60 s' }
  })

  // Contract wallets.
  {
    const contractChain = (signer = CONTRACT) => {
      const message = `Decentraland Login\nEphemeral address: ${eph.address}\nExpiration: ${iso(NOW + 600_000)}`
      return [link('SIGNER', signer), link('ECDSA_EIP_1654_EPHEMERAL', message, sign(root, message))]
    }
    add('eip1654: accepted', request('GET', '/v1/ping', { prefix: contractChain() }), { provider: 'accept' })
    add('eip1654: refused', request('GET', '/v1/ping', { prefix: contractChain() }), { provider: 'reject' })
    add('eip1654: rpc down', request('GET', '/v1/ping', { prefix: contractChain() }), { provider: 'down' })
    add('eip1654: precompile address', request('GET', '/v1/ping', { prefix: contractChain(LOW_ADDRESS) }), { provider: 'accept' }, {
      rust: { status: 401, reason: 'addresses 0x00..00 to 0x00..ff are not asked' }
    })
  }

  // Scoped logins.
  {
    const S = 'insights'
    const route = { service: S, operation: 'logs-read' }
    const scoped = (grants, expirationMs, prefix = [link('SIGNER', root.address)], from = root, to = eph) => [
      ...prefix,
      scopedLink(grants, expirationMs, from, to)
    ]
    const g = (operation, resource, service = S) => ({ service, operation, ...(resource ? { resource } : {}) })
    const req = (grants, expirationMs) => request('GET', '/v1/logs', { prefix: scoped(grants, expirationMs) })
    add('scoped: granted', req([g('logs-read'), g('x', undefined, 'other')]), route)
    add('scoped: wildcard', req([g('*')]), route)
    add('scoped: denied', req([g('logs-write')]), route)
    add('scoped: no service', req([g('logs-read')]), {})
    add('scoped: grants another service', req([g('logs-read', undefined, 'other')]), route)
    add('scoped: no route operation', req([g('logs-read')]), { service: S })
    add('scoped: route operation undefined', req([g('logs-read')]), { service: S, operation: { fn: 'none' } })
    add('scoped: lifetime at cap', req([g('logs-read')], NOW + 86_400_000), route)
    add('scoped: lifetime over cap', req([g('logs-read')], NOW + 86_400_001), route)
    add('scoped: lifetime 1h cap', req([g('logs-read')], NOW + 3_600_001), { ...route, maxScopedLoginLifetime: 3_600_000 })
    add('scoped: expired', req([g('logs-read')], NOW), route)
    add('scoped: resource granted', req([g('logs-read', 'scene:a')]), { ...route, resource: 'scene:a' })
    add('scoped: resource denied', req([g('logs-read', 'scene:a')]), { ...route, resource: 'scene:b' })
    add('scoped: grant limited, route has none', req([g('logs-read', 'scene:a')]), route)
    add('scoped: resource unresolved', req([g('logs-read')]), { ...route, resource: { fn: 'none' } })
    add('scoped: resource throws', req([g('logs-read')]), { ...route, resource: { fn: 'throw' } })
    add('scoped: resource malformed', req([g('logs-read')]), { ...route, resource: 'scene a' })
    add('scoped: after a delegation', request('GET', '/v1/logs', {
      prefix: scoped([g('logs-read')], NOW + 600_000, [link('SIGNER', root.address), delegation(iso(NOW + 600_000), root, eph2)], eph2, eph)
    }), route)
    add('scoped: identity only', { method: 'GET', path: '/v1/logs', headers: chainHeaders(scoped([g('logs-read')]), String(NOW), '{}') }, route)
    add('scoped: optional, denied', req([g('logs-write')]), { ...route, optional: true })
    add('full login: route with operation', request('GET', '/v1/logs'), route)
    add('full login: route resource', request('GET', '/v1/logs'), { ...route, resource: 'scene:a' })
    add('full login: malformed resource', request('GET', '/v1/logs'), { ...route, resource: 'scene a' })
    add('full login: resource unresolved', request('GET', '/v1/logs'), { ...route, resource: { fn: 'none' } })
    {
      const message = `Decentraland Scoped Login\nEphemeral: ${eph.address.toLowerCase()}\nExpiration: ${iso(NOW + 600_000).replace('.000Z', 'Z').replace(/\.\d{3}Z$/, 'Z')}\nAllow: ${S}:logs-read`
      add('scoped: expiration not toISOString', request('GET', '/v1/logs', {
        prefix: [link('SIGNER', root.address), link('ECDSA_SCOPED_EPHEMERAL', message, sign(root, message))]
      }), route)
    }
    // Messages the signer refuses to build, signed as they are.
    const lines = (grants, address = eph.address.toLowerCase(), expiration = iso(NOW + 600_000)) =>
      ['Decentraland Scoped Login', `Ephemeral: ${address}`, `Expiration: ${expiration}`, ...grants.map((l) => `Allow: ${l}`)].join('\n')
    const signedScoped = (message, type = 'ECDSA_SCOPED_EPHEMERAL', prefix = [link('SIGNER', root.address)], from = root) =>
      request('GET', '/v1/logs', { prefix: [...prefix, link(type, message, sign(from, message))] })
    add('scoped: CRLF message', signedScoped(lines([`${S}:logs-read`]).replace(/\n/g, '\r\n')), route)
    add('scoped: uppercase ephemeral address', signedScoped(lines([`${S}:logs-read`], '0x' + eph.address.slice(2).toUpperCase())), route)
    add('scoped: repeated grant', signedScoped(lines([`${S}:logs-read`, `${S}:logs-read`])), route)
    add('scoped: 33 grants', signedScoped(lines([`${S}:logs-read`, ...Array.from({ length: 32 }, (_, i) => `${S}:op-${i}`)])), route)
    add('scoped: 32 grants', signedScoped(lines([`${S}:logs-read`, ...Array.from({ length: 31 }, (_, i) => `${S}:op-${i}`)])), route)
    add('scoped: over 4096 bytes', signedScoped(lines([`${S}:logs-read`, ...Array.from({ length: 20 }, (_, i) => `${S}:op-${i} for scene:${'a'.repeat(200)}`)])), route)
    add('scoped: grant with spaces', signedScoped(lines([`${S}:logs-read for a b`])), route)
    add('scoped: grant uppercase', signedScoped(lines([`${S}:Logs-Read`])), route)
    {
      const first = scopedLink([g('logs-read')], NOW + 600_000, root, eph2)
      add('scoped: two scoped links', request('GET', '/v1/logs', {
        prefix: [link('SIGNER', root.address), first, scopedLink([g('logs-read')], NOW + 600_000, eph2, eph)]
      }), route)
      add('scoped: delegation after the scoped link', request('GET', '/v1/logs', {
        prefix: [link('SIGNER', root.address), first, delegation(iso(NOW + 600_000), eph2, eph)]
      }), route)
    }
    {
      const scopedMessage = Authenticator.getScopedEphemeralMessage(eph.address, new Date(NOW + 600_000), [g('logs-read')])
      const contractScoped = link('ECDSA_EIP_1654_SCOPED_EPHEMERAL', scopedMessage, sign(root, scopedMessage))
      add('scoped: contract wallet', request('GET', '/v1/logs', { prefix: [link('SIGNER', CONTRACT), contractScoped] }), { ...route, provider: 'accept' })
      add('scoped: contract wallet refused', request('GET', '/v1/logs', { prefix: [link('SIGNER', CONTRACT), contractScoped] }), { ...route, provider: 'reject' })
      add('scoped: contract wallet, no service', request('GET', '/v1/logs', { prefix: [link('SIGNER', CONTRACT), contractScoped] }), { provider: 'accept' })
      const contractDelegation = (() => {
        const m = `Decentraland Login\nEphemeral address: ${eph2.address}\nExpiration: ${iso(NOW + 600_000)}`
        return link('ECDSA_EIP_1654_EPHEMERAL', m, sign(root, m))
      })()
      const lateMessage = Authenticator.getScopedEphemeralMessage(eph.address, new Date(NOW + 600_000), [g('logs-read')])
      add('scoped: contract scoped link not at index 1', request('GET', '/v1/logs', {
        prefix: [link('SIGNER', CONTRACT), contractDelegation, link('ECDSA_EIP_1654_SCOPED_EPHEMERAL', lateMessage, sign(eph2, lateMessage))]
      }), { ...route, provider: 'accept' })
    }
  }

  // Optional.
  add('optional: no headers', { method: 'GET', path: '/v1/ping', headers: [] }, { optional: true })
  add('optional: expired', request('GET', '/v1/ping', { ts: String(NOW - 120_000) }), { optional: true })
  add('no headers', { method: 'GET', path: '/v1/ping', headers: [] })

  for (const row of rows) row.expected = await verdict(row)
  process.stdout.write(JSON.stringify({ generator: `node ${process.version}, @dcl/crypto-middleware 6.5.0, @dcl/crypto 3.9.0`, now: NOW, vectors: rows }, null, 1) + '\n')
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
