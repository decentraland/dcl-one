#!/usr/bin/env node
// Run: node this-file <clean-social-pin> <output.json> <clean-companion-pin>.
// Original source queries use an owned PostgreSQL database; no media egress.
const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm')
const { execFileSync } = require('node:child_process')
const { randomUUID, createHash } = require('node:crypto')
const { req, load, logs, caller, target, revision, upstream } = require('./social-service-voice-source-cases.cjs')
const companion = process.argv[4]
const companionRevision = 'be6a4b97a2189157884503485892ff46d0ac68da'
const git = (...args) => execFileSync('git', ['-C', companion, ...args], {encoding:'utf8'}).trim()
if(git('rev-parse','HEAD') !== companionRevision || git('status','--porcelain','--untracked-files=no')) throw Error('Expected clean companion pin')
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
 const {Pool}=req('pg'), url=new URL(process.env.CATALYST_TEST_DATABASE_URL), name='social_stale_'+process.pid+'_'+randomUUID().replaceAll('-','')
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
  fs.writeFileSync(process.argv[3],JSON.stringify({social:revision,companion:companionRevision,sources:[...['src/adapters/voice-db.ts','src/logic/voice/voice.ts'].map(file=>({repository:'decentraland/social-service-ea',file,sha256:createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')})),...['src/adapters/db/voice-db.ts','src/adapters/db/types.ts','src/adapters/db/errors.ts','src/adapters/livekit.ts','src/logic/voice/voice.ts','src/components.ts'].map(file=>({repository:'decentraland/comms-gatekeeper',file,sha256:createHash('sha256').update(fs.readFileSync(path.join(companion,file))).digest('hex')}))],scope:'Original Social voice and pending DB + original companion presence DB on PostgreSQL. Actual initial/interrupted TTL300000; media table retains rows until its minute sweep. No LiveKit transport instantiated; one source constant dependency extracted verbatim.',entries},null,2)+'\n')
 }finally{if(pg)await pg.end();await admin.query('DROP DATABASE IF EXISTS "'+name+'" WITH(FORCE)');await admin.end()}
}
main().catch(e=>{console.error(e);process.exitCode=1})
