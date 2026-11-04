#!/usr/bin/env node
// Execute the actual source notification creator/retrier with a local publisher.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto')
const{execFileSync}=require('node:child_process'),{createRequire}=require('node:module')
const revision='b6b2cefefdf727b2ed47ddb887e77d226a41eea6',upstream=path.resolve(process.argv[2]||''),output=process.argv[3]
if(!process.argv[2]||!output)throw Error('Usage: node tooling/social-service-notification-cases.cjs <upstream> <output.json>')
const git=(...args)=>execFileSync('git',['-C',upstream,...args],{encoding:'utf8'}).trim()
if(git('rev-parse','HEAD')!==revision||git('status','--porcelain','--untracked-files=no'))throw Error('Pinned clean upstream required')
const from=createRequire(path.join(upstream,'package.json'));from('ts-node').register({transpileOnly:true,project:path.join(upstream,'tsconfig.json')})
const load=file=>from(path.join(upstream,'src',file))
let waits=[];load('utils/timer.ts').sleep=async ms=>{waits.push(ms)}
let clock=1770000000000;Date.now=()=>clock++
const sender='0x9511111111111111111111111111111111111111',receiver='0x9622222222222222222222222222222222222222'
const profile=(address,name)=>({avatars:[{userId:address,name,hasClaimedName:true,avatar:{snapshots:{face256:`https://images.example.org/${name}.png`}}}]})
const context={requestId:'00000000-0000-0000-0000-000000000099',senderAddress:sender,receiverAddress:receiver,senderProfile:profile(sender,'Alice'),receiverProfile:profile(receiver,'Bob')}
async function main(){
 const cases=[]
 for(const [id,action,failures,message]of[['request','request',0,'hello'],['accepted','accept',0],['retry-success','request',2,'hello'],['retry-exhausted','request',3,'hello']]){
  waits=[];const events=[];const logs=[]
  await load('logic/notifications.ts').sendNotification(action,{...context,...(message?{message}:{})},{sns:{publishMessage:async event=>{events.push(event);if(events.length<=failures)throw Error('fixture rejection')}},logs:{getLogger:()=>({warn:message=>logs.push(message),error:message=>logs.push(message)})}})
  cases.push({id,action,failures,message,events,waits:[...waits],logs})
 }
 const sources=['src/logic/notifications.ts','src/utils/retrier.ts','src/logic/profiles.ts']
 fs.writeFileSync(output,JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,yarn_lock_sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,'yarn.lock'))).digest('hex'),sources:sources.map(file=>({path:file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')}))},runtime:{node:process.version},scope:'Actual pinned notification event creator and three-attempt retrier; injected SNS component, clock and sleep. SDK HTTP retries and deployed topic policies are outside this fixture.',context,cases},null,2)+'\n')
 process.stdout.write(`Captured ${cases.length} notification scenarios\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
