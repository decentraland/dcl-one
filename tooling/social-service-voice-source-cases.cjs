#!/usr/bin/env node
// Run the pinned upstream handlers/parsers with injected component states. No sibling
// checkout is an application/build dependency; this tool only regenerates captures.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { createRequire } = require('node:module')
const revision = 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6'
const upstream = path.resolve(process.argv[2] || '')
const output = process.argv[3]
if (!process.argv[2] || !output) throw Error('Usage: node tooling/social-service-voice-source-cases.cjs <upstream-checkout> <output.json>')
const git = (...args) => execFileSync('git', ['-C', upstream, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== revision || git('status', '--porcelain', '--untracked-files=no')) throw Error('Expected clean pinned upstream')
const req = createRequire(path.join(upstream, 'package.json'))
req('ts-node').register({ transpileOnly: true, project: path.join(upstream, 'tsconfig.json') })
const load = file => req(path.join(upstream, 'src', file))
const proto = req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const errors = load('logic/community-voice/errors.ts')
const voiceErrors = load('logic/voice/errors.ts')
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const caller = '0x1111111111111111111111111111111111111111'
const target = '0x2222222222222222222222222222222222222222'
const community = '33333333-3333-4333-8333-333333333333'
const call = '44444444-4444-4444-8444-444444444444'
const cases = []
const url = address => `wss://livekit.test/voice-chat-private-${call}?for=${address}`
const communityUrl = `wss://livekit.test/voice-chat-community-${community}?for=${caller}`
const configurations = [
  ['start-community-voice-chat', 'startCommunityVoiceChatService', 'StartCommunityVoiceChatResponse', 'startCommunityVoiceChat'],
  ['join-community-voice-chat', 'joinCommunityVoiceChatService', 'JoinCommunityVoiceChatResponse', 'joinCommunityVoiceChat'],
  ['request-to-speak-in-community-voice-chat', 'requestToSpeakInCommunityVoiceChatService', 'RequestToSpeakInCommunityVoiceChatResponse', 'requestToSpeakInCommunityVoiceChat'],
  ['promote-speaker-in-community-voice-chat', 'promoteSpeakerInCommunityVoiceChatService', 'PromoteSpeakerInCommunityVoiceChatResponse', 'promoteSpeakerInCommunityVoiceChat'],
  ['demote-speaker-in-community-voice-chat', 'demoteSpeakerInCommunityVoiceChatService', 'DemoteSpeakerInCommunityVoiceChatResponse', 'demoteSpeakerInCommunityVoiceChat'],
  ['kick-player-from-community-voice-chat', 'kickPlayerFromCommunityVoiceChatService', 'KickPlayerFromCommunityVoiceChatResponse', 'kickUserFromCommunityVoiceChat'],
  ['reject-speak-request-in-community-voice-chat', 'rejectSpeakRequestInCommunityVoiceChatService', 'RejectSpeakRequestInCommunityVoiceChatResponse', 'rejectSpeakRequestInCommunityVoiceChat'],
  ['end-community-voice-chat', 'endCommunityVoiceChatService', 'EndCommunityVoiceChatResponse', 'endCommunityVoiceChat'],
  ['mute-speaker-from-community-voice-chat', 'muteSpeakerFromCommunityVoiceChatService', 'MuteSpeakerFromCommunityVoiceChatResponse', 'muteSpeakerInCommunityVoiceChat']
]
async function capture(config, scenario, request, components, address = caller) {
  const [file, factory, responseType] = config
  const fn = load(`controllers/handlers/rpc/${file}.ts`)[factory]({ components: { logs, ...components } })
  const response = await fn(request, { address })
  cases.push({ group: 'rpc', id: `${file}.${scenario}`, method: factory.replace(/Service$/, ''), scenario, caller: address, request, response_type: responseType, response,
    expected_hex: Buffer.from(proto[responseType].encode(response).finish()).toString('hex') })
}
async function components(scenario, starting = false) {
  const failure = () => { throw Error('voice runtime not configured (no LiveKit adapter attached)') }
  const missing = () => { throw new errors.CommunityVoiceChatResourceNotFoundError(community) }
  const act = scenario === 'runtime-failure' ? failure : scenario === 'runtime-not-found' ? missing : async () => {}
  const communitiesDb = {
    getCommunity: async () => ({ id: community, name: 'Community', privacy: 'public', visibility: 'all', role: 'owner' }),
    getCommunityMemberRole: async () => scenario === 'forbidden' ? 'none' : 'owner',
    isMemberBanned: async () => false,
    getCommunityMemberRoles: async () => ({ [caller]: scenario === 'forbidden' ? 'none' : 'owner', [target]: 'member' }),
    getBannedMemberAddresses: async () => [], getCommunityPlaces: async () => []
  }
  if (scenario === 'database-failure') {
    for (const method of Object.keys(communitiesDb)) communitiesDb[method] = async () => { throw Error('pool timed out while waiting for an open connection') }
  }
  const commsGatekeeper = {
    getCommunityVoiceChatStatus: scenario === 'runtime-failure' ? failure : async () => ({ isActive: scenario === 'conflict' || !starting }),
    createCommunityVoiceChatRoom: async () => ({ connectionUrl: communityUrl }),
    getCommunityVoiceChatCredentials: scenario === 'runtime-not-found' ? missing : async () => ({ connectionUrl: communityUrl }),
    requestToSpeakInCommunityVoiceChat: act, endCommunityVoiceChatRoom: act, muteSpeakerInCommunityVoiceChat: act,
    promoteSpeakerInCommunityVoiceChat: act, demoteSpeakerInCommunityVoiceChat: act,
    kickUserFromCommunityVoiceChat: act, rejectSpeakRequestInCommunityVoiceChat: act
  }
  if (scenario === 'noncanonical-community' || scenario === 'invalid-target') {
    const privileged = await load('adapters/comms-gatekeeper.ts').createCommsGatekeeperComponent({ logs,
      config: { requireString: async () => 'https://gatekeeper.example' },
      fetcher: { fetch: async () => { throw Error('identifier guard must run before fetch') } } })
    Object.assign(commsGatekeeper, privileged)
  }
  const communityVoice = await load('logic/community-voice/community-voice.ts').createCommunityVoiceComponent({
    logs, communitiesDb, commsGatekeeper, pubsub: { publishInChannel: async () => {} }, analytics: { fireEvent() {} },
    registry: { getProfile: async () => { throw Error('profile unavailable') } }, communityThumbnail: { getThumbnail: async () => null },
    communityPlaces: {}, placesApi: {}, communityBroadcaster: { broadcast: async () => {} },
    communityVoiceChatCache: { setCommunityVoiceChat: async () => {}, getCommunityVoiceChat: async () => ({ notificationScope: 'all' }), removeCommunityVoiceChat: async () => {} }
  })
  return { communityVoice, communitiesDb, commsGatekeeper }

}
async function main() {
  const base = { communityId: community, userAddress: target, isRaisingHand: true, muted: true }
  for (const config of configurations) {
    await capture(config, 'success', base, await components('success', config[0].startsWith('start-')))
    await capture(config, 'empty-community', { ...base, communityId: ' ' }, await components('success', config[0].startsWith('start-')))
    await capture(config, 'runtime-failure', base, await components('runtime-failure', config[0].startsWith('start-')))
    await capture(config, 'database-failure', base, await components('database-failure', config[0].startsWith('start-')))
    await capture(config, 'noncanonical-community', { ...base, communityId: community.replaceAll('-', '') }, await components('noncanonical-community', config[0].startsWith('start-')))
    if (['promote-', 'demote-', 'kick-', 'reject-speak-', 'mute-'].some(prefix => config[0].startsWith(prefix))) {
      await capture(config, 'empty-target', { ...base, userAddress: ' ' }, await components('success', config[0].startsWith('start-')))
      await capture(config, 'invalid-target', { ...base, userAddress: 'invalid' }, await components('invalid-target', config[0].startsWith('start-')))
    }
    if (['promote-', 'demote-', 'kick-', 'reject-speak-'].some(prefix => config[0].startsWith(prefix))) {
      await capture(config, 'forbidden', base, await components('forbidden'))
    }
    if (!config[0].startsWith('start-')) await capture(config, 'runtime-not-found', base, await components('runtime-not-found'))
  }
  await capture(configurations[0], 'conflict', base, await components('conflict', true))
  await capture(configurations[2], 'lower-hand', { ...base, isRaisingHand: false }, await components('success'))
  const selfDemote = await components('success'); selfDemote.communitiesDb.getCommunity = async () => ({ privacy: 'public', role: 'member' })
  await capture(configurations[4], 'self-demote', { ...base, userAddress: caller }, selfDemote)
  const privateConfigs = [
    ['start-private-voice-chat', 'startPrivateVoiceChatService', 'StartPrivateVoiceChatResponse', 'startPrivateVoiceChat'],
    ['accept-private-voice-chat', 'acceptPrivateVoiceChatService', 'AcceptPrivateVoiceChatResponse', 'acceptPrivateVoiceChat'],
    ['reject-private-voice-chat', 'rejectPrivateVoiceChatService', 'RejectPrivateVoiceChatResponse', 'rejectPrivateVoiceChat'],
    ['end-private-voice-chat', 'endPrivateVoiceChatService', 'EndPrivateVoiceChatResponse', 'endPrivateVoiceChat'],
    ['get-incoming-private-voice-chat-requests', 'getIncomingPrivateVoiceChatRequestsService', 'GetIncomingPrivateVoiceChatRequestResponse', 'getIncomingPrivateVoiceChat']
  ]
  for (const config of privateConfigs) {
    const method = config[3]
    const success = method === 'startPrivateVoiceChat' ? call : method === 'acceptPrivateVoiceChat' ? { connectionUrl: url(target) } : method === 'getIncomingPrivateVoiceChat' ? { id: call, caller_address: target } : undefined
    await capture(config, 'success', { callee: { address: target }, callId: call }, { voice: { [method]: async () => success } }, method === 'acceptPrivateVoiceChat' ? target : caller)
    const error = method === 'startPrivateVoiceChat' ? new voiceErrors.VoiceChatNotAllowedError() : method === 'getIncomingPrivateVoiceChat' ? new voiceErrors.IncomingVoiceChatNotFoundError(caller) : new voiceErrors.VoiceChatNotFoundError(call)
    await capture(config, 'refused', { callee: { address: caller }, callId: call }, { voice: { [method]: async () => { throw error } } })
    await capture(config, 'database-failure', { callee: { address: target }, callId: call }, { voice: { [method]: async () => { throw Error('pool timed out while waiting for an open connection') } } })
  }
  const registry = load('controllers/handlers/uws/stream-registry.ts')
  const profile = { avatars: [{ userId: target, name: 'Stream Person', hasClaimedName: true, avatar: { snapshots: { face256: 'https://profiles.example/face.png' } } }] }
  const streamConfigs = [
    ['friendship', 'subscribe-to-friendship-updates', 'subscribeToFriendshipUpdatesService', 'FriendshipUpdate', { action: 'request', id: call, from: target, to: caller, timestamp: 1234, metadata: { message: 'Hello' } }, registry.toFriendshipStreamEvent],
    ['friend-connectivity', 'subscribe-to-friend-connectivity-updates', 'subscribeToFriendConnectivityUpdatesService', 'FriendConnectivityUpdate', { address: target, status: proto.ConnectivityStatus.ONLINE }, registry.toFriendConnectivityStreamEvent],
    ['block', 'subscribe-to-block-updates', 'subscribeToBlockUpdatesService', 'BlockUpdate', { blockerAddress: target, blockedAddress: caller, isBlocked: true }, registry.toBlockStreamEvent],
    ['private-voice', 'subscribe-to-private-voice-chat-updates', 'subscribeToPrivateVoiceChatUpdatesService', 'PrivateVoiceChatUpdate', { callId: call, callerAddress: target, calleeAddress: caller, status: 'requested' }, registry.toPrivateVoiceChatStreamEvent],
    ['community-member', 'subscribe-to-community-member-connectivity-updates', 'subscribeToCommunityMemberConnectivityUpdatesService', 'CommunityMemberConnectivityUpdate', { communityId: community, memberAddress: target, status: proto.ConnectivityStatus.OFFLINE }, registry.toCommunityMemberConnectivityStreamEvent],
    ['community-voice', 'subscribe-to-community-voice-chat-updates', 'subscribeToCommunityVoiceChatUpdatesService', 'CommunityVoiceChatUpdate', { communityId: community, createdAt: 1234, status: proto.CommunityVoiceChatStatus.COMMUNITY_VOICE_CHAT_STARTED, positions: ['1,2'], worlds: ['example.dcl.eth'], isMember: true, communityName: 'Community', communityImage: 'https://images.example/community.png' }, registry.toCommunityVoiceChatStreamEvent]
  ]
  for (const [id, file, factory, responseType, update, toJSON] of streamConfigs) {
    let options
    const updateHandler = { async *handleSubscriptionUpdates(value) { options = value; const parsed = await value.parser(update, profile); if (parsed) yield parsed } }
    const generator = load(`controllers/handlers/rpc/${file}.ts`)[factory]({ components: { logs, updateHandler, friendsDb: {}, registry: {}, peersStats: {} } })({}, { address: caller })
    const result = await generator.next()
    const response = proto[responseType].fromPartial(result.value)
    cases.push({ group: 'stream', id, response_type: responseType, update, response, expected_hex: Buffer.from(proto[responseType].encode(response).finish()).toString('hex'), json: toJSON(update) })
    const closed = options.buildStreamClosedUpdate({ reason: proto.SubscriptionStreamClosedReason.STREAM_CLOSED_DUPLICATE_SUBSCRIPTION, message: 'duplicate' })
    cases.push({ group: 'stream-closed', id, response_type: responseType, response: closed, expected_hex: Buffer.from(proto[responseType].encode(closed).finish()).toString('hex') })
    if (id === 'friendship') {
      // Execute the real lifecycle: a registry failure skips that queued update;
      // a following address-only cancellation still reaches the same subscription.
      const emitter = req('mitt').default ? req('mitt').default() : req('mitt')()
      const subscribersContext = { hasActiveSubscription: () => false, getSubscriber: () => emitter,
        setActiveSubscription() {}, clearActiveSubscription() {}, registerGenerator() {}, unregisterGenerator() {} }
      let reads = 0
      const actual = load('logic/updates.ts').createUpdateHandlerComponent({ logs, subscribersContext,
        friendsDb: {}, communitiesDb: {}, communityMembers: {}, peersStats: {}, metrics: { increment() {} },
        registry: { getProfile: async () => { if (reads++ === 0) throw Error('Profile not found'); return profile } } })
      const stream = actual.handleSubscriptionUpdates({ ...options, rpcContext: { address: caller, wsConnectionId: 'capture', subscribersContext } })
      const next = stream.next()
      await new Promise(resolve => setImmediate(resolve))
      emitter.emit('friendshipUpdate', update)
      await new Promise(resolve => setImmediate(resolve))
      emitter.emit('friendshipUpdate', { ...update, action: 'cancel' })
      const delivered = await next
      await stream.return()
      const parsed = proto.FriendshipUpdate.fromPartial(delivered.value)
      cases.push({ group: 'stream-missing-profile', id, rpc_next_update: parsed, expected_hex: Buffer.from(proto.FriendshipUpdate.encode(parsed).finish()).toString('hex'), json: toJSON(update) })
    }
  }
  fs.writeFileSync(output, JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision, yarn_lock_sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream, 'yarn.lock'))).digest('hex') },
    runtime: { node: process.version, protocol_package: req('@dcl/protocol/package.json').version }, scope: 'Executed real upstream RPC handlers, community voice/moderation components, stream parsers and missing-profile subscription lifecycle with deterministic injected dependencies; private RPC domain outcomes are injected. Not deployed PostgreSQL, Redis or LiveKit equivalence.', caller, target, community, call, cases }, null, 2) + '\n')
  process.stdout.write(`Captured ${cases.length} upstream voice/stream cases\n`)
}
main().catch(error => { console.error(error); process.exitCode = 1 })
