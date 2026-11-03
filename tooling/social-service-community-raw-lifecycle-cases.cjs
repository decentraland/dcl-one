#!/usr/bin/env node
// Run: node this-file <clean-social-pin> <output.json>.
// Cache component rebuilds share a controlled, JSON-serializing Redis facade.
const fs=require('node:fs'), path=require('node:path'), {createHash}=require('node:crypto')
const {req,load,logs,caller,target,revision,upstream}=require('./social-service-voice-source-cases.cjs')
const id='a3333333-b333-4c33-8d33-e33333333333', upper=id.toUpperCase(), guest='0x5555555555555555555555555555555555555555'
const proto=req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const mapper=load('controllers/handlers/uws/stream-registry.ts').toCommunityVoiceChatStreamEvent
const rpcFactory=load('controllers/handlers/rpc/subscribe-to-community-voice-chat-updates.ts').subscribeToCommunityVoiceChatUpdatesService
async function pollingJobTiming(){
 const key='COMMUNITY_VOICE_CHAT_POLLING_JOB_INTERVAL'
 const interval=Number(fs.readFileSync(path.join(upstream,'.env.default'),'utf8').match(new RegExp(`^${key}='([0-9]+)'`,'m'))[1])
 const saved={setTimeout:global.setTimeout,clearTimeout:global.clearTimeout},timers=[];let runs=0
 try{
  global.setTimeout=(callback,delay)=>{const timer={callback,delay};timers.push(timer);return timer}
  global.clearTimeout=timer=>{timer.cancelled=true}
  const {START_COMPONENT,STOP_COMPONENT}=req('@well-known-components/interfaces')
  const job=req('@dcl/job-component').createJobComponent({logs},async()=>{runs++},interval,{repeat:true})
  await job[START_COMPONENT]();const before=runs
  timers[0].callback();await new Promise(setImmediate)
  const after=runs;await job[STOP_COMPONENT]()
  return{interval_ms:interval,initial_delay_ms:timers[0].delay,after_first_run_delay_ms:timers[1].delay,callback_runs_before_timer:before,callback_runs_after_timer:after,sdk:{name:'@dcl/job-component',version:req('@dcl/job-component/package.json').version,files:['dist/src/component.js','dist/src/runner.js'].map(file=>({file,sha256:createHash('sha256').update(fs.readFileSync(path.join(path.dirname(req.resolve('@dcl/job-component/package.json')),file))).digest('hex')}))}}
 }finally{Object.assign(global,saved)}
}
async function main(){
 const oldNow=Date.now;let now=1234567890000;Date.now=()=>now
 const cases=[]
 try{for(const scenario of ['uppercase-start-lowercase-end','two-alias-cache-entries','aliases-own-scope-and-expiry','explicit-end-restart-keeps-other-alias']){
  now=1234567890000
  const values=new Map(), expiry=new Map(), keys=[], emitted=[], published=[], broadcasts=[], sns=[], http=[], reads=[];let active=false, privateCommunity=false, phase='start-upper'
  const value=k=>{if((expiry.get(k)??Infinity)<=now){values.delete(k);expiry.delete(k)}return structuredClone(values.get(k)??null)}
  const redis={put:async(k,v,o)=>{values.set(k,structuredClone(v));expiry.set(k,now+o.EX*1000);keys.push({op:'put',key:k,ttl:o.EX})},get:async k=>value(k),mGet:async ks=>ks.map(k=>value(k)),client:{del:async k=>{values.delete(k);expiry.delete(k);keys.push({op:'del',key:k})},async *scanIterator(){for(const k of [...values.keys()])if(value(k))yield k}}}
  const cache=()=>load('logic/community-voice/community-voice-cache.ts').createCommunityVoiceChatCacheComponent({logs,redis})
  let communityVoiceChatCache=cache()
  const db={getCommunity:async()=>({id,name:'Community',privacy:privateCommunity?'private':'public',visibility:'all',role:'owner'}),getCommunityMemberRole:async()=> 'owner',isMemberBanned:async()=>false,getCommunityPlaces:async()=>[]}
  const updateHandler=load('logic/updates.ts').createUpdateHandlerComponent({logs,subscribersContext:{getLocalSubscribersAddresses:()=>[target,guest],getSubscriber:address=>({emit:(_event,update)=>emitted.push({phase,address,update:structuredClone(update)})})},communitiesDb:db,communityMembers:{async *getOnlineMembersFromCommunity(){yield[{memberAddress:target}]}},friendsDb:{},registry:{},metrics:{},peersStats:{}})
  const pubsub={publishInChannel:async(channel,update)=>{published.push(structuredClone(update));await updateHandler.communityVoiceChatUpdateHandler(JSON.stringify(update))}}
  const commsGatekeeper=await load('adapters/comms-gatekeeper.ts').createCommsGatekeeperComponent({logs,config:{requireString:async k=>k==='COMMS_GATEKEEPER_URL'?'https://gatekeeper.example':'fixture-token'},fetcher:{fetch:async(url,options)=>{
   const route=new URL(url).pathname,body=options.body?JSON.parse(options.body):undefined;http.push({method:options.method,path:route,...body&&{body}})
   if(options.method==='GET')return new Response(JSON.stringify({active,participant_count:active?1:0,moderator_count:active?1:0}))
   if(options.method==='DELETE'){active=false;return new Response(null,{status:204})}
   active=true;return new Response(JSON.stringify({connection_url:'community-grant'}))
  }}})
  db.getCommunityMembers=async(_id,options)=>[caller,target,'0x6666666666666666666666666666666666666666'].filter(address=>(!options.filterByMembers||options.filterByMembers.includes(address))&&!(options.excludedAddresses??[]).includes(address)).map(memberAddress=>({memberAddress}))
  const broadcaster=load('logic/community/broadcaster.ts').createCommunityBroadcasterComponent({logs,communitiesDb:db,peersStats:{getConnectedPeers:async()=>[caller,target,guest]},sns:{publishMessages:async messages=>{sns.push(...structuredClone(messages));return[]}}})
  const domain=()=>load('logic/community-voice/community-voice.ts').createCommunityVoiceComponent({logs,communitiesDb:db,commsGatekeeper,pubsub,analytics:{fireEvent(){}},registry:{getProfile:async()=>{throw Error('no profile')}},communityThumbnail:{getThumbnail:async()=>null},communityPlaces:{},placesApi:{},communityBroadcaster:{broadcast:async(event,options)=>{broadcasts.push({event:structuredClone(event),options:structuredClone(options)});await broadcaster.broadcast(event,options)}},communityVoiceChatCache})
  const poll=()=>load('logic/community-voice/community-voice-polling.ts').createCommunityVoiceChatPollingComponent({logs,commsGatekeeper,pubsub,communityVoiceChatCache})
  await (await domain()).startCommunityVoiceChat(upper,caller)
  communityVoiceChatCache=cache();reads.push({step:'restart-after-start',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  phase='active-poll';await poll().checkAllVoiceChats()
  if(scenario==='uppercase-start-lowercase-end'){
   phase='end-lower';await (await domain()).endCommunityVoiceChat(id,caller)
   communityVoiceChatCache=cache();reads.push({step:'restart-after-end-other-spelling',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  }else{
   if(scenario!=='two-alias-cache-entries'){now+=1000;privateCommunity=true}
   phase='start-lower';active=false;await (await domain()).startCommunityVoiceChat(id,caller)
   communityVoiceChatCache=cache();reads.push({step:'restart-two-aliases',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
   if(scenario!=='aliases-own-scope-and-expiry'){phase='active-sibling-poll';await poll().checkAllVoiceChats();reads.push({step:'after-active-sibling-poll',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})}
   if(scenario==='aliases-own-scope-and-expiry'){
    now=1234567890000+24*60*60*1000+500
    reads.push({step:'upper-expired-lower-fresh',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
   }
   if(scenario==='explicit-end-restart-keeps-other-alias'){
    phase='end-upper';await (await domain()).endCommunityVoiceChat(upper,caller)
    reads.push({step:'after-upper-end',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
    now+=1000;phase='restart-upper';await (await domain()).startCommunityVoiceChat(upper,caller)
    communityVoiceChatCache=cache();reads.push({step:'restart-after-new-upper-start',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
   }
   active=false
   if(scenario==='explicit-end-restart-keeps-other-alias')reads.push({step:'media-closed-before-poll',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats(),publishedCount:published.length})
  }
  phase='inactive-poll';await poll().checkAllVoiceChats()
  reads.push({step:'after-inactive-poll',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  const wire=[]
  for(const {phase,address,update}of emitted){
   const service=rpcFactory({components:{logs,updateHandler:{async *handleSubscriptionUpdates({parser}){yield parser(update)}}}})
   const generator=service({},{}), item=await generator.next();await generator.return()
   wire.push({phase,address,rpc:item.value,rpc_hex:Buffer.from(proto.CommunityVoiceChatUpdate.encode(item.value).finish()).toString('hex'),json:mapper(update)})
  }
  cases.push({scenario,http,keys,reads,published,broadcasts,sns,wire})
 }}finally{Date.now=oldNow}
 const polling_job=await pollingJobTiming()
 fs.writeFileSync(process.argv[3],JSON.stringify({revision,polling_job,sources:['.env.default','src/components.ts','src/logic/community-voice/community-voice.ts','src/logic/community/broadcaster.ts','src/logic/community-voice/community-voice-cache.ts','src/logic/community-voice/community-voice-polling.ts','src/adapters/comms-gatekeeper.ts','src/utils/uuid.ts','src/logic/updates.ts','src/controllers/handlers/rpc/subscribe-to-community-voice-chat-updates.ts','src/controllers/handlers/uws/stream-registry.ts'].map(file=>({repository:'decentraland/social-service-ea',file,sha256:createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')})),scope:'Original community domain/broadcast invocation, cache, poller, privileged adapter identifier guards, update audience handler and both client parsers; deterministic JSON-serializing Redis facade shared across component rebuilds; controlled media HTTP responses and clock with real24h facade expiry. No network Redis/LiveKit used.',cases},null,2)+'\n')
}
main().catch(e=>{console.error(e);process.exitCode=1})
