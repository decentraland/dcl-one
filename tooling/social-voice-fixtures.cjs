// Shared deterministic voice dependencies for capture scripts.
// The caller validates/registers its source before supplying load.
function createCommunityVoiceFixtures({ load, logs }) {
  const caller = '0x1111111111111111111111111111111111111111'
  const target = '0x2222222222222222222222222222222222222222'
  const community = '33333333-3333-4333-8333-333333333333'
  const communityUrl = `wss://livekit.test/voice-chat-community-${community}?for=${caller}`
  return async function components(scenario, starting = false) {
    const failure = () => { throw Error('voice runtime not configured (no LiveKit adapter attached)') }
    const missing = () => {
      const { CommunityVoiceChatResourceNotFoundError } = load('logic/community-voice/errors.ts')
      throw new CommunityVoiceChatResourceNotFoundError(community)
    }
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
}

module.exports = { createCommunityVoiceFixtures }
