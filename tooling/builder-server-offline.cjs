'use strict'

/** Loopback-only network guard for the builder capture and its migration child; see `refused`. */

const dns = require('node:dns')
const net = require('node:net')

const PROXY_VARIABLES = ['HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NO_PROXY', 'NODE_USE_ENV_PROXY']
const refused = []

for (const name of PROXY_VARIABLES) {
  delete process.env[name]
  delete process.env[name.toLowerCase()]
}

function isLoopback(host) {
  if (host === undefined || host === null || host === '' || host === 'localhost') return true
  const address = String(host).replace(/^\[|\]$/g, '').replace(/^::ffff:/i, '')
  if (net.isIPv4(address)) return address.startsWith('127.')
  return net.isIPv6(address) && /^(0*:)*:?0*1$/.test(address)
}

function refuse(target) {
  refused.push(target)
  return Object.assign(Error(`capture is offline: ${target}`), { code: 'ECONNREFUSED' })
}

const lookup = dns.lookup
dns.lookup = function (hostname, ...rest) {
  if (isLoopback(hostname)) return lookup.call(this, hostname, ...rest)
  const callback = rest[rest.length - 1]
  const error = refuse(hostname)
  process.nextTick(() => callback(error))
  return {}
}

const connect = net.Socket.prototype.connect
net.Socket.prototype.connect = function (...args) {
  const first = Array.isArray(args[0]) ? args[0][0] : args[0]
  const options = first !== null && typeof first === 'object' ? first : { port: first, host: args[1] }
  const local = options.path !== undefined || (typeof first === 'string' && !/^\d+$/.test(first))
  if (local || isLoopback(options.host)) return connect.apply(this, args)
  const error = refuse(`${options.host}:${options.port}`)
  process.nextTick(() => this.destroy(error))
  return this
}

process.on('exit', () => {
  if (!refused.length) return
  process.exitCode = 1
  require('node:fs').writeSync(2, `offline capture refused: ${[...new Set(refused)].join(', ')}\n`)
})

module.exports = { refused, isLoopback, PROXY_VARIABLES }
