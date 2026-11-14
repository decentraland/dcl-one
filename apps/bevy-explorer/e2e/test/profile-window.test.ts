import assert from 'node:assert/strict'
import { setImmediate as flush } from 'node:timers/promises'
import { test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer, PROFILE_DEPLOY_WINDOW_MS } from '../driver/explorer.ts'

const user = (version: number) => `tester (0x${'a'.repeat(40)}): v${version}, web3=true`

test('a slow deployment must settle before the next write window starts', async (t) => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 10_000 })
  let version = 3
  let pending = false
  const fake = new FakeConsole({
    get_user_data: () => user(version),
    profile_requests: () => JSON.stringify({ ownVersion: version, peerVersion: pending ? 3 : version, deployPending: pending })
  })
  const explorer = new Explorer(fake)
  await explorer.userData()
  version = 4
  pending = true
  await explorer.userData()
  // The local version changed more than a window ago, but its HTTP deployment is still running.
  t.mock.timers.tick(PROFILE_DEPLOY_WINDOW_MS + 2_000)
  let ready = false
  const waiting = explorer.profileDeployWindow().then(() => { ready = true })
  await flush()
  assert.equal(ready, false, 'an expired local-version clock is not server completion')
  t.mock.timers.tick(5_000)
  await flush()
  assert.equal(ready, false, 'a slow deployment must still block another write')

  pending = false
  t.mock.timers.tick(250)
  await flush()
  t.mock.timers.tick(PROFILE_DEPLOY_WINDOW_MS - 1)
  await flush()
  assert.equal(ready, false, 'the complete window starts after server confirmation')
  t.mock.timers.tick(1)
  await waiting
  assert.equal(ready, true)
  const finishedAt = Date.now()
  await explorer.profileDeployWindow()
  assert.equal(Date.now(), finishedAt, 'the confirmed version spends its window only once')
})

test('a pending deployment found without an earlier version read still waits after confirmation', async (t) => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 10_000 })
  let pending = true
  const explorer = new Explorer(new FakeConsole({
    get_user_data: user(4),
    profile_requests: () => JSON.stringify({ ownVersion: 4, peerVersion: pending ? 3 : 4, deployPending: pending })
  }))
  let ready = false
  const waiting = explorer.profileDeployWindow().then(() => { ready = true })
  await flush()
  assert.equal(ready, false)
  pending = false
  t.mock.timers.tick(250)
  await flush()
  assert.equal(ready, false)
  t.mock.timers.tick(PROFILE_DEPLOY_WINDOW_MS)
  await waiting
})

test('a failed deployment that restores an older version refuses another profile write', async () => {
  let version = 3
  const explorer = new Explorer(new FakeConsole({
    get_user_data: () => user(version),
    profile_requests: () => JSON.stringify({ ownVersion: version, peerVersion: version, deployPending: false })
  }), { profileDeployWindowMs: 0 })
  await explorer.userData()
  version = 4
  await explorer.userData()
  version = 3
  await explorer.userData()
  await assert.rejects(explorer.profileDeployWindow(), /rolled back from v4 to v3; refusing another profile write/)
})
