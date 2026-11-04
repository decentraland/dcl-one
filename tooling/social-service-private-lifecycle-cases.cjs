#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/private-lifecycle-source-cases.json
// Run: node this-file <clean-social-pin> <output.json> <clean-companion-pin>.
// Original source queries use an owned PostgreSQL database; no media egress.
const { requireNode26, sourceCapture, sha256, requireDatabaseUrl, GATEKEEPER_REVISION: companionRevision } = require('./social-capture-common.cjs')
requireNode26()
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: node tooling/social-service-private-lifecycle-cases.cjs <upstream-checkout> <output.json> <companion-checkout>')
const databaseUrl = requireDatabaseUrl(process.env.CATALYST_TEST_DATABASE_URL)
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm')
const { randomUUID } = require('node:crypto')
const companion = process.argv[4]
const { req, load, revision, upstream } = sourceCapture(process.argv[2], { companions: [{ root: companion, kind: 'gatekeeper' }] })
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const caller = '0x1111111111111111111111111111111111111111'
const target = '0x2222222222222222222222222222222222222222'
const ts = req('typescript'), cache = new Map()
// Compile original DB/enum/error source. The LiveKit constant dependency is
// read from its original source; no media transport module is instantiated.
function original(file) {
 file = path.resolve(file)
 if(cache.has(file))return cache.get(file)
 if(file.endsWith('/adapters/livekit.ts')) {
  const match=fs.readFileSync(file,'utf8').match(/export const COMMUNITY_VOICE_CHAT_ROOM_PREFIX = '([^']+)'/)
  return {COMMUNITY_VOICE_CHAT_ROOM_PREFIX:match[1]}
 }
 const module={exports:{}}; cache.set(file,module.exports)
 const code=ts.transpileModule(fs.readFileSync(file,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText
 vm.runInThisContext('(function(require,module,exports){'+code+'\n})',{filename:file})(name=>name.startsWith('.')?original(path.resolve(path.dirname(file),name+'.ts')):req(name),module,module.exports)
 return module.exports
}
async function main(){
 const {Pool}=req('pg'), url=new URL(databaseUrl), name='social_stale_'+process.pid+'_'+randomUUID().replaceAll('-','')
 const admin=new Pool({connectionString:url.toString(),max:1});let pg
 try{
  await admin.query('CREATE DATABASE "'+name+'"'); url.pathname='/'+name;pg=new Pool({connectionString:url.toString(),max:2})
  await pg.query('CREATE TABLE private_voice_chats(id uuid primary key,caller_address text,callee_address text,created_at timestamp default now()); CREATE TABLE voice_chat_users(address varchar(42),room_name varchar,status varchar,joined_at bigint,status_updated_at bigint,PRIMARY KEY(address,room_name))')
  pg.exists=async(q,f)=>(await pg.query(q)).rows[0][f]
  pg.withTransaction=async fn=>{const client=await pg.connect();try{await client.query('BEGIN');const value=await fn(client);await client.query('COMMIT');return value}catch(e){await client.query('ROLLBACK');throw e}finally{client.release()}}
  const companionDb=await original(path.join(companion,'src/adapters/db/voice-db.ts')).createVoiceDBComponent({database:{query:q=>pg.query(q),getPool:()=>pg},logs,config:{requireNumber:async k=>k.endsWith('BATCH_SIZE')?50:300000},livekit:{}})
  const voiceDb=await load('adapters/voice-db.ts').createVoiceDBComponent({pg,config:{requireNumber:async()=>60000}})
  const entries=[]
  for(const [status,stale] of [['connected',false],['not_connected',false],['not_connected',true],['connection_interrupted',false],['connection_interrupted',true],['disconnected',false]]){
   await pg.query('TRUNCATE private_voice_chats,voice_chat_users')
   const room='voice-chat-private-c6666666-6666-4666-8666-666666666666',now=Date.now()-(stale?3600000:0)
   await pg.query('INSERT INTO voice_chat_users VALUES($1,$3,$4,$5,$5),($2,$3,$4,$5,$5)',[caller,target,room,status,now])
   const online=await companionDb.getRoomUserIsIn(caller)
   const voice=await load('logic/voice/voice.ts').createVoiceComponent({logs,config:{requireNumber:async()=>60000},voiceDb,analytics:{fireEvent(){}},friendsDb:{isFriendshipBlocked:async()=>false},settings:{getUsersSettings:async()=>[{private_messages_privacy:'all'},{private_messages_privacy:'all'}]},pubsub:{publishInChannel:async()=>{}},commsGatekeeper:{isUserInCommunityVoiceChat:async()=>false,isUserInAVoiceChat:async a=>(await companionDb.getRoomUserIsIn(a))!==null}})
   let error=null,started=false;try{await voice.startPrivateVoiceChat(caller,target);started=true}catch(e){error=e.message}
   entries.push({status,stale,source_presence:online!==null,started,error,media_rows:Number((await pg.query('SELECT count(*) AS n FROM voice_chat_users')).rows[0].n),pending_rows:Number((await pg.query('SELECT count(*) AS n FROM private_voice_chats')).rows[0].n)})
  }
  const writerSequences=[]
  await pg.query('TRUNCATE private_voice_chats,voice_chat_users')
  const pendingId=await voiceDb.createPrivateVoiceChat(caller,target)
  const third='0x3333333333333333333333333333333333333333'
  await companionDb.createVoiceChatRoom('voice-chat-private-media-one',[third,caller])
  writerSequences.push({scenario:'pending-then-media-other-role',pending_retained:!!(await voiceDb.getPrivateVoiceChat(pendingId)),media_rows:Number((await pg.query('SELECT count(*) AS n FROM voice_chat_users')).rows[0].n)})
  await companionDb.createVoiceChatRoom('voice-chat-private-media-two',[caller,target])
  writerSequences.push({scenario:'two-media-writers-retain-both-rooms',pending_retained:!!(await voiceDb.getPrivateVoiceChat(pendingId)),media_rows:Number((await pg.query('SELECT count(*) AS n FROM voice_chat_users')).rows[0].n),rooms:(await pg.query('SELECT DISTINCT room_name FROM voice_chat_users ORDER BY room_name')).rows.map(r=>r.room_name)})
  fs.writeFileSync(process.argv[3],JSON.stringify({social:revision,companion:companionRevision,sources:[...['src/adapters/voice-db.ts','src/logic/voice/voice.ts'].map(file=>({repository:'decentraland/social-service-ea',file,sha256:sha256(upstream, file)})),...['src/adapters/db/voice-db.ts','src/adapters/db/types.ts','src/adapters/db/errors.ts','src/adapters/livekit.ts','src/logic/voice/voice.ts','src/components.ts'].map(file=>({repository:'decentraland/comms-gatekeeper',file,sha256:sha256(companion, file)}))],scope:'Original Social voice and pending DB + original companion presence and media-write DB on PostgreSQL. Actual initial/interrupted TTL300000; media table retains rows until its minute sweep. No LiveKit transport instantiated; one source constant dependency extracted verbatim.',entries,writerSequences},null,2)+'\n')
 }finally{if(pg)await pg.end();await admin.query('DROP DATABASE IF EXISTS "'+name+'" WITH(FORCE)');await admin.end()}
}
main().catch(e=>{console.error(e);process.exitCode=1})
