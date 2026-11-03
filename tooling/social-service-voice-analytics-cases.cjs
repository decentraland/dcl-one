#!/usr/bin/env node
// Executes source voice mutations + analytics SDK; PostgreSQL is owned by this
// capture. Media dependencies and analytics fetch are deterministic local stubs.
const fs = require('node:fs')
const { randomUUID } = require('node:crypto')
const { req, load, logs, caller, target, revision, components } = require('./social-service-voice-source-cases.cjs')
const { Pool } = req('pg')
const { createAnalyticsComponent } = req('@dcl/analytics-component')
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const community = 'a3333333-b333-4c33-8d33-e33333333333'
const call = 'c6666666-6666-4666-8666-666666666666'
async function main() {
  if (!process.env.CATALYST_TEST_DATABASE_URL) throw Error('CATALYST_TEST_DATABASE_URL required; no silent capture skip')
  const url = new URL(process.env.CATALYST_TEST_DATABASE_URL)
  const name = `social_analytics_voice_${process.pid}_${randomUUID().replaceAll('-', '')}`
  const admin = new Pool({ connectionString: url.toString(), max: 1 })
  let pg
  try {
    await admin.query(`CREATE DATABASE "${name}"`)
    url.pathname = '/' + name
    pg = new Pool({ connectionString: url.toString(), max: 2 })
    await pg.query('CREATE TABLE private_voice_chats(id UUID PRIMARY KEY,caller_address TEXT,callee_address TEXT,created_at TIMESTAMP DEFAULT now())')
    pg.exists = async (query, field) => (await pg.query(query)).rows[0][field]
    pg.withTransaction = async callback => {
      const client = await pg.connect()
      try { await client.query('BEGIN'); const result = await callback(client); await client.query('COMMIT'); return result }
      catch (error) { await client.query('ROLLBACK'); throw error }
      finally { client.release() }
    }
    const cases = []
    for (const id of ['start', 'self-start', 'start-expired-pending', 'accept', 'accept-missing-caller', 'accept-missing-callee', 'reject', 'end-pending-caller', 'end-pending-callee', 'end-media-caller', 'end-media-callee', 'end-not-found', 'expire', 'disconnect-pending-caller', 'disconnect-pending-callee', 'disconnect-accepted']) {
      await pg.query('TRUNCATE private_voice_chats')
      const steps = [], payloads = []
      const analytics = await createAnalyticsComponent({ logs, config: createRecordConfigComponent({ ANALYTICS_CONTEXT: 'social-fixture', ANALYTICS_API_URL: 'https://analytics.example/events', ANALYTICS_API_TOKEN: 'fixture-token', ENV: 'fixture' }),
        fetcher: { fetch: async (_url, options) => { const body = JSON.parse(options.body); payloads.push(body); steps.push(`analytics:${body.event}`); return new Response(null) } } })
      const config = { requireNumber: async key => key === 'PRIVATE_VOICE_CHAT_EXPIRATION_TIME' ? 1000 : 8 }
      const voiceDb = await load('adapters/voice-db.ts').createVoiceDBComponent({ pg, config })
      const voice = await load('logic/voice/voice.ts').createVoiceComponent({ logs, config, voiceDb, analytics,
        friendsDb: { isFriendshipBlocked: async () => false }, settings: { getUsersSettings: async () => [{ private_messages_privacy: 'all' }, { private_messages_privacy: 'all' }] },
        pubsub: { publishInChannel: async (_channel, update) => steps.push(`publish:${update.status}`) },
        commsGatekeeper: { isUserInCommunityVoiceChat: async () => false, isUserInAVoiceChat: async () => false,
          getPrivateVoiceChatCredentials: async () => { steps.push('media:credentials'); const grants = {}; if (id !== 'accept-missing-caller') grants[caller] = { connectionUrl: 'caller-grant' }; if (id !== 'accept-missing-callee') grants[target] = { connectionUrl: 'callee-grant' }; return grants },
          endPrivateVoiceChat: async () => { steps.push('media:end'); return id === 'end-not-found' ? [] : [caller, target] } } })
      const pending = ['start-expired-pending', 'accept', 'accept-missing-caller', 'accept-missing-callee', 'reject', 'end-pending-caller', 'end-pending-callee', 'expire', 'disconnect-pending-caller', 'disconnect-pending-callee'].includes(id)
      if (pending) await pg.query('INSERT INTO private_voice_chats(id,caller_address,callee_address,created_at) VALUES($1,$2,$3,now()-($4::int*interval \'1 second\'))', [call, caller, target, id === 'expire' || id === 'start-expired-pending' ? 60 : 0])
      let generated, error = null
      try {
        if (id.startsWith('start') || id === 'self-start') generated = await voice.startPrivateVoiceChat(caller, id === 'self-start' ? caller : target)
        else if (id.startsWith('accept')) await voice.acceptPrivateVoiceChat(call, target)
        else if (id === 'reject') await voice.rejectPrivateVoiceChat(call, target)
        else if (id.startsWith('end')) await voice.endPrivateVoiceChat(call, id.endsWith('callee') ? target : caller)
        else if (id === 'expire') await voice.expirePrivateVoiceChat()
        else await voice.endIncomingOrOutgoingPrivateVoiceChatForUser(id.endsWith('callee') ? target : caller)
      } catch (caught) { error = caught.message }
      await new Promise(setImmediate)
      let entry = { id: `private.${id}`, kind: 'private', scenario: id, error, payloads, steps,
        remaining_rows: (await pg.query('SELECT count(*)::int AS n FROM private_voice_chats')).rows[0].n }
      if (id === 'start-expired-pending') entry.lead_only = 'Source busy until expiry sweep; target opportunistic reclaim is a separate pending fix.'
      if (generated) entry = JSON.parse(JSON.stringify(entry).replaceAll(generated, call))
      cases.push(entry)
    }
    for (const id of ['start', 'start-media-failure', 'join', 'join-racing-ban', 'end', 'mute', 'unmute', 'mute-media-failure', 'start-uppercase-id', 'join-uppercase-id', 'end-uppercase-id', 'mute-uppercase-id', 'mute-uppercase-target']) {
      const steps = [], payloads = []
      const analytics = await createAnalyticsComponent({ logs, config: createRecordConfigComponent({ ANALYTICS_CONTEXT: 'social-fixture', ANALYTICS_API_URL: 'https://analytics.example/events', ANALYTICS_API_TOKEN: 'fixture-token', ENV: 'fixture' }), fetcher: { fetch: async (_url, options) => { const body = JSON.parse(options.body); payloads.push(body); steps.push(`analytics:${body.event}`); return new Response(null) } } })
      const deps = await components('success', id.startsWith('start'))
      const requestedId = id.endsWith('uppercase-id') ? community.toUpperCase() : community
      const requestedTarget = id === 'mute-uppercase-target' ? target.toUpperCase() : target
      const getCommunity = deps.communitiesDb.getCommunity
      deps.communitiesDb.getCommunity = async () => ({ ...await getCommunity(), id: community })
      const action = async () => { steps.push('media:action'); if (id.endsWith('media-failure')) throw Error('synthetic media failure') }
      deps.commsGatekeeper.createCommunityVoiceChatRoom = async () => { await action(); return { connectionUrl: 'community-grant' } }
      deps.commsGatekeeper.getCommunityVoiceChatCredentials = async () => { await action(); return { connectionUrl: 'community-grant' } }
      deps.commsGatekeeper.endCommunityVoiceChatRoom = action
      deps.commsGatekeeper.muteSpeakerInCommunityVoiceChat = action
      deps.commsGatekeeper.kickUserFromCommunityVoiceChat = action
      if (id === 'join-racing-ban') { let checks = 0; deps.communitiesDb.isMemberBanned = async () => checks++ > 0 }
      const domain = await load('logic/community-voice/community-voice.ts').createCommunityVoiceComponent({ ...deps, logs, analytics,
        pubsub: { publishInChannel: async (_channel, update) => steps.push(`publish:${update.status}`) }, registry: { getProfile: async () => { throw Error('profile unavailable') } },
        communityThumbnail: { getThumbnail: async () => null }, communityPlaces: {}, placesApi: {}, communityBroadcaster: { broadcast: async () => steps.push('broadcast') },
        communityVoiceChatCache: { setCommunityVoiceChat: async () => steps.push('cache:set'), getCommunityVoiceChat: async () => ({ notificationScope: 'all' }), removeCommunityVoiceChat: async () => steps.push('cache:remove') } })
      let error = null
      try {
        if (id.startsWith('start')) await domain.startCommunityVoiceChat(requestedId, caller)
        else if (id.startsWith('join')) await domain.joinCommunityVoiceChat(requestedId, caller)
        else if (id.startsWith('end')) await domain.endCommunityVoiceChat(requestedId, caller)
        else await domain.muteSpeakerInCommunityVoiceChat(requestedId, requestedTarget, caller, id !== 'unmute')
      } catch (caught) { error = caught.message }
      await new Promise(setImmediate)
      cases.push({ id: `community.${id}`, kind: 'community', scenario: id, error, payloads, steps })
    }
    fs.writeFileSync(process.argv[3], JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision }, scope: 'Actual source voice/community domain and analytics SDK, private voice-db on owned PostgreSQL; media replies and analytics fetch controlled. Random start UUID normalized to the fixed call UUID. Steps capture source invocation order; network completion order is not promised. A tagged lead-only case records a separately pending source mismatch.', caller, target, community, call, cases }, null, 2) + '\n')
    process.stdout.write(`Captured ${cases.length} voice analytics cases\n`)
  } finally { if (pg) await pg.end(); await admin.query(`DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`); await admin.end() }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
