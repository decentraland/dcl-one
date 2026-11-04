#!/usr/bin/env node
// Optional development capture: exact upstream TS domain + DB adapter on real
// PostgreSQL TEMP relations, injected Redis/day cache and external providers.
const fs = require('node:fs')
const path = require('node:path')
const crypto = require('node:crypto')
const { execFileSync } = require('node:child_process')
const { createRequire } = require('node:module')
const baseline = 'b6b2cefefdf727b2ed47ddb887e77d226a41eea6'
const upstream = path.resolve(process.argv[2] || '')
const output = process.argv[3]
if (!process.argv[2] || !output || !process.env.CATALYST_TEST_DATABASE_URL) throw Error('Usage: CATALYST_TEST_DATABASE_URL=<disposable-pg> node tooling/social-service-referral-cases.cjs <upstream> <output.json>')
const git = (...args) => execFileSync('git', ['-C', upstream, ...args], { encoding: 'utf8' }).trim()
if (git('rev-parse', 'HEAD') !== baseline || git('status', '--porcelain', '--untracked-files=no')) throw Error('Pinned clean upstream required')
const from = createRequire(path.join(upstream, 'package.json'))
from('ts-node').register({ transpileOnly: true, project: path.join(upstream, 'tsconfig.json') })
const load = file => from(path.join(upstream, 'src', file))
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const address = n => `0x${n.toString(16).padStart(40, '0')}`
const referrer = address(1)
const originalNow = Date.now
let clock = Date.parse('2026-01-01T12:00:00Z')
const RealDate = Date
// Source uses both Date.now and new Date().toISOString for login-day storage.
global.Date = class extends RealDate { constructor(...args) { super(...(args.length ? args : [clock])) } static now() { return clock } }
const configValues = { REFERRAL_MAX_IP_MATCHES: 2, REFERRAL_MIN_LOGIN_DAYS: 3, REFERRAL_FIVE_MINUTES_IN_MS: 300000, PROFILE_URL: 'https://profile.example.org', ENV: 'dev', REFERRAL_METABASE_DASHBOARD: 'https://dashboard.example.org' }
const config = { requireNumber: async name => configValues[name], requireString: async name => configValues[name] ?? `campaign-${name.split('_').at(-1)}` }
let deny = [], effects = [], cache = new Map()
global.fetch = async () => ({ ok: true, json: async () => ({ users: deny.map(wallet => ({ wallet })) }) })
async function main() {
 const client = new (from('pg').Client)({ connectionString: process.env.CATALYST_TEST_DATABASE_URL })
 await client.connect()
 try {
  // These TEMP relations follow the pinned source migrations; nothing is
  // installed in or removed from the supplied database's persistent schema.
  await client.query(`CREATE TEMP TABLE referral_progress(id uuid PRIMARY KEY,referrer text NOT NULL,invited_user text NOT NULL UNIQUE,status text NOT NULL,signed_up_at bigint,tier_granted boolean NOT NULL DEFAULT false,tier_granted_at bigint,created_at bigint NOT NULL,updated_at bigint NOT NULL,invited_user_ip text);
   CREATE TEMP TABLE referral_progress_viewed(referrer text PRIMARY KEY,invites_accepted_viewed bigint,created_at bigint DEFAULT 0,updated_at bigint);
   CREATE TEMP TABLE referral_emails(id uuid PRIMARY KEY,referrer text,email text,created_at bigint,updated_at bigint);
   CREATE TEMP TABLE referral_reward_images(id uuid PRIMARY KEY,referrer text,reward_image_url text,tier integer,created_at bigint)`)
  const referralDb = await load('adapters/referral-db.ts').createReferralDBComponent({ pg: { query: query => client.query(query) }, logs, config })
  const referral = await load('logic/referral/referral.ts').createReferralComponent({ referralDb, logs, config,
   redis: { get: async key => cache.get(key), put: async (key, value, options) => { cache.set(key, value); effects.push({ kind: 'cache', key, value, options }) } },
   sns: { publishMessage: async event => effects.push({ kind: 'sns', event }) },
   rewards: { sendReward: async (campaign, beneficiary) => { effects.push({ kind: 'reward', campaign, beneficiary }); return [{ image: 'https://reward.example.org/tier.png', rarity: 'rare' }] } },
   email: { sendEmail: async (...args) => effects.push({ kind: 'email', args }) },
   slack: { sendMessage: async message => effects.push({ kind: 'slack', message }) }
  })
  const cases = []
  async function scenario(id, steps) {
   await client.query('TRUNCATE referral_progress,referral_progress_viewed,referral_emails,referral_reward_images')
   cache = new Map(); deny = []; clock = RealDate.parse('2026-01-01T12:00:00Z')
   const attempts = []
   for (const step of steps) {
    if (step.day) clock = RealDate.parse(`${step.day}T12:00:00Z`)
    if (step.deny) deny = step.deny
    effects = []
    let result = 'ok'
    try {
     if (step.action === 'create') await referral.create({ referrer: step.referrer || referrer, invitedUser: step.invited, invitedUserIP: step.ip || '192.0.2.1' })
     if (step.action === 'signup') await referral.updateProgress(step.invited, 'signed_up')
     if (step.action === 'login') await referral.finalizeReferral(step.invited)
     if (step.action === 'stats') result = await referral.getInvitedUsersAcceptedStats(step.referrer || referrer)
     if (step.action === 'email') await referral.setReferralEmail({ referrer: step.referrer || referrer, email: step.email })
     if (step.action === 'seed-granted') await client.query("INSERT INTO referral_progress(id,referrer,invited_user,status,tier_granted,created_at,updated_at) SELECT gen_random_uuid(),$1,'0x'||lpad(to_hex(i+10000),40,'0'),'tier_granted',true,$2,$2 FROM generate_series(1,$3)i", [referrer, clock, step.count])
    } catch (error) { result = { error: error.constructor.name, message: error.message } }
    const rows = (await client.query('SELECT referrer,invited_user,invited_user_ip,status,signed_up_at,tier_granted,tier_granted_at,created_at,updated_at FROM referral_progress ORDER BY invited_user')).rows
    const images = (await client.query('SELECT referrer,reward_image_url,tier,created_at FROM referral_reward_images ORDER BY tier')).rows
    attempts.push({ step, now_ms: clock, result, rows, images, cache: Object.fromEntries(cache), effects })
   }
   cases.push({ id, attempts })
  }
  const create = invited => ({ action: 'create', invited: address(invited) })
  const login = (invited, day) => ({ action: 'login', invited: address(invited), day })
  await scenario('create-first-wins-and-idempotent-before-denylist', [create(2), { ...create(2), deny: [referrer] }, { ...create(2), referrer: address(3) }])
  await scenario('third-ip-match-persists-rejection-before-error', [create(2), create(3), create(4), create(4)])
  await scenario('direct-and-one-hop-denylist', [{ ...create(2), deny: [referrer] }, { ...create(2), deny: [] }, { ...create(3), referrer: address(2), deny: [referrer] }])
  await scenario('signup-state-and-denylist-order', [create(2), { action: 'signup', invited: address(2) }, { action: 'signup', invited: address(2) }, { action: 'signup', invited: address(2), deny: [referrer] }])
  await scenario('three-days-grant-on-next-invocation-and-only-once', [create(2), login(2,'2026-01-01'),login(2,'2026-01-01'),login(2,'2026-01-02'),login(2,'2026-01-03'),login(2,'2026-01-03'),login(2,'2026-01-04'),{action:'stats'},{action:'stats'}])
  await scenario('fifth-invite-rewards-art-and-notifications', [{action:'seed-granted',count:4},create(2),login(2,'2026-01-01'),login(2,'2026-01-02'),login(2,'2026-01-03'),login(2,'2026-01-04'),{action:'stats'}])
  await scenario('irl-email-threshold-and-normalization', [{action:'email',email:'person@example.org'},{action:'seed-granted',count:100},{action:'email',email:' Person@Example.org '},{action:'email',email:'other@example.org'}])
  const validator = from('@dcl/schema-validator-component').createSchemaValidatorComponent({ensureJsonContentType:false})
  const schemas = []
  for (const [schema, inputs] of [['CreateReferralSchema',[{},null,[],{referrer:''},{referrer:referrer,extra:1},{referrer:referrer}]],['AddReferralEmailSchema',[{}, {email:''}, {email:'not-an-email'}, {email:' Person@Example.org '}, {email:'a+b@example.org'}, {email:'person@example.org'}, {email:1}]]]) {
   const middleware = validator.withSchemaValidatorMiddleware(load('controllers/handlers/http/schemas.ts')[schema])
   for (const input of inputs) {
    const raw = JSON.stringify(input)
    const response = await middleware({request:new Request('https://example.org/referral',{method:'POST',body:raw})},async()=>({status:204}))
    schemas.push({schema,raw,response})
   }
  }
  const queueCases = [
   {body:JSON.stringify({type:'client',subType:'logged-in',metadata:{userAddress:address(2)}}),receipt:'login'},
   {body:JSON.stringify({type:'client',subType:'logged-in-cached',metadata:{userAddress:address(2)}}),receipt:'cached'},
   {body:JSON.stringify({type:'client',subType:'logged-in',metadata:{}}),receipt:'missing-user'},
   {body:JSON.stringify({type:'client',subType:'logged-in',metadata:{userAddress:'invalid'}}),receipt:'handler-error'},
   {body:'{invalid',receipt:'malformed'}, {body:'null',receipt:'null'},
   {body:JSON.stringify({type:'unknown',subType:'unknown'}),receipt:'unknown'},
   {body:JSON.stringify({Message:JSON.stringify({type:'client',subType:'logged-in',metadata:{userAddress:address(2)}})}),receipt:'sns-envelope'}
  ]
  const queueEffects=[]
  let received=false,done
  const completed=new Promise(resolve=>{done=resolve})
  const queue={receiveMessages:async(amount,options)=>{
   if(!received){received=true;return queueCases.map(value=>({Body:value.body,ReceiptHandle:value.receipt}))}
   return new Promise((resolve,reject)=>options.abortSignal.addEventListener('abort',()=>reject(Error('aborted')),{once:true}))
  },deleteMessage:async receipt=>{queueEffects.push({kind:'delete',receipt});if(receipt==='sns-envelope')done()},changeMessagesVisibility:async()=>{}}
  const processor=from('@dcl/queue-consumer-component').createQueueConsumerComponent({sqs:queue,logs})
  const loggedIn=load('controllers/handlers/sqs/logged-in-handler.ts').createLoggedInHandler({logs,referral:{finalizeReferral:async user=>{queueEffects.push({kind:'call',user});if(user==='invalid')throw Error('invalid invited user')}}})
  for(const subtype of loggedIn.subTypes)processor.addMessageHandler(loggedIn.type,subtype,loggedIn.handle)
  const lifecycle=from('@well-known-components/interfaces')
  await processor[lifecycle.START_COMPONENT]();await completed;await processor[lifecycle.STOP_COMPONENT]()
  const version = (await client.query('SHOW server_version')).rows[0].server_version
  const lock = fs.readFileSync(path.join(upstream,'yarn.lock'))
  fs.writeFileSync(output, JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision:baseline,yarn_lock_sha256:crypto.createHash('sha256').update(lock).digest('hex')},runtime:{node:process.version,postgres:version},scope:'Actual upstream referral component and referral-db adapter executed against PostgreSQL TEMP relations. Redis login cache, denylist fetch and outbound SNS/rewards/email/Slack are injected deterministic providers; not deployed ingestion or delivery equivalence. Random UUIDs omitted from snapshots; all other stored source fields retained.',referrer,cases,schemas,queue_cases:queueCases,queue_effects:queueEffects},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} upstream referral scenarios\n`)
 } finally { await client.end(); global.Date=RealDate; Date.now=originalNow }
}
main().catch(error => { console.error(error); process.exitCode=1 })
