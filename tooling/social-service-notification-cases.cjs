#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/notification-cases.json
// Execute the actual source notification creator/retrier with a local publisher.
const { requireNode26, sourceCapture, sha256 } = require('./social-capture-common.cjs')
requireNode26()
const fs=require('node:fs')
const output = process.argv[3]
if (!process.argv[2] || !output) throw Error('Usage: node tooling/social-service-notification-cases.cjs <upstream> <output.json>')
const { upstream, revision, req: from, load } = sourceCapture(process.argv[2])
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
 fs.writeFileSync(output,JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,yarn_lock_sha256:sha256(upstream, 'yarn.lock'),sources:sources.map(file=>({path:file,sha256:sha256(upstream, file)}))},runtime:{node:process.version},scope:'Actual pinned notification event creator and three-attempt retrier; injected SNS component, clock and sleep. SDK HTTP retries and deployed topic policies are outside this fixture.',context,cases},null,2)+'\n')
 process.stdout.write(`Captured ${cases.length} notification scenarios\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
