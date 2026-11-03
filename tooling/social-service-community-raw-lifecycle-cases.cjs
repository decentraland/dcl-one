#!/usr/bin/env node
// Run: node this-file <clean-social-pin> <output.json>.
// Cache component rebuilds share a controlled, JSON-serializing Redis facade.
const fs=require('node:fs'), path=require('node:path'), {createHash}=require('node:crypto')
const {load,logs,caller,target,revision,upstream}=require('./social-service-voice-source-cases.cjs')
const id='a3333333-b333-4c33-8d33-e33333333333', upper=id.toUpperCase(), guest='0x5555555555555555555555555555555555555555'
const mapper=load('controllers/handlers/uws/stream-registry.ts').toCommunityVoiceChatStreamEvent
const rpcFactory=load('controllers/handlers/rpc/subscribe-to-community-voice-chat-updates.ts').subscribeToCommunityVoiceChatUpdatesService
async function main(){
 const oldNow=Date.now;Date.now=()=>1234567890000
 const cases=[]
 try{for(const scenario of ['uppercase-start-lowercase-end','two-alias-cache-entries']){
  const values=new Map(), keys=[], emitted=[], published=[], http=[], reads=[];let active=false
  const redis={put:async(k,v,o)=>{values.set(k,structuredClone(v));keys.push({op:'put',key:k,ttl:o.EX})},get:async k=>structuredClone(values.get(k)??null),mGet:async ks=>ks.map(k=>structuredClone(values.get(k))),client:{del:async k=>{values.delete(k);keys.push({op:'del',key:k})},async *scanIterator(){yield* values.keys()}}}
  const cache=()=>load('logic/community-voice/community-voice-cache.ts').createCommunityVoiceChatCacheComponent({logs,redis})
  let communityVoiceChatCache=cache()
  const db={getCommunity:async()=>({id,name:'Community',privacy:'public',visibility:'all',role:'owner'}),getCommunityMemberRole:async()=> 'owner',isMemberBanned:async()=>false,getCommunityPlaces:async()=>[]}
  const updateHandler=load('logic/updates.ts').createUpdateHandlerComponent({logs,subscribersContext:{getLocalSubscribersAddresses:()=>[target,guest],getSubscriber:address=>({emit:(_event,update)=>emitted.push({address,update:structuredClone(update)})})},communitiesDb:db,communityMembers:{async *getOnlineMembersFromCommunity(){yield[{memberAddress:target}]}},friendsDb:{},registry:{},metrics:{},peersStats:{}})
  const pubsub={publishInChannel:async(channel,update)=>{published.push(structuredClone(update));await updateHandler.communityVoiceChatUpdateHandler(JSON.stringify(update))}}
  const commsGatekeeper=await load('adapters/comms-gatekeeper.ts').createCommsGatekeeperComponent({logs,config:{requireString:async k=>k==='COMMS_GATEKEEPER_URL'?'https://gatekeeper.example':'fixture-token'},fetcher:{fetch:async(url,options)=>{
   const route=new URL(url).pathname,body=options.body?JSON.parse(options.body):undefined;http.push({method:options.method,path:route,...body&&{body}})
   if(options.method==='GET')return new Response(JSON.stringify({active,participant_count:active?1:0,moderator_count:active?1:0}))
   if(options.method==='DELETE'){active=false;return new Response(null,{status:204})}
   active=true;return new Response(JSON.stringify({connection_url:'community-grant'}))
  }}})
  const domain=()=>load('logic/community-voice/community-voice.ts').createCommunityVoiceComponent({logs,communitiesDb:db,commsGatekeeper,pubsub,analytics:{fireEvent(){}},registry:{getProfile:async()=>{throw Error('no profile')}},communityThumbnail:{getThumbnail:async()=>null},communityPlaces:{},placesApi:{},communityBroadcaster:{broadcast:async()=>{}},communityVoiceChatCache})
  const poll=()=>load('logic/community-voice/community-voice-polling.ts').createCommunityVoiceChatPollingComponent({logs,commsGatekeeper,pubsub,communityVoiceChatCache})
  await (await domain()).startCommunityVoiceChat(upper,caller)
  communityVoiceChatCache=cache();reads.push({step:'restart-after-start',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  await poll().checkAllVoiceChats()
  if(scenario==='uppercase-start-lowercase-end'){
   await (await domain()).endCommunityVoiceChat(id,caller)
   communityVoiceChatCache=cache();reads.push({step:'restart-after-end-other-spelling',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  }else{
   active=false;await (await domain()).startCommunityVoiceChat(id,caller)
   communityVoiceChatCache=cache();reads.push({step:'restart-two-aliases',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()});active=false
  }
  await poll().checkAllVoiceChats()
  reads.push({step:'after-inactive-poll',entries:await communityVoiceChatCache.getActiveCommunityVoiceChats()})
  const wire=[]
  for(const {address,update}of emitted){
   const service=rpcFactory({components:{logs,updateHandler:{async *handleSubscriptionUpdates({parser}){yield parser(update)}}}})
   const generator=service({},{}), item=await generator.next();await generator.return()
   wire.push({address,rpc:item.value,json:mapper(update)})
  }
  cases.push({scenario,http,keys,reads,published,wire})
 }}finally{Date.now=oldNow}
 fs.writeFileSync(process.argv[3],JSON.stringify({revision,sources:['src/logic/community-voice/community-voice.ts','src/logic/community-voice/community-voice-cache.ts','src/logic/community-voice/community-voice-polling.ts','src/adapters/comms-gatekeeper.ts','src/utils/uuid.ts','src/logic/updates.ts','src/controllers/handlers/rpc/subscribe-to-community-voice-chat-updates.ts','src/controllers/handlers/uws/stream-registry.ts'].map(file=>({repository:'decentraland/social-service-ea',file,sha256:createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')})),scope:'Original community domain, cache, poller, privileged adapter identifier guards, update audience handler and both client parsers; deterministic JSON-serializing Redis facade shared across component rebuilds; controlled media HTTP responses and clock. No network Redis/LiveKit used.',cases},null,2)+'\n')
}
main().catch(e=>{console.error(e);process.exitCode=1})
