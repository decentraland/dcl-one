#!/usr/bin/env node
// Output fixture: services/catalyst/tests/fixtures/conformance/social/source-contracts/voice-membership.json
// Actual source update handler and membership generator; their database page
// provider is injected to fail on the second read. No network calls.
const { requireNode26, sourceCapture, sha256 } = require('./social-capture-common.cjs')
requireNode26()
if (!process.argv[2] || !process.argv[3]) throw Error('Usage: node tooling/social-service-voice-membership-cases.cjs <upstream-checkout> <output.json>')
const fs = require('node:fs')
const { req, load, revision, upstream } = sourceCapture(process.argv[2])
const logs = { getLogger: () => ({ error() {}, warn() {}, info() {}, debug() {} }) }
const proto = req('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const community = '31333333-3333-4333-8333-333333333333'
const subscribers = Array.from({length:514}, (_, index) => `0x${(index+1).toString(16).padStart(40,'0')}`)
const creator = subscribers[0].toUpperCase()
async function main() {
  const cases = []
  for (const config of [
    {id:'members-start-late-failure',ended:false,notification_scope:'all',private:true,fail_at:2},
    {id:'public-start-late-failure',ended:false,notification_scope:'members',private:false,fail_at:2},
    {id:'members-end-late-failure',ended:true,notification_scope:'members',private:false,fail_at:2},
    {id:'public-end-unavailable-membership',ended:true,notification_scope:'all',private:true,fail_at:1},
    {id:'public-start-healthy',ended:false,notification_scope:'members',private:false,fail_at:0},
    {id:'members-end-healthy',ended:true,notification_scope:'members',private:false,fail_at:0}
  ]) {
    const emitted = [], member_reads = []
    let scope_reads = 0
    const communitiesDb = {
      getCommunity:async()=>{scope_reads++;return {privacy:config.private?'private':'public',visibility:'all'}},
      getCommunityMembers:async(_,options)=>{
        member_reads.push({offset:options.pagination.offset,limit:options.pagination.limit,filter_count:options.filterByMembers.length,
          includes_creator:options.filterByMembers.includes(creator.toLowerCase()),emitted_before_read:emitted.length})
        if (member_reads.length === config.fail_at) throw Error('fixture later membership page failure')
        return options.filterByMembers.slice(options.pagination.offset,options.pagination.offset+options.pagination.limit).map(memberAddress=>({memberAddress}))
      }
    }
    const communityMembers = await load('logic/community/members.ts').createCommunityMembersComponent({logs,communitiesDb})
    const subscribersContext = {
      getLocalSubscribersAddresses:()=>subscribers,
      getSubscriber:address=>({emit:(name,update)=>{
        if(name!=='communityVoiceChatUpdate')throw Error('unexpected event')
        emitted.push({address,wire_hex:Buffer.from(proto.CommunityVoiceChatUpdate.encode(proto.CommunityVoiceChatUpdate.fromPartial(update)).finish()).toString('hex')})
      }})
    }
    const handler = load('logic/updates.ts').createUpdateHandlerComponent({logs,subscribersContext,communitiesDb,communityMembers})
    const update = {communityId:community,createdAt:1234,status:config.ended?proto.CommunityVoiceChatStatus.COMMUNITY_VOICE_CHAT_ENDED:proto.CommunityVoiceChatStatus.COMMUNITY_VOICE_CHAT_STARTED,
      ...(config.ended?{endedAt:4321}:{positions:['1,2'],worlds:['example.dcl.eth'],communityName:'Membership boundary',communityImage:'https://images.example.org/community.png'}),
      creatorAddress:creator,notificationScope:config.notification_scope}
    await handler.communityVoiceChatUpdateHandler(JSON.stringify(update))
    const groups = new Map()
    for(const event of emitted){if(!groups.has(event.wire_hex))groups.set(event.wire_hex,[]);groups.get(event.wire_hex).push(event.address)}
    cases.push({...config,scope_reads,member_reads,input_hex:Buffer.from(proto.CommunityVoiceChatUpdate.encode(proto.CommunityVoiceChatUpdate.fromPartial(update)).finish()).toString('hex'),
      emissions:[...groups].sort(([a],[b])=>a.localeCompare(b)).map(([wire_hex,addresses])=>({wire_hex,addresses:addresses.sort()}))})
  }
  const sources=['src/logic/updates.ts','src/logic/community/members.ts']
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,yarn_lock_sha256:sha256(upstream, 'yarn.lock'),
    sources:sources.map(file=>({path:file,sha256:sha256(upstream, file)}))},runtime:{node:process.version},
    scope:'Actual source update handler plus actual 100-row community membership generator. The database page provider is injected with 514 known members and a selected failure. Captures complete recipient/wire payload groups and emission counts before every source read. No deployed Redis/SQL/NATS equivalence or identical physical page size is claimed.',community,creator,subscribers,cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} membership-boundary source cases for ${subscribers.length-1} eligible subscribers\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
