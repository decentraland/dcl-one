#!/usr/bin/env node
// Executes upstream's actual Redis adapter and friends component against an
// isolated Redis over a Unix socket. No listener port or project dependency.
const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const crypto = require('node:crypto')
const { spawn, execFileSync } = require('node:child_process')
const Module = require('node:module')
const upstream = path.resolve(process.argv[2] || '')
const output = process.argv[3]
if (!process.argv[2] || !output) throw Error('Usage: node tooling/social-service-limiter-cases.cjs <upstream> <output.json> [redis-server]')
const revision = 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6'
if (execFileSync('git', ['-C', upstream, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim() !== revision) throw Error('Wrong baseline')
if (execFileSync('git', ['-C', upstream, 'status', '--porcelain', '--untracked-files=no'], { encoding: 'utf8' }).trim()) throw Error('Modified baseline')
const requireUpstream = Module.createRequire(path.join(upstream, 'package.json'))
requireUpstream('ts-node').register({ transpileOnly: true, project: path.join(upstream, 'tsconfig.json') })
const redisPackage = requireUpstream('redis')
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const delay = ms => new Promise(resolve => setTimeout(resolve, ms))

async function main() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'social-source-redis-'))
  const socket = path.join(directory, 'redis.sock')
  const server = spawn(process.argv[4] || 'redis-server', ['--port', '0', '--save', '', '--appendonly', 'no', '--unixsocket', socket], { stdio: 'ignore' })
  const client = redisPackage.createClient({ socket: { path: socket } })
  let adapter
  try {
    for (let i = 0; !fs.existsSync(socket); i++) {
      if (i > 100 || server.exitCode !== null) throw Error('Redis did not start')
      await delay(20)
    }
    const originalLoad = Module._load
    let createRedisComponent
    try {
      Module._load = function (request, parent, isMain) {
        if (request === 'redis' && parent?.filename === path.join(upstream, 'src/adapters/redis.ts')) return { ...redisPackage, createClient: () => client }
        return originalLoad.call(this, request, parent, isMain)
      }
      createRedisComponent = requireUpstream(path.join(upstream, 'src/adapters/redis.ts')).createRedisComponent
    } finally { Module._load = originalLoad }
    adapter = await createRedisComponent({ logs, config: { getString: async () => undefined } })
    await adapter.start()
    const createFriends = requireUpstream(path.join(upstream, 'src/logic/friends/component.ts')).createFriendsComponent
    const cases = []
    async function scenario(id, configuration, attempts) {
      await client.flushDb()
      const makeFriends = () => createFriends({
        logs, redis: adapter, metrics: { increment() {} }, config: { getNumber: async key => configuration[key] }, registry: {}, sns: {},
        pubsub: { publishInChannel: async () => {} },
        friendsDb: { executeTx: async fn => fn({}), blockUser: async () => ({ blocked_at: new Date(0) }), getFriendship: async () => undefined }
      })
      const instances = [await makeFriends(), await makeFriends()]
      const observed = []
      let elapsed = 0
      for (const [instance, actor, target, afterMs] of attempts) {
        if (afterMs > elapsed) await delay(afterMs - elapsed)
        elapsed = afterMs
        let allowed = true
        let error = ''
        try { await instances[instance].blockUserWithoutProfile(actor, target) } catch (failure) { allowed = false; error = failure.message }
        const keys = (await client.keys('friends:rate:*')).sort()
        const counters = []
        for (const key of keys) {
          const ttl = await client.pTTL(key)
          counters.push({ key, attempts: Number(await client.get(key)), expiry: ttl === -1 ? 'persistent' : ttl === -2 ? 'absent' : 'expiring' })
        }
        observed.push({ instance, actor, target, after_ms: afterMs, allowed, error, counters })
      }
      cases.push({ id, configuration, attempts: observed })
    }
    await scenario('fixed-window-shared-instances', { FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS: 1, FRIENDSHIP_RATE_LIMIT_PER_ACTOR: 30, FRIENDSHIP_RATE_LIMIT_PER_PAIR: 2 }, [[0, 'actor', 'target', 0], [1, 'actor', 'target', 0], [0, 'actor', 'target', 0], [1, 'actor', 'target', 600], [0, 'actor', 'target', 1100]])
    await scenario('denied-actor-does-not-touch-pair', { FRIENDSHIP_RATE_LIMIT_PER_ACTOR: 1 }, [[0, 'actor', 'target', 0], [1, 'actor', 'other', 0]])
    await scenario('denied-pair-still-consumes-actor', { FRIENDSHIP_RATE_LIMIT_PER_PAIR: 1 }, [[0, 'actor', 'target', 0], [1, 'actor', 'target', 0]])
    for (const limit of [0, -1]) await scenario(`actor-limit-${limit}`, { FRIENDSHIP_RATE_LIMIT_PER_ACTOR: limit }, [[0, 'actor', 'target', 0]])
    for (const window of [0, -1]) await scenario(`window-${window}`, { FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS: window, FRIENDSHIP_RATE_LIMIT_PER_ACTOR: 1, FRIENDSHIP_RATE_LIMIT_PER_PAIR: 1 }, [[0, 'actor', 'target', 0], [1, 'actor', 'target', 0]])
    await scenario('fractional-expire-leaves-persistent-increment', { FRIENDSHIP_RATE_LIMIT_WINDOW_SECONDS: 1.5, FRIENDSHIP_RATE_LIMIT_PER_ACTOR: 1, FRIENDSHIP_RATE_LIMIT_PER_PAIR: 1 }, [[0, 'actor', 'target', 0], [1, 'actor', 'target', 0]])
    fs.writeFileSync(output, JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision, yarn_lock_sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream, 'yarn.lock'))).digest('hex') }, runtime: { node: process.version, redis: execFileSync(process.argv[4] || 'redis-server', ['--version'], { encoding: 'utf8' }).trim() }, scope: 'Real upstream Redis adapter Lua and friends limiter, real isolated Redis, two independent friends components. Domain writes/publications are injected; elapsed milliseconds are actual waits. These are block-directional cases; symmetric friendship keys have separate local tests.', cases }, null, 2) + '\n')
    process.stdout.write(`Captured ${cases.length} upstream Redis limiter scenarios\n`)
  } finally {
    if (client.isOpen) await adapter.stop()
    server.kill('SIGTERM')
    await new Promise(resolve => server.exitCode !== null ? resolve() : server.once('exit', resolve))
    fs.rmSync(directory, { recursive: true, force: true })
  }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
