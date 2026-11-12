// The ban & block status service (`/ban_block_state`, the engine's one answer to "is this user
// banned or blocked?"), not an explorer-automation fixture: unity-explorer keeps blocks in
// `UserBlockingCache` and each ban in its own place, with no console view of either. The
// scenarios read the engine's state only; no HUD control is involved.
//
// | Scenario                                              | Class |
// | the block state goes live once signed in              | New   |
// | blocking an address shows in the state and the lookup | New   |
// | the debug panel lists the section                     | New   |
//
// The second scenario blocks a real address through zone's social service and lifts the block
// afterwards, whatever the outcome. It needs a second account in `DCL_E2E_ACCOUNT_KEYS` to have
// an address that exists; with one key it is a todo.
import assert from 'node:assert/strict'
import { addressOf, readTarget, spec, until } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const STATE_TIMEOUT_MS = 60_000
const UPDATE_TIMEOUT_MS = 15_000

/** The address of another configured account, if any: the only address known to exist. */
function otherAccountAddress(): string | undefined {
  const target = readTarget()
  const other = target.accounts.find((_, index) => index !== target.worker % Math.max(target.accounts.length, 1))
  return other ? addressOf(other).toLowerCase() : undefined
}

scenario('the block state goes live once signed in', async ({ explorer }) => {
  const state = await until(() => explorer.banBlockState(), (s) => s.blocks.state === 'live', {
    timeoutMs: STATE_TIMEOUT_MS,
    what: 'the social blocks to be live'
  })
  const { address } = await explorer.worldState()
  assert.ok(address, 'the player should be signed in')
  assert.equal(state.address, address.toLowerCase())
  assert.equal(state.blocks.reason, null)
  assert.equal(state.lookup, null)
  assert.equal(state.unavailable.lookup, 'no address given')
})

const target = otherAccountAddress()
scenario(
  'blocking an address shows in the state and the lookup, and is lifted again',
  async ({ explorer, restore }) => {
    assert.ok(target)
    await until(() => explorer.banBlockState(), (s) => s.blocks.state === 'live', { timeoutMs: STATE_TIMEOUT_MS, what: 'the social blocks to be live' })
    restore(() => explorer.unblockUser(target))
    await explorer.blockUser(target)
    const blocked = await until(() => explorer.banBlockState(target), (s) => s.blocks.blocked.includes(target), {
      timeoutMs: UPDATE_TIMEOUT_MS,
      what: 'the blocked address to appear in the state'
    })
    assert.equal(blocked.lookup?.isBlockedEitherWay, true)
    assert.equal(blocked.lookup?.direction, 'local_user_blocked_them')
    await explorer.unblockUser(target)
    const lifted = await until(() => explorer.banBlockState(target), (s) => !s.blocks.blocked.includes(target), {
      timeoutMs: UPDATE_TIMEOUT_MS,
      what: 'the blocked address to leave the state'
    })
    assert.equal(lifted.lookup?.direction, 'none')
  },
  target ? {} : { todo: 'needs a second account in DCL_E2E_ACCOUNT_KEYS for an address that exists' }
)

scenario('the debug panel lists the section', async ({ explorer }) => {
  const sections = await explorer.json<{ title: string; query: string }[]>('/debug_panel')
  const section = sections.find((s) => s.title === 'Bans and blocks')
  assert.ok(section, 'the "Bans and blocks" section should be registered')
  assert.equal(section.query, '/ban_block_state')
})
