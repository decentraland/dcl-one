#!/usr/bin/env node
// Actual pinned HTTP controllers, schema validator and both error middlewares;
// community domain is real with deterministic dependencies, private outcomes injected.
const fs = require('node:fs')
const { req, load, logs, caller, target, community, revision, components } = require('./social-service-voice-source-cases.cjs')
const call = 'a4444444-4444-4444-8444-444444444444'
const schema = load('controllers/handlers/http/schemas.ts')
const validator = req('@dcl/schema-validator-component').createSchemaValidatorComponent({ ensureJsonContentType: false })
const { errorHandler } = req('@dcl/http-commons')
const { voiceChatsErrorsHandler } = load('controllers/middlewares/voice-chats-errors.ts')
const voiceErrors = load('logic/voice/errors.ts')
const url = address => `wss://livekit.test/voice-chat-private-${call}?for=${address}`
const configs = [
  ['private-start','start-private-voice-chat','startPrivateVoiceChatHandler','POST','/v1/private-voice-chats',{}, {calleeAddress:target},'StartPrivateVoiceChatSchema'],
  ['private-incoming','get-incoming-private-voice-chat','getIncomingPrivateVoiceChatHandler','GET','/v1/private-voice-chats/incoming',{},null,null],
  ['private-accept','update-private-voice-chat','updatePrivateVoiceChatHandler','PATCH',`/v1/private-voice-chats/${call}`,{callId:call},{intention:'accepted'},'UpdatePrivateVoiceChatSchema'],
  ['private-reject','update-private-voice-chat','updatePrivateVoiceChatHandler','PATCH',`/v1/private-voice-chats/${call}`,{callId:call},{intention:'rejected'},'UpdatePrivateVoiceChatSchema'],
  ['private-end','end-private-voice-chat','endPrivateVoiceChatHandler','DELETE',`/v1/private-voice-chats/${call}`,{callId:call},null,null],
  ['community-start','start-community-voice-chat','startCommunityVoiceChatHandler','POST','/v1/community-voice-chats',{}, {communityId:community},'StartCommunityVoiceChatSchema'],
  ['community-join','join-community-voice-chat','joinCommunityVoiceChatHandler','POST',`/v1/community-voice-chats/${community}/participants`,{communityId:community},null,null],
  ['community-end','end-community-voice-chat','endCommunityVoiceChatHandler','DELETE',`/v1/community-voice-chats/${community}`,{communityId:community},null,null],
  ['community-request','create-community-voice-chat-speak-request','createCommunityVoiceChatSpeakRequestHandler','POST',`/v1/community-voice-chats/${community}/speak-requests`,{communityId:community},null,null],
  ['community-reject','remove-community-voice-chat-speak-request','removeCommunityVoiceChatSpeakRequestHandler','DELETE',`/v1/community-voice-chats/${community}/speak-requests/${target}`,{communityId:community,address:target},null,null],
  ['community-promote','promote-community-voice-chat-speaker','promoteCommunityVoiceChatSpeakerHandler','PUT',`/v1/community-voice-chats/${community}/speakers/${target}`,{communityId:community,address:target},null,null],
  ['community-demote','demote-community-voice-chat-speaker','demoteCommunityVoiceChatSpeakerHandler','DELETE',`/v1/community-voice-chats/${community}/speakers/${target}`,{communityId:community,address:target},null,null],
  ['community-kick','kick-community-voice-chat-participant','kickCommunityVoiceChatParticipantHandler','DELETE',`/v1/community-voice-chats/${community}/participants/${target}`,{communityId:community,address:target},null,null],
  ['community-mute','mute-community-voice-chat-speaker','muteCommunityVoiceChatSpeakerHandler','PATCH',`/v1/community-voice-chats/${community}/speakers/${target}`,{communityId:community,address:target},{muted:true},'UpdateCommunityVoiceChatSpeakerSchema']
]
async function capture(config, scenario) {
  const [id,file,factory,method,basePath,baseParams,baseBody,schemaName] = config
  let path = basePath, params = {...baseParams}, body = baseBody && {...baseBody}
  const address = id === 'private-accept' || id === 'private-reject' ? target : caller
  if (scenario === 'invalid-id') {
    if (params.communityId) { path = path.replace(community,'invalid'); params.communityId='invalid' }
    if (params.callId) { path = path.replace(call,'invalid'); params.callId='invalid' }
  }
  const spellings = { 'compact-id':call.replaceAll('-',''), 'braced-id':`{${call}}`, 'uppercase-id':call.toUpperCase(),
    'urn-id':`urn:uuid:${call}`, 'four-digit-id':'a444-4444-4444-4444-8444-4444-4444-4444' }
  if (spellings[scenario]) { path=path.replace(call,spellings[scenario]);params.callId=spellings[scenario] }
  if (scenario === 'schema-required') body={}
  if (scenario === 'invalid-target') { path=path.replace(target,'invalid');params.address='invalid' }
  const deps = await components(scenario, id === 'community-start')
  const success = async () => undefined
  deps.voice = {
    startPrivateVoiceChat: async () => call, acceptPrivateVoiceChat: async () => ({connectionUrl:url(address)}),
    rejectPrivateVoiceChat: success, endPrivateVoiceChat: success,
    getIncomingPrivateVoiceChat: async () => ({id:call,caller_address:target})
  }
  if (scenario === 'runtime-failure') for (const method of Object.keys(deps.voice)) deps.voice[method] = async () => { throw Error('voice runtime not configured') }
  if (scenario === 'not-found') for (const method of Object.keys(deps.voice)) deps.voice[method] = async () => {
    if (id === 'private-incoming') throw new voiceErrors.IncomingVoiceChatNotFoundError(address)
    throw new voiceErrors.VoiceChatNotFoundError(call)
  }
  const ctx = { components:{logs,...deps}, params, verification:{auth:address}, url:new URL('https://social.example'+path),
    request:new Request('https://social.example'+path,{method,...(body ? {headers:{'content-type':'application/json'},body:JSON.stringify(body)} : {})}) }
  const controller = load(`controllers/handlers/http/${file}-handler.ts`)[factory]
  const invoke = () => controller(ctx)
  const response = await errorHandler(ctx, () => voiceChatsErrorsHandler(ctx, () =>
    schemaName ? validator.withSchemaValidatorMiddleware(schema[schemaName])(ctx,invoke) : invoke()))
  return { id:`${id}.${scenario}`,operation:id,scenario,method,path,params,body,caller:address,response:{status:response.status,body:response.body??null} }
}
async function main() {
  const cases=[]
  for (const config of configs) {
    cases.push(await capture(config,'success'))
    cases.push(await capture(config,'runtime-failure'))
    if (Object.keys(config[5]).length) cases.push(await capture(config,'invalid-id'))
    if (config[7]) cases.push(await capture(config,'schema-required'))
    if (config[5].callId) for (const spelling of ['compact-id','braced-id','uppercase-id','urn-id','four-digit-id']) cases.push(await capture(config,spelling))
    if (config[5].address) cases.push(await capture(config,'invalid-target'))
    if (config[0].startsWith('private-') && config[0]!=='private-start') cases.push(await capture(config,'not-found'))
    if (config[0].startsWith('community-') && config[0]!=='community-start') cases.push(await capture(config,'runtime-not-found'))
  }
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},
    scope:'Executed actual HTTP controllers, schema middleware and voice/http error middleware; real community domain with deterministic injected dependencies, private voice outcomes injected. 13 unique HTTP operations, including both private PATCH branches; active-community GET is owned separately. Authentication bypassed with explicit verified actor; no live-media claim.',caller,target,community,call,cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} upstream voice HTTP cases\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
