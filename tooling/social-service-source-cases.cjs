#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/source-cases.json
// Executes the pinned upstream handlers with deterministic component inputs.
// No upstream checkout or package is a build dependency of dcl-one.
const { requireNode26, sourceCapture, sha256 } = require('./social-capture-common.cjs')
requireNode26()
const fs = require('node:fs')

const output = process.argv[3]
if (!process.argv[2] || !output) throw Error('Usage: node tooling/social-service-source-cases.cjs <upstream-checkout> <output.json>')
const { upstream, revision: baseline, req: requireUpstream, load } = sourceCapture(process.argv[2])
const proto = requireUpstream('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const caller = '0x1111111111111111111111111111111111111111'
const target = '0x2222222222222222222222222222222222222222'
const cases = []
async function capture(id, file, factory, responseType, request, friends, extra = {}) {
  const handler = load(`controllers/handlers/rpc/${file}.ts`)[factory]({ components: { logs, friends, ...extra } })
  const response = await handler(request, { address: caller })
  cases.push({ id, method: factory.replace(/Service$/, ''), request, response, expected_hex: Buffer.from(proto[responseType].encode(response).finish()).toString('hex') })
}

async function main() {
  for (const [file, factory, responseType] of [
    ['block-user', 'blockUserService', 'BlockUserResponse'],
    ['unblock-user', 'unblockUserService', 'UnblockUserResponse']
  ]) {
    for (const [suffix, request] of [['missing', {}], ['invalid', { user: { address: 'invalid' } }], ['self', { user: { address: caller } }]]) {
      await capture(`${file}.${suffix}`, file, factory, responseType, request, {})
    }
  }
  for (const [suffix, request] of [
    ['missing-action', {}], ['missing-user', { action: { $case: 'request', request: {} } }], ['invalid', { action: { $case: 'request', request: { user: { address: 'invalid' } } } }],
    ['whitespace', { action: { $case: 'request', request: { user: { address: ` ${target}` } } } }],
    ['self', { action: { $case: 'request', request: { user: { address: caller } } } }]
  ]) await capture(`upsert-friendship.${suffix}`, 'upsert-friendship', 'upsertFriendshipService', 'UpsertFriendshipResponse', request, {})

  for (const [suffix, request] of [['missing', {}], ['invalid', { user: { address: 'invalid' } }], ['no-action', { user: { address: target } }]]) {
    await capture(`get-friendship-status.${suffix}`, 'get-friendship-status', 'getFriendshipStatusService', 'GetFriendshipStatusResponse', request, { getFriendshipStatus: async () => undefined })
  }
  await capture('get-friendship-status.synthesized-block-action', 'get-friendship-status', 'getFriendshipStatusService', 'GetFriendshipStatusResponse', { user: { address: target } }, { getFriendshipStatus: async () => ({ action: 'block', acting_user: caller }) })
  const friends = {
    getFriendsProfiles: async () => ({ friendsProfiles: [], total: 1 }),
    getMutualFriendsProfiles: async () => ({ friendsProfiles: [], total: 1 }),
    getPendingFriendshipRequests: async () => ({ requests: [{ id: 'request', address: target, timestamp: new Date(0) }], profiles: [], total: 1 }),
    getSentFriendshipRequests: async () => ({ requests: [{ id: 'request', address: target, timestamp: new Date(0) }], profiles: [], total: 1 }),
    getBlockedUsers: async () => ({ blockedUsers: [{ address: target, blocked_at: new Date(0) }], blockedProfiles: [], total: 1 })
  }
  for (const [file, factory, responseType] of [
    ['get-friends', 'getFriendsService', 'PaginatedFriendsProfilesResponse'],
    ['get-mutual-friends', 'getMutualFriendsService', 'PaginatedFriendsProfilesResponse'],
    ['get-pending-friendship-requests', 'getPendingFriendshipRequestsService', 'PaginatedFriendshipRequestsResponse'],
    ['get-sent-friendship-requests', 'getSentFriendshipRequestsService', 'PaginatedFriendshipRequestsResponse'],
    ['get-blocked-users', 'getBlockedUsersService', 'GetBlockedUsersResponse']
  ]) await capture(`${file}.missing-profile`, file, factory, responseType, { user: { address: target }, pagination: { limit: 1, offset: 0 } }, friends)

  // Execute the real friends component: missing profiles fail AFTER its transaction.
  const effects = []
  const friendsDb = {
    isFriendshipBlocked: async () => false,
    getLastFriendshipActionByUsers: async () => undefined,
    executeTx: async fn => fn({}),
    createFriendship: async () => { effects.push('friendship-written'); return { id: 'friendship', created_at: new Date(0) } },
    recordFriendshipAction: async () => { effects.push('action-written'); return 'action' }
  }
  const realFriends = await load('logic/friends/component.ts').createFriendsComponent({
    logs, friendsDb, registry: { getProfiles: async () => [] }, config: { getNumber: async () => undefined },
    redis: { consumeRateLimit: async () => true }, metrics: { increment() {} }, sns: {},
    pubsub: { publishInChannel: async () => { effects.push('update-published') } }
  })
  await capture('upsert-friendship.missing-profiles-after-commit', 'upsert-friendship', 'upsertFriendshipService', 'UpsertFriendshipResponse', { action: { $case: 'request', request: { user: { address: target } } } }, realFriends)
  cases.at(-1).effects = effects

  const settings = load('logic/settings/settings.ts').createSettingsComponent({
    friendsDb: { getSocialSettings: async () => [], getFriendsFromList: async () => [] }, commsGatekeeper: {}
  })
  await capture('get-social-settings.absent', 'get-social-settings', 'getSocialSettingsService', 'GetSocialSettingsResponse', {}, {}, { settings })
  await capture('get-private-messages-settings.duplicates', 'get-private-messages-settings', 'getPrivateMessagesSettingsService', 'GetPrivateMessagesSettingsResponse', { user: [{ address: target }, { address: target }] }, {}, { settings })
  for (const [suffix, request] of [['empty', {}], ['privacy', { privateMessagesPrivacy: 99 }], ['blocked-visibility', { blockedUsersMessagesVisibility: 99 }], ['reactions', { showSituationReactions: 99 }]]) {
    await capture(`upsert-social-settings.${suffix}`, 'upsert-social-settings', 'upsertSocialSettingsService', 'UpsertSocialSettingsResponse', request, {}, { settings })
  }

  const fixture = { baseline: { repository: 'decentraland/social-service-ea', revision: baseline, yarn_lock_sha256: sha256(upstream, 'yarn.lock') }, runtime: { node: process.version, protocol_package: requireUpstream('@dcl/protocol/package.json').version }, scope: 'Real upstream RPC handlers and friendship/settings components with deterministic injected dependencies; not deployed HTTP, Redis, SNS, or PostgreSQL equivalence.', caller, target, cases }
  fs.writeFileSync(output, JSON.stringify(fixture, null, 2) + '\n')
  process.stdout.write(`Captured ${cases.length} upstream cases\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
