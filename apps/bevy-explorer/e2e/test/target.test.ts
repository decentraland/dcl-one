import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { DEFAULT_WEB_APP } from '../driver/paths.ts'
import { accountKey, describeTarget, parseWindowSize, readTarget } from '../driver/target.ts'

const KEY_A = '0x' + 'a'.repeat(64)
const KEY_B = '0x' + 'b'.repeat(64)

describe('readTarget', () => {
  test('defaults to zone, the local web app and a headed browser', () => {
    const target = readTarget({})
    assert.equal(target.baseDomain, 'decentraland.zone')
    assert.equal(target.realm, 'https://peer.decentraland.zone')
    assert.equal(target.position, '100,100')
    assert.equal(target.transport, 'web')
    assert.equal(target.webApp, DEFAULT_WEB_APP)
    assert.equal(target.headed, true)
    assert.deepEqual(target.windowSize, [1600, 900])
    assert.deepEqual(target.accounts, [])
    assert.deepEqual(target.ports, { web: 18743, realm: 18744 })
    assert.equal(target.record, false)
  })

  test('reads the environment', () => {
    const target = readTarget({
      DCL_E2E_BASE_DOMAIN: 'example.test',
      DCL_E2E_REALM: 'https://realm.example.test',
      DCL_E2E_POSITION: '1,2',
      DCL_E2E_TRANSPORT: 'native',
      DCL_E2E_ACCOUNT_KEYS: ` ${KEY_A}, ${KEY_B} `,
      DCL_E2E_WORKER: '1',
      DCL_E2E_HEADLESS: '1',
      DCL_E2E_WINDOW: '960x540',
      DCL_E2E_RECORD: 'true',
      DCL_E2E_WEB_PORT: '4000',
      DCL_E2E_NATIVE_BIN: '/bin/client',
      DCL_E2E_NATIVE_ARGS: '--log-file run.log  --no_fog'
    })
    assert.equal(target.baseDomain, 'example.test')
    assert.equal(target.transport, 'native')
    assert.deepEqual(target.accounts, [KEY_A, KEY_B])
    assert.equal(accountKey(target), KEY_B)
    assert.equal(target.headed, false)
    assert.deepEqual(target.windowSize, [960, 540])
    assert.equal(target.record, true)
    assert.equal(target.ports.web, 4000)
    assert.deepEqual(target.native.args, ['--log-file', 'run.log', '--no_fog'])
  })

  test('refuses a bad transport, key or window', () => {
    assert.throws(() => readTarget({ DCL_E2E_TRANSPORT: 'cef' }), /DCL_E2E_TRANSPORT must be web or native/)
    assert.throws(() => readTarget({ DCL_E2E_ACCOUNT_KEYS: 'abc' }), /32-byte hex key/)
    assert.throws(() => parseWindowSize('big'), /<width>x<height>/)
    assert.throws(() => readTarget({ DCL_E2E_WORKER: '-1' }), /DCL_E2E_WORKER must be a non-negative integer/)
  })

  test('accountKey wraps the worker over the accounts and fails without any', () => {
    const target = readTarget({ DCL_E2E_ACCOUNT_KEYS: `${KEY_A},${KEY_B}`, DCL_E2E_WORKER: '3' })
    assert.equal(accountKey(target), KEY_B)
    assert.throws(() => accountKey(readTarget({})), /set DCL_E2E_ACCOUNT_KEYS/)
  })

  test('describeTarget never contains a key', () => {
    const text = describeTarget(readTarget({ DCL_E2E_ACCOUNT_KEYS: KEY_A }))
    assert.ok(!text.includes('aaaa'))
    assert.match(text, /accounts=1/)
  })
})
