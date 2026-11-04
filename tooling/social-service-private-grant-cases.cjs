#!/usr/bin/env node
// Executes the pinned voice database/domain and RPC/HTTP handlers with incomplete
// media credential maps. The database is owned by this capture and always removed.
const fs = require('node:fs')
const { randomUUID } = require('node:crypto')
const { req, load, logs, caller, target, revision } = require('./social-service-voice-source-cases.cjs')
const databaseUrl = process.env.CATALYST_TEST_DATABASE_URL
if (!databaseUrl) throw Error('CATALYST_TEST_DATABASE_URL is required; capture never silently skips')
const { Pool } = req('pg')
const proto = req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const { errorHandler } = req('@dcl/http-commons')
const { voiceChatsErrorsHandler } = load('controllers/middlewares/voice-chats-errors.ts')
const validator = req('@dcl/schema-validator-component').createSchemaValidatorComponent({ ensureJsonContentType: false })
const schema = load('controllers/handlers/http/schemas.ts').UpdatePrivateVoiceChatSchema
const call = 'b5555555-5555-4555-8555-555555555555'
const connection = address => `wss://livekit.test/voice-chat-private-${call}?for=${address}`

async function main() {
  const adminUrl = new URL(databaseUrl)
  adminUrl.pathname = '/postgres'
  const admin = new Pool({ connectionString: adminUrl.toString(), max: 1 })
  const name = `social_source_grants_${process.pid}_${randomUUID().replaceAll('-', '')}`
  let pg
  try {
    await admin.query(`CREATE DATABASE "${name}"`)
    const ownedUrl = new URL(databaseUrl)
    ownedUrl.pathname = '/' + name
    pg = new Pool({ connectionString: ownedUrl.toString(), max: 2 })
    await pg.query('CREATE TABLE private_voice_chats (id UUID PRIMARY KEY, caller_address TEXT, callee_address TEXT, created_at TIMESTAMP DEFAULT now())')
    const config = { requireNumber: async () => 60000, requireString: async () => 'https://gatekeeper.example' }
    const voiceDb = await load('adapters/voice-db.ts').createVoiceDBComponent({ pg, config })
    let grants = {}, updates = [], analytics = []
    const gatekeeper = await load('adapters/comms-gatekeeper.ts').createCommsGatekeeperComponent({ logs, config,
      fetcher: { fetch: async () => new Response(JSON.stringify(grants), { status: 200 }) } })
    const voice = await load('logic/voice/voice.ts').createVoiceComponent({ logs, config, voiceDb, commsGatekeeper: gatekeeper,
      settings: {}, friendsDb: {}, analytics: { fireEvent: (event, data) => analytics.push({ event, data }) },
      pubsub: { publishInChannel: async (_channel, update) => updates.push(update) } })
    const components = { logs, voice }
    const rpc = load('controllers/handlers/rpc/accept-private-voice-chat.ts').acceptPrivateVoiceChatService({ components })
    const http = load('controllers/handlers/http/update-private-voice-chat-handler.ts').updatePrivateVoiceChatHandler
    const cases = []
    for (const protocol of ['rpc', 'http']) for (const scenario of ['complete', 'missing-caller', 'missing-callee', 'missing-both']) {
      await pg.query('TRUNCATE private_voice_chats')
      await pg.query('INSERT INTO private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,$3)', [call, caller, target])
      grants = {}
      if (!['missing-caller', 'missing-both'].includes(scenario)) grants[caller] = { connection_url: connection(caller) }
      if (!['missing-callee', 'missing-both'].includes(scenario)) grants[target] = { connection_url: connection(target) }
      updates = []
      analytics = []
      let response, expected_hex
      if (protocol === 'rpc') {
        response = await rpc({ callId: call }, { address: target })
        expected_hex = Buffer.from(proto.AcceptPrivateVoiceChatResponse.encode(response).finish()).toString('hex')
      } else {
        const url = new URL(`https://social.example/v1/private-voice-chats/${call}`)
        const ctx = { components, params: { callId: call }, verification: { auth: target }, url,
          request: new Request(url, { method: 'PATCH', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ intention: 'accepted' }) }) }
        const result = await errorHandler(ctx, () => voiceChatsErrorsHandler(ctx, () =>
          validator.withSchemaValidatorMiddleware(schema)(ctx, () => http(ctx))))
        response = { status: result.status, body: result.body ?? null }
      }
      const remaining = await pg.query('SELECT count(*)::int AS count FROM private_voice_chats')
      cases.push({ id: `${protocol}.${scenario}`, protocol, scenario, response,
        ...(expected_hex === undefined ? {} : { expected_hex }), remaining_rows: remaining.rows[0].count, updates, analytics })
    }
    fs.writeFileSync(process.argv[3], JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision },
      scope: 'Actual upstream voice-db PostgreSQL queries, media HTTP response adapter, private voice domain, RPC/HTTP controllers and HTTP schema/error middleware; owned temporary database. HTTP authentication is bypassed with an explicit verified actor. Credential responses are controlled, not a live-media claim.',
      caller, target, call, cases }, null, 2) + '\n')
    process.stdout.write(`Captured ${cases.length} upstream private credential cases\n`)
  } finally {
    if (pg) await pg.end()
    await admin.query(`DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`)
    await admin.end()
  }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
