#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/private-id-source-cases.json
// Executes the pinned private voice database, domain and RPC components against
// an owned temporary PostgreSQL database. Media HTTP replies are deterministic.
const { requireNode26, sourceCapture, requireDatabaseUrl } = require('./social-capture-common.cjs')
requireNode26()
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: node tooling/social-service-private-id-cases.cjs <upstream-checkout> <output.json>')
const databaseUrl = requireDatabaseUrl(process.env.CATALYST_TEST_DATABASE_URL)
const fs = require('node:fs')
const { randomUUID } = require('node:crypto')
const { req, load, revision } = sourceCapture(process.argv[2])
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const caller = '0x1111111111111111111111111111111111111111'
const target = '0x2222222222222222222222222222222222222222'
const output = process.argv[3]
const { Pool } = req('pg')
const proto = req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const call = 'a4444444-4444-4444-8444-444444444444'
const connection = (address) => `wss://livekit.test/voice-chat-private-${call}?for=${address}`
async function main() {
  const adminUrl = new URL(databaseUrl); adminUrl.pathname = '/postgres'
  const admin = new Pool({ connectionString: adminUrl.toString(), max: 1 })
  const name = `social_source_voice_${process.pid}_${randomUUID().replaceAll('-', '')}`
  let pg
  try {
    await admin.query(`CREATE DATABASE "${name}"`)
    const ownedUrl = new URL(databaseUrl); ownedUrl.pathname = '/' + name
    pg = new Pool({ connectionString: ownedUrl.toString(), max: 2 })
    await pg.query('CREATE TABLE private_voice_chats (id UUID PRIMARY KEY, caller_address TEXT, callee_address TEXT, created_at TIMESTAMP DEFAULT now())')
    const config = { requireNumber: async () => 60000, requireString: async () => 'https://gatekeeper.example' }
    const voiceDb = await load('adapters/voice-db.ts').createVoiceDBComponent({ pg, config })
    const gatekeeper = await load('adapters/comms-gatekeeper.ts').createCommsGatekeeperComponent({ logs, config,
      fetcher: { fetch: async (_url, options) => new Response(JSON.stringify(options.method === 'DELETE'
        ? { users_in_voice_chat: [] }
        : { [caller]: { connection_url: connection(caller) }, [target]: { connection_url: connection(target) } }), { status: 200 }) } })
    let updates = []
    const voice = await load('logic/voice/voice.ts').createVoiceComponent({ logs, config, voiceDb, commsGatekeeper: gatekeeper,
      settings: {}, friendsDb: {}, analytics: { fireEvent() {} }, pubsub: { publishInChannel: async (_channel, update) => updates.push(update) } })
    const variants = [
      ['empty', ''], ['malformed', 'not-a-uuid'], ['canonical', call], ['uppercase', call.toUpperCase()],
      ['compact', call.replaceAll('-', '')], ['braced', `{${call}}`], ['urn', `urn:uuid:${call}`],
      ['four-digit-groups', 'a444-4444-4444-4444-8444-4444-4444-4444']
    ]
    const cases = []
    for (const method of ['accept', 'reject', 'end']) {
      const factory = `${method}PrivateVoiceChatService`
      const responseType = `${method[0].toUpperCase() + method.slice(1)}PrivateVoiceChatResponse`
      const handler = load(`controllers/handlers/rpc/${method}-private-voice-chat.ts`)[factory]({ components: { logs, voice } })
      for (const pending of [false, true]) for (const [variant, callId] of variants) {
        await pg.query('TRUNCATE private_voice_chats')
        if (pending) await pg.query('INSERT INTO private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,$3)', [call, caller, target])
        updates = []
        const address = method === 'end' ? caller : target
        const response = await handler({ callId }, { address })
        const remaining = await pg.query('SELECT count(*)::int AS count FROM private_voice_chats')
        cases.push({ id: `${method}.${pending ? 'pending' : 'absent'}.${variant}`, method, pending, variant, call_id: callId,
          caller: address, response, expected_hex: Buffer.from(proto[responseType].encode(response).finish()).toString('hex'),
          remaining_rows: remaining.rows[0].count, updates })
      }
    }
    fs.writeFileSync(output, JSON.stringify({ baseline: { repository: 'decentraland/social-service-ea', revision },
      scope: 'Actual upstream voice-db PostgreSQL queries, private voice domain, RPC handlers and privileged identifier guards; owned temporary database; deterministic media HTTP replies. Not live-media equivalence.',
      caller, target, call, cases }, null, 2) + '\n')
    process.stdout.write(`Captured ${cases.length} upstream private identifier cases\n`)
  } finally {
    if (pg) await pg.end()
    await admin.query(`DROP DATABASE IF EXISTS "${name}" WITH (FORCE)`)
    await admin.end()
  }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
