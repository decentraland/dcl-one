#!/usr/bin/env node
// Optional source execution. Only a PostgreSQL TEMP function is installed;
// external services are injected and no messages leave the process.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { req, load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const output = process.argv[3]
async function main() {
  if (!process.env.CATALYST_TEST_DATABASE_URL) throw Error('Disposable PostgreSQL URL required')
  const client = new (req('pg').Client)({ connectionString: process.env.CATALYST_TEST_DATABASE_URL })
  await client.connect()
  try {
    await client.query("CREATE FUNCTION pg_temp.social_handler_failure() RETURNS text LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'source dependency failure'; END $$")
    const fail = async () => client.query('SELECT pg_temp.social_handler_failure()')
    const address = '0x9111111111111111111111111111111111111111'
    const configurations = [
      ['GET', '/v1/referral-progress', 'get-invited-users-accepted', 'getInvitedUsersAcceptedHandler', 'getInvitedUsersAcceptedStats'],
      ['POST', '/v1/referral-progress', 'create-referral', 'createReferralHandler', 'create'],
      ['PATCH', '/v1/referral-progress', 'update-referral-signed-up', 'updateReferralSignedUpHandler', 'updateProgress'],
      ['POST', '/v1/referral-email', 'add-referral-email', 'addReferralEmailHandler', 'setReferralEmail']
    ]
    const cases = []
    for (const [method, route, file, factory, operation] of configurations) {
      const request = new Request(`https://social.example.org${route}`, {
        method, ...(method === 'POST' ? { body: JSON.stringify({ referrer: address, email: 'person@example.org' }) } : {}),
        headers: { 'cf-connecting-ip': '192.0.2.1', 'content-type': 'application/json' }
      })
      const response = await load(`controllers/handlers/http/${file}-handler.ts`)[factory]({
        components: { logs, referral: { [operation]: fail }, featureFlags: { isEnabled: () => false } },
        request, verification: { auth: address }
      })
      cases.push({ method, path: route, response })
    }
    const privacy = await load('controllers/handlers/uws/privacy-handler.ts').createPrivacyHandler({ logs, friendsDb: { getSocialSettings: fail } })
    cases.push({ method: 'GET', path: '/v1/users/:address/privacy-settings', response: await privacy.f({}, { getParameter: () => address }) })
    const sources = ['src/controllers/handlers/uws/privacy-handler.ts', 'src/utils/errors.ts', 'src/adapters/pg.ts', ...configurations.map(([, , file]) => `src/controllers/handlers/http/${file}-handler.ts`)]
    fs.writeFileSync(output, JSON.stringify({
      baseline: { repository: 'decentraland/social-service-ea', revision, sources: sources.map(file => ({ path: file, sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream, file))).digest('hex') })) },
      runtime: { node: process.version },
      scope: 'Actual five pinned HTTP/uWS handlers with dependency calls raising a real PostgreSQL server exception through node-pg. Domain dependencies are injected; local replay raises the same server exception through SQLx and the real routes. This does not establish equal messages for unrelated drivers or operating-system failures.',
      cases
    }, null, 2) + '\n')
    process.stdout.write(`Captured ${cases.length} source handler dependency errors\n`)
  } finally { await client.end() }
}
main().catch(error => { console.error(error); process.exitCode = 1 })
