// The platform-ban half of the ban & block status service: the engine's public read of
// `GET /users/<address>/bans` on the comms gatekeeper. Not an explorer-automation fixture;
// unity-explorer checks the same endpoint once at start-up (`ApplicationBlocklistGuard`).
//
// | Scenario                                             | Class            |
// | the platform ban is checked after sign-in            | New              |
// | a refresh re-checks                                  | New              |
// | a banned answer is reported                          | New, native only |
// | a failing gatekeeper fails open                      | New, native only |
// | not banned: the realm is entered                     | New              |
// | banned: a realm change is refused                    | New, native only |
// | a failed check: a realm change enters                | New, native only |
// | banned at sign-in: known before any realm change     | New, native only |
//
// Asserts every realm change is refused while a ban is known.
//
// Zone's live accounts cannot be banned by a test, so the stub scenarios start their own
// standalone pointed at a loopback gatekeeper stub (`--comms-gatekeeper`). On the web a service
// override stops the boot at the launch gate before the console is up: those two are todos there.
import assert from 'node:assert/strict'
import { before } from 'node:test'
import { bannedReply, NOT_BANNED, readTarget, refusal, sharedGatekeeperStub, spec, until, type BanBlockState, type Explorer, type GatekeeperStub } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const CHECK_TIMEOUT_MS = 60_000
const STUB_PORT = Number(process.env.DCL_E2E_GATEKEEPER_PORT ?? 18745)
const target = readTarget()
const native = target.transport === 'native'
const BANNED_REFUSAL = /You are banned from Decentraland/
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
  // The session is shared with earlier specs, so its sign-in check may be minutes old.
  // Freshness after an explicit refresh is checked by the next scenario.
  const age = state.platformBan.checkedAgoSecs
  assert.ok(age !== null && Number.isFinite(age) && age >= 0, 'a completed check reports its elapsed age')
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
    try {
      await explorer.run('/ban_block_refresh')
      const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.isBanned === true, {
        timeoutMs: 15_000,
        what: 'the banned answer to be reported'
      })
      assert.equal(state.platformBan.reason, 'e2e')
      assert.equal(state.platformBan.customMessage, 'a test ban')
      assert.equal(state.platformBan.source, 'ban_status_endpoint')
    } finally {
      // a known ban survives a failed check: the next scenario starts from not banned
      stub.banStatus(NOT_BANNED)
      await stateAfterRefresh(explorer, (s) => s.platformBan.isBanned === false, 'the ban to be lifted')
    }
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

/** Waits for a fresh check's answer to satisfy `ready`. */
async function stateAfterRefresh(explorer: Explorer, ready: (s: BanBlockState) => boolean, what: string): Promise<BanBlockState> {
  await explorer.run('/ban_block_refresh')
  return until(() => explorer.banBlockState(), ready, { timeoutMs: 15_000, what })
}

scenario('not banned: the realm is entered', async ({ explorer }) => {
  const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.state === 'checked', {
    timeoutMs: CHECK_TIMEOUT_MS,
    what: 'the platform ban to be checked'
  })
  assert.equal(state.platformBan.isBanned, false)
  assert.equal((await explorer.worldState()).inWorld, true)
})

scenario(
  'banned: a realm change is refused',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.banStatus(bannedReply('e2e', 'a test ban'))
    try {
      await stateAfterRefresh(explorer, (s) => s.platformBan.isBanned === true, 'the ban to be known')
      const before = await explorer.worldState()
      await assert.rejects(explorer.run(`/changerealm ${target.realm}`), BANNED_REFUSAL)
      const after = await explorer.worldState()
      assert.equal(after.realm, before.realm)
      assert.equal(after.arrivals, before.arrivals)
    } finally {
      stub.banStatus(NOT_BANNED)
      await stateAfterRefresh(explorer, (s) => s.platformBan.isBanned === false, 'the ban to be lifted')
    }
  },
  native ? stubSession : webOnly
)

scenario(
  'a failed check: a realm change enters',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.banStatus(refusal(500, 'stub failure'))
    try {
      await stateAfterRefresh(explorer, (s) => s.platformBan.state === 'failed', 'the check to fail')
      const state = await explorer.changeRealm(target.realm)
      assert.equal(state.inWorld, true)
    } finally {
      stub.banStatus(NOT_BANNED)
    }
  },
  native ? stubSession : webOnly
)

scenario(
  'banned at sign-in: known before any realm change',
  async ({ explorer }) => {
    assert.ok(stub)
    stub.banStatus(bannedReply('e2e'))
    try {
      // the new wallet's ban is known before any realm change is asked for
      await explorer.logout()
      await explorer.loginGuest()
      const state = await until(() => explorer.banBlockState(), (s) => s.platformBan.isBanned === true, {
        timeoutMs: 15_000,
        what: 'the ban to be known for the new sign-in'
      })
      assert.equal(state.platformBan.source, 'ban_status_endpoint')
      await assert.rejects(explorer.run(`/changerealm ${target.realm}`), BANNED_REFUSAL)
    } finally {
      stub.banStatus(NOT_BANNED)
    }
  },
  // the session is left signed in as a throwaway guest
  native ? { ...stubSession, closeSession: true } : webOnly
)
