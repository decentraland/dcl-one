#!/usr/bin/env node
// Actual pinned domain/DB adapters on PostgreSQL TEMP relations, controlled
// external providers and barriers. No provider endpoint is contacted.
const fs = require('node:fs'), crypto = require('node:crypto')
const { req, load, logs, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const { createRecordConfigComponent } = req('@well-known-components/env-config-provider')
const address = n => '0x' + n.toString(16).padStart(40, '0')
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r }); return { promise, resolve } }
const turn = () => new Promise(setImmediate)
async function main() {
 const client = new (req('pg').Client)({ connectionString: process.env.CATALYST_TEST_DATABASE_URL })
 await client.connect()
 try {
  await client.query(`CREATE TEMP TABLE community_members(community_id uuid, member_address text, role text, joined_at timestamp);
  CREATE TEMP TABLE referral_progress(id uuid PRIMARY KEY, referrer text NOT NULL, invited_user text UNIQUE NOT NULL, invited_user_ip text, status text NOT NULL, signed_up_at bigint, tier_granted boolean NOT NULL DEFAULT false, tier_granted_at bigint, created_at bigint NOT NULL, updated_at bigint NOT NULL);
  CREATE TEMP TABLE referral_reward_images(id uuid PRIMARY KEY, referrer text, reward_image_url text, tier integer, created_at bigint)`)
  const statements = []
  const pg = { query: async query => { statements.push({ text: query.text.replace(/\s+/g,' ').trim(), values: query.values }); return client.query(query) } }
  const communities = load('adapters/communities-db.ts').createCommunitiesDBComponent({ pg, logs })
  const id = '00000000-0000-4000-8000-000000000001'
  await client.query("INSERT INTO community_members SELECT $1,'0x'||lpad(to_hex(n),40,'0'),'member',timestamp '2026-01-01' FROM generate_series(230,1,-1)n", [id])
  const members = await communities.getCommunityMembers(id, { pagination: {limit:1000,offset:0}, excludedAddresses:[address(10),address(11)] })
  const membership = { excluded:[address(10),address(11)], returned_excluded:members.map(row=>row.memberAddress).filter(value=>[address(10),address(11)].includes(value)), sql: statements.at(-1).text, bound_exclusions:statements.at(-1).values.find(Array.isArray) }
  const config = createRecordConfigComponent({ REFERRAL_MAX_IP_MATCHES:'2', REFERRAL_MIN_LOGIN_DAYS:'0', REFERRAL_FIVE_MINUTES_IN_MS:'0', PROFILE_URL:'https://profile.example.org', ENV:'dev', REFERRAL_METABASE_DASHBOARD:'', ...Object.fromEntries([5,10,20,25,30,50,60,75].map(tier=>['REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_'+tier,'fixture'])) })
  const db = await load('adapters/referral-db.ts').createReferralDBComponent({ pg, logs, config })
  const originalFetch = global.fetch
  global.fetch = async () => ({ok:true,json:async()=>({users:[]})})
  const failures = []
  try {
   for (const scenario of ['first-sns-failure','reward-failure','tier-sns-failure','image-failure-first']) {
    await client.query('TRUNCATE referral_progress,referral_reward_images')
    await client.query("INSERT INTO referral_progress(id,referrer,invited_user,status,tier_granted,created_at,updated_at) SELECT gen_random_uuid(),$1,'0x'||lpad(to_hex(n+100),40,'0'),'tier_granted',true,1,1 FROM generate_series(1,4)n",[address(1)])
    await db.createReferral({referrer:address(1), invitedUser:address(2),invitedUserIP:'192.0.2.1'})
    const trace=[], blocked=deferred(), imageStarted=deferred()
    let snsCalls=0
    const component = await load('logic/referral/referral.ts').createReferralComponent({config,logs,referralDb:{...db,setReferralRewardImage:async input=>{trace.push('image-start');imageStarted.resolve();if(scenario==='tier-sns-failure')await blocked.promise;if(scenario==='image-failure-first')throw Error('fixture image failure');await db.setReferralRewardImage(input);trace.push('image-committed')}},redis:{get:async()=>[],put:async()=>{}},sns:{publishMessage:async()=>{snsCalls++;trace.push('sns-'+snsCalls);if(scenario==='first-sns-failure'||(scenario==='tier-sns-failure'&&snsCalls===2))throw Error('fixture SNS failure');if(scenario==='image-failure-first'&&snsCalls===2)await blocked.promise;trace.push('sns-'+snsCalls+'-completed')}},rewards:{sendReward:async()=>{trace.push('reward');if(scenario==='reward-failure')throw Error('fixture reward failure');return[{image:'https://reward.example.org/tier.png',rarity:'rare'}]}},slack:{sendMessage:async()=>{}},email:{sendEmail:async()=>{}}})
    let error
    await component.finalizeReferral(address(2)).catch(failure=>{error=failure.message;trace.push('handler-error')})
    const before_release=[...trace]
    blocked.resolve()
    await turn();await turn()
    await component.finalizeReferral(address(2))
    const state=(await client.query('SELECT status,tier_granted FROM referral_progress WHERE invited_user=$1',[address(2)])).rows[0]
    const images=(await client.query('SELECT tier FROM referral_reward_images ORDER BY tier')).rows.map(row=>row.tier)
    failures.push({scenario,error,before_release,trace,state,images,sns_calls:snsCalls})
   }
  } finally { global.fetch=originalFetch }
  await client.query('TRUNCATE referral_progress')
  await db.createReferral({referrer:address(1),invitedUser:address(2),invitedUserIP:'192.0.2.1'})
  await db.updateReferralProgress(address(2),'tier_granted')
  await db.updateReferralProgress(address(2),'signed_up')
  const signed_up_adapter={sql:statements.at(-1).text,state:(await client.query('SELECT status,tier_granted FROM referral_progress WHERE invited_user=$1',[address(2)])).rows[0]}
  const sources=['src/adapters/communities-db.ts','src/adapters/referral-db.ts','src/logic/referral/referral.ts','src/utils/client-ip.ts','node_modules/@dcl/sns-component/dist/src/component.js']
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},node:process.version,scope:'Actual pinned adapters and referral component on PostgreSQL TEMP relations. Provider failures and independent Promise.all operations use explicit barriers; SNS/rewards/email/Slack are controlled in-process implementations. Member exclusion captures the source ANY bug; it is not a source claim that ALL is equivalent. Signed-up adapter edge captures unguarded source SQL, not a full concurrent updateProgress proof.',sources:sources.map(path=>({path,sha256:crypto.createHash('sha256').update(fs.readFileSync(upstream+'/'+path)).digest('hex')})),membership,failures,signed_up_adapter},null,2)+'\n')
 } finally { await client.end() }
}
main().catch(error=>{console.error(error);process.exitCode=1})
