#!/usr/bin/env node
// Actual config provider, referral component/adapter, PG wrapper and HTTP error
// middleware. PostgreSQL TEMP relations; all outbound providers injected.
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto')
const { req, load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const referrer = '0xa711111111111111111111111111111111111111', invited = n => `0xa8${n.toString(16).padStart(38,'0')}`
const RealDate = Date
let clock = RealDate.parse('2026-01-01T12:00:00Z')
global.Date = class extends RealDate { constructor(...args) { super(...(args.length ? args : [clock])) } static now() { return clock } }
global.fetch = async () => ({ ok: true, json: async () => ({ users: [] }) })
const defaults = { REFERRAL_MAX_IP_MATCHES:'2', REFERRAL_MIN_LOGIN_DAYS:'3', REFERRAL_FIVE_MINUTES_IN_MS:'300000', PROFILE_URL:'https://profile.example.org', ENV:'dev', REFERRAL_METABASE_DASHBOARD:'https://dashboard.example.org' }
for (const tier of [5,10,20,25,30,50,60,75]) defaults[`REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_${tier}`] = ''
async function main() {
  if (!process.env.CATALYST_TEST_DATABASE_URL) throw Error('Disposable PostgreSQL URL required')
  const makeConfig = values => req('@well-known-components/env-config-provider').createConfigComponent({ ...defaults, ...values })
  const pg = await load('adapters/pg.ts').createPgComponent({ logs, config:makeConfig({}) }, {pool:{connectionString:process.env.CATALYST_TEST_DATABASE_URL,max:1}})
  await pg.start()
  try {
    await pg.query(`CREATE TEMP TABLE referral_progress(id uuid PRIMARY KEY,referrer text NOT NULL,invited_user text NOT NULL UNIQUE,status text NOT NULL,signed_up_at bigint,tier_granted boolean NOT NULL DEFAULT false,tier_granted_at bigint,created_at bigint NOT NULL,updated_at bigint NOT NULL,invited_user_ip text);
      CREATE TEMP TABLE referral_progress_viewed(referrer text PRIMARY KEY,invites_accepted_viewed bigint,created_at bigint DEFAULT 0,updated_at bigint);
      CREATE TEMP TABLE referral_reward_images(id uuid PRIMARY KEY,referrer text,reward_image_url text,tier integer,created_at bigint);
      CREATE TEMP TABLE referral_emails(id uuid PRIMARY KEY,referrer text,email text,created_at bigint,updated_at bigint)`)
    const cases = []
    for (const [kind, inputs] of [['max',['2tail','2.5','Infinity','-Infinity','0','-1','1e20','1e21','1e-7','NaN']], ['min',['2.5days','0','-1','Infinity','-Infinity','NaN']]]) {
      for (const raw of inputs) {
        await pg.query('TRUNCATE referral_progress,referral_progress_viewed,referral_reward_images,referral_emails')
        clock = RealDate.parse('2026-01-01T12:00:00Z')
        const key = kind === 'max' ? 'REFERRAL_MAX_IP_MATCHES' : 'REFERRAL_MIN_LOGIN_DAYS'
        const config = makeConfig({ [key]:raw }), cache = new Map(), steps = []
        let number, startup_error
        try { number = String(await config.requireNumber(key)) } catch(error) { startup_error = error.message }
        if (!startup_error) {
          const referralDb = await load('adapters/referral-db.ts').createReferralDBComponent({ pg, logs, config })
          const referral = await load('logic/referral/referral.ts').createReferralComponent({ referralDb, logs, config,
            redis:{get:async key=>cache.get(key),put:async(key,value)=>cache.set(key,value)},
            sns:{publishMessage:async()=>{}},slack:{sendMessage:async()=>{}},email:{sendEmail:async()=>{}},rewards:{sendReward:async()=>{throw Error('unexpected reward')}} })
          if (kind === 'max') {
            for (let i=1;i<=3;i++) {
              const ctx={components:{logs,referral,featureFlags:{isEnabled:()=>false}},verification:{auth:invited(i)},url:new URL('https://social.example.org/v1/referral-progress'),request:new Request('https://social.example.org/v1/referral-progress',{method:'POST',headers:{'cf-connecting-ip':'192.0.2.1'},body:JSON.stringify({referrer})})}
              const response = await req('@dcl/http-commons').errorHandler(ctx,()=>load('controllers/handlers/http/create-referral-handler.ts').createReferralHandler(ctx))
              const rows = (await pg.query('SELECT status FROM referral_progress ORDER BY invited_user')).rows.map(row=>row.status)
              steps.push({response,rows})
            }
          } else {
            await referral.create({referrer,invitedUser:invited(1),invitedUserIP:'192.0.2.1'})
            for (let i=1;i<=5;i++) {
              clock = RealDate.parse(`2026-01-0${i}T12:00:00Z`)
              await referral.finalizeReferral(invited(1))
              const row = (await pg.query('SELECT status,tier_granted FROM referral_progress')).rows[0]
              steps.push({day:`2026-01-0${i}`,row,days:cache.get(`referral:invited-user:${invited(1)}`)||[]})
            }
          }
        }
        cases.push({kind,raw,number,startup_error,steps})
      }
    }
    const files=['src/adapters/pg.ts','src/adapters/referral-db.ts','src/logic/referral/referral.ts','src/controllers/handlers/http/create-referral-handler.ts','node_modules/@well-known-components/env-config-provider/dist/index.js','node_modules/@dcl/http-commons/dist/controllers/handlers/error-handler.js']
    fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,sources:files.map(file=>({path:file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')}))},runtime:{node:process.version},scope:'Actual source numeric config, referral domain, DB adapter, translating PG wrapper and create-handler/error middleware execute against PostgreSQL TEMP relations. Login-day cache and outbound effects are injected. Startup rejection text is recorded, not a claim that Clap formats startup diagnostics identically.',referrer,invited:[invited(1),invited(2),invited(3)],cases},null,2)+'\n')
    process.stdout.write(`Captured ${cases.length} source referral numeric configurations\n`)
  } finally { await pg.stop() }
}
main().catch(error=>{console.error(error);process.exitCode=1})
