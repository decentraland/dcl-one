'use strict'

/** Loopback-only network guard for the builder capture and its migration child; see `refused`. */

const dgram = require('node:dgram')
const dns = require('node:dns')
const net = require('node:net')

const PROXY_VARIABLES = ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NODE_USE_ENV_PROXY']
const DNS_QUERIES = ['resolve', 'resolve4', 'resolve6', 'resolveAny', 'resolveCaa', 'resolveCname', 'resolveMx', 'resolveNaptr',
  'resolveNs', 'resolvePtr', 'resolveSoa', 'resolveSrv', 'resolveTlsa', 'resolveTxt', 'reverse']
const refused = []

const proxied = PROXY_VARIABLES.flatMap(name => [name, name.toLowerCase()]).filter(name => process.env[name] !== undefined)
if (proxied.length) throw Error(`unset ${proxied.join(', ')} before capturing: a proxy would carry requests past the offline guard`)

function isLoopbackAddress(host) {
  const address = String(host).replace(/^\[|\]$/g, '').replace(/^::ffff:/i, '')
  if (net.isIPv4(address)) return address.startsWith('127.')
  return net.isIPv6(address) && /^(0*:)*:?0*1$/.test(address)
}

function isLoopback(host) {
  return host === undefined || host === null || host === '' || host === 'localhost' || isLoopbackAddress(host)
}

function never() {
  return false
}

function refuse(target) {
  refused.push(target)
  return Object.assign(Error(`capture is offline: ${target}`), { code: 'ECONNREFUSED' })
}

function guardCallbacks(target, names, allowed) {
  for (const name of names) {
    const original = target[name]
    if (typeof original !== 'function') continue
    target[name] = function (hostname, ...rest) {
      if (allowed(hostname)) return original.call(this, hostname, ...rest)
      const callback = rest[rest.length - 1]
      const error = refuse(hostname)
      process.nextTick(() => callback(error))
      return {}
    }
  }
}

function guardPromises(target, names, allowed) {
  for (const name of names) {
    const original = target[name]
    if (typeof original !== 'function') continue
    target[name] = function (hostname, ...rest) {
      if (allowed(hostname)) return original.call(this, hostname, ...rest)
      return Promise.reject(refuse(hostname))
    }
  }
}

guardCallbacks(dns, ['lookup', 'lookupService'], isLoopback)
guardCallbacks(dns, DNS_QUERIES, never)
guardCallbacks(dns.Resolver.prototype, DNS_QUERIES, never)
guardPromises(dns.promises, ['lookup', 'lookupService'], isLoopback)
guardPromises(dns.promises, DNS_QUERIES, never)
guardPromises(dns.promises.Resolver.prototype, DNS_QUERIES, never)

const connect = net.Socket.prototype.connect
net.Socket.prototype.connect = function (...args) {
  const first = Array.isArray(args[0]) ? args[0][0] : args[0]
  const options = first !== null && typeof first === 'object' ? first : { port: first, host: typeof args[1] === 'string' ? args[1] : undefined }
  const local = Boolean(options.path) || (typeof first === 'string' && !(Number(first) >= 0))
  const loopback = typeof options.lookup === 'function' ? isLoopbackAddress(options.host) : isLoopback(options.host)
  if (local || loopback) return connect.apply(this, args)
  const error = refuse(`${options.host}:${options.port}`)
  process.nextTick(() => this.destroy(error))
  return this
}

const datagramConnect = dgram.Socket.prototype.connect
dgram.Socket.prototype.connect = function (port, address, ...rest) {
  if (isLoopback(typeof address === 'function' ? undefined : address)) return datagramConnect.call(this, port, address, ...rest)
  throw refuse(`udp ${address}:${port}`)
}

const send = dgram.Socket.prototype.send
dgram.Socket.prototype.send = function (...args) {
  const plain = args.filter(arg => typeof arg !== 'function')
  const address = plain.length > 1 && typeof plain.at(-1) === 'string' ? plain.at(-1) : undefined
  if (isLoopback(address)) return send.apply(this, args)
  throw refuse(`udp ${address}`)
}

process.on('exit', () => {
  if (!refused.length) return
  process.exitCode = 1
  require('node:fs').writeSync(2, `offline capture refused: ${[...new Set(refused)].join(', ')}\n`)
})

module.exports = { refused, isLoopback, PROXY_VARIABLES }
