import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import { runInNewContext } from 'node:vm'

const require = createRequire(import.meta.url)
const { transformSync } = createRequire(require.resolve('@dcl/sdk-commands/package.json'))('esbuild')
const source = transformSync(readFileSync(new URL('../src/login.ts', import.meta.url), 'utf8'), {
  loader: 'ts', format: 'cjs'
}).code

function fixture(player = null, previous = null) {
  const calls = []
  const current = { player }
  const api = {
    getPreviousLogin: async () => { calls.push('previous'); return previous },
    loginPrevious: async () => { calls.push('restore'); current.player = { userId: previous.userId } },
    loginGuest: () => { calls.push('guest'); current.player = { userId: 'new-guest' } }
  }
  const modules = {
    '@dcl/sdk/players': { getPlayer: () => current.player },
    './bevy-api': { BevyApi: api },
    './utils': { waitFor: async predicate => { assert.ok(predicate(), 'player must become available') } }
  }
  const module = { exports: {} }
  runInNewContext(source, {
    module, console: { log() {}, error() {} },
    require(name) { assert.ok(name in modules, `unexpected dependency ${name}`); return modules[name] }
  })
  return { login: module.exports.autoLogin, current, api, calls }
}

test('an existing player is preserved without another login or wallet change', async () => {
  const f = fixture({ userId: 'connected-wallet' }, { userId: 'saved-wallet' })
  await f.login()
  assert.deepEqual(f.calls, [])
  assert.equal(f.current.player.userId, 'connected-wallet')
})

test('a player that arrives during saved-login lookup is not replaced with a guest', async () => {
  const f = fixture()
  f.api.getPreviousLogin = async () => {
    f.calls.push('previous')
    f.current.player = { userId: 'engine-guest' }
    return null
  }
  await f.login()
  assert.deepEqual(f.calls, ['previous'])
  assert.equal(f.current.player.userId, 'engine-guest')
})

test('without a current player the saved identity is restored', async () => {
  const f = fixture(null, { userId: 'saved-wallet' })
  await f.login()
  assert.deepEqual(f.calls, ['previous', 'restore'])
  assert.equal(f.current.player.userId, 'saved-wallet')
})

test('without a current or saved player guest login still works', async () => {
  const f = fixture()
  await f.login()
  assert.deepEqual(f.calls, ['previous', 'guest'])
  assert.equal(f.current.player.userId, 'new-guest')
})
