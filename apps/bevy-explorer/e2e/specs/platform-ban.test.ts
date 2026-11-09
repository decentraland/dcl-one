// The platform-ban half of the ban & block status service: the engine's public read of
// `GET /users/<address>/bans` on the comms gatekeeper. Not an explorer-automation fixture;
// unity-explorer checks the same endpoint once at start-up (`ApplicationBlocklistGuard`) and
// shows a blocked screen, which is a consumer this service only feeds.
//
// | Scenario                                             | Class            |
// | the platform ban is checked after sign-in            | New              |
// | a refresh re-checks                                  | New              |
// | a banned answer is reported                          | New, native only |
// | a failing gatekeeper fails open                      | New, native only |
//
// Zone's live accounts cannot be banned by a test, so the last two scenarios start their own
// standalone pointed at a loopback gatekeeper stub (`--comms-gatekeeper`). On the web a service
// override stops the boot at the launch gate before the console is up: those two are todos there.
import assert from 'node:assert/strict'
import { before } from 'node:test'
import { bannedReply, readTarget, refusal, sharedGatekeeperStub, spec, until, type GatekeeperStub } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CHECK_TIMEOUT_MS = 60_000
const STUB_PORT = Number(process.env.DCL_E2E_GATEKEEPER_PORT ?? 18745)
const native = readTarget().transport === 'native'
const webOnly = { todo: 'web: a loopback service override stops at the launch gate' }
const stubSession = { session: { nativeArgs: ['--comms-gatekeeper', `http://127.0.0.1:${STUB_PORT}`] } }

let stub: GatekeeperStub | undefined
before(async () => {
  if (native) stub = await sharedGatekeeperStub(STUB_PORT)
})

scenario('the platform ban is checked after sign-in', async ({ explorer }) => {
  const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.state === 'checked', {
    timeoutMs: CHECK_TIMEOUT_MS,
    what: 'the platform ban to be checked'
  })
  assert.equal(state.platformBan.isBanned, false)
  assert.equal(state.platformBan.source, null)
  assert.ok(state.platformBan.checkedAgoSecs !== null && state.platformBan.checkedAgoSecs < CHECK_TIMEOUT_MS / 1000)
})

scenario('a refresh re-checks', async ({ explorer }) => {
  // the last check ages first, so the re-check is told apart from it
  const aged = await until(() => explorer.banBlockState(), (s) => s.platformBan.state === 'checked' && (s.platformBan.checkedAgoSecs ?? 0) >= 4, {
    timeoutMs: CHECK_TIMEOUT_MS,
    what: 'the platform ban check to be a few seconds old'
  })
  const before = aged.platformBan.checkedAgoSecs ?? 0
  await explorer.run('/ban_block_refresh')
  const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.checkedAgoSecs !== null && s.platformBan.checkedAgoSecs < before, {
    timeoutMs: 15_000,
    what: 'the platform ban to be re-checked'
  })
  assert.equal(state.platformBan.state, 'checked')
})

scenario(
  'a banned answer is reported',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.banStatus(bannedReply('e2e', 'a test ban'))
    await explorer.run('/ban_block_refresh')
    const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.isBanned === true, {
      timeoutMs: 15_000,
      what: 'the banned answer to be reported'
    })
    assert.equal(state.platformBan.reason, 'e2e')
    assert.equal(state.platformBan.customMessage, 'a test ban')
    assert.equal(state.platformBan.source, 'ban_status_endpoint')
  },
  native ? stubSession : webOnly
)

scenario(
  'a failing gatekeeper fails open',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.banStatus(refusal(500, 'stub failure'))
    await explorer.run('/ban_block_refresh')
    const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.state === 'failed', {
      timeoutMs: 15_000,
      what: 'the failed check to be reported'
    })
    assert.equal(state.platformBan.isBanned, null)
    assert.ok(state.unavailable['platformBan.isBanned']?.startsWith('check failed'))
  },
  native ? stubSession : webOnly
)
