#!/usr/bin/env node
// Actual source registry cache semantics and RPC DTO parsers; no network calls.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto')
const{execFileSync}=require('node:child_process'),{createRequire}=require('node:module')
const revision='b6b2cefefdf727b2ed47ddb887e77d226a41eea6',upstream=path.resolve(process.argv[2]||''),output=process.argv[3]
if(!process.argv[2]||!output)throw Error('Usage: node tooling/social-service-profile-cases.cjs <upstream> <output.json>')
const git=(...args)=>execFileSync('git',['-C',upstream,...args],{encoding:'utf8'}).trim()
if(git('rev-parse','HEAD')!==revision||git('status','--porcelain','--untracked-files=no'))throw Error('Pinned clean upstream required')
const from=createRequire(path.join(upstream,'package.json'));from('ts-node').register({transpileOnly:true,project:path.join(upstream,'tsconfig.json')})
const load=file=>from(path.join(upstream,'src',file)),logs={getLogger:()=>({error(){},warn(){},info(){},debug(){}})}
const proto=from('@dcl/protocol/out-js/decentraland/social_service/v2/social_service_v2.gen')
const caller='0x9111111111111111111111111111111111111111',owner='0x9222222222222222222222222222222222222222',alias='0x9333333333333333333333333333333333333333',second='0x9444444444444444444444444444444444444444'
const profile=userId=>({avatars:[{userId,name:'Source profile',hasClaimedName:true,avatar:{snapshots:{face256:'https://images.example.org/raw-face.png'}}}]})
async function main(){
 const cases=[]
 for(const [id,documents,operations]of[
  ['historical-alias-lists-and-single-cache',{[owner]:profile(alias)},['getFriends','getFriends','getMutualFriends','getOwnerName','getOwnerNames','blockUser','getFriends','getBlockedUsers','getPendingFriendshipRequests','getSentFriendshipRequests','unblockUser']],
  ['regular-positive-profiles',{[owner]:profile(owner)},['getFriends','getOwnerName','getOwnerNames','blockUser','getBlockedUsers','getPendingFriendshipRequests','getSentFriendshipRequests','unblockUser']],
  ['cached-profiles-precede-newly-fetched-profiles',{[owner]:profile(owner),[second]:profile(second)},['getFriends','getFriendsMixed']],
  ['missing-user-id-remains-unavailable',{[owner]:{avatars:[{name:'Invalid',avatar:{snapshots:{face256:'https://images.example.org/invalid.png'}}}]}},['getFriends','blockUser','getOwnerName']],
  ['missing-profile-owner',{},['getOwnerName','getOwnerNames']],
  ['cache-write-failure',{[owner]:profile(owner),[second]:profile(second)},['getFriendsMixed','getFriendsReverse','getOwnerName']]
 ]){
  const cache=new Map();let fetches=[]
  const createRegistry=async()=>load('adapters/registry.ts').createRegistryComponent({logs,config:{requireString:async()=> 'https://registry.example.org'},redis:{get:async key=>cache.get(key),mGet:async keys=>keys.map(key=>cache.get(key)),put:async(key,value)=>{if(id==='cache-write-failure')throw Error('fixture cache write failure');cache.set(key,value)}},fetcher:{fetch:async(url,options)=>{const ids=JSON.parse(options.body).ids;fetches.push(ids);return{ok:true,json:async()=>ids.flatMap(id=>documents[id]?[documents[id]]:[])}}}})
  const registries=[await createRegistry(),await createRegistry()]
  const steps=[]
  for(const [index,operation] of operations.entries()){
   const registry=registries[index%2]
   const targets=operation==='getFriendsMixed'?[second,owner]:operation==='getFriendsReverse'?[owner,second]:[owner]
   if(operation==='getOwnerName'||operation==='getOwnerNames'){
    const owners=load('logic/community/owners.ts').createCommunityOwnersComponent({registry})
    let expected_json,expected_error
    try{expected_json=operation==='getOwnerName'?await owners.getOwnerName(owner):await owners.getOwnersNames([owner])}catch(error){expected_error=error.message}
    steps.push({operation,reader:index%2?'B':'A',targets,expected_json,expected_error,fetches:[...fetches],cache_keys:[...cache.keys()].sort()});continue
   }
   const friends={getFriendsProfiles:async()=>({friendsProfiles:await registry.getProfiles(targets),total:targets.length}),getMutualFriendsProfiles:async()=>({friendsProfiles:await registry.getProfiles(targets),total:targets.length}),getBlockedUsers:async()=>({blockedUsers:[{address:owner,blocked_at:new Date(0)}],blockedProfiles:await registry.getProfiles([owner]),total:1}),getSentFriendshipRequests:async()=>({requests:[{id:'00000000-0000-0000-0000-000000000001',address:owner,timestamp:new Date(0)}],profiles:await registry.getProfiles([owner]),total:1}),getPendingFriendshipRequests:async()=>({requests:[{id:'00000000-0000-0000-0000-000000000001',address:owner,timestamp:new Date(0)}],profiles:await registry.getProfiles([owner]),total:1}),blockUser:async()=>{let profile;try{profile=await registry.getProfile(owner)}catch{}return{profile,blockedAt:new Date(0)}},unblockUser:async()=>{let profile;try{profile=await registry.getProfile(owner)}catch{}return profile}}
   const [file,factory,type]=operation==='getMutualFriends'?['get-mutual-friends','getMutualFriendsService','PaginatedFriendsProfilesResponse']:operation==='getSentFriendshipRequests'?['get-sent-friendship-requests','getSentFriendshipRequestsService','PaginatedFriendshipRequestsResponse']:operation.startsWith('getFriends')?['get-friends','getFriendsService','PaginatedFriendsProfilesResponse']:operation==='blockUser'?['block-user','blockUserService','BlockUserResponse']:operation==='unblockUser'?['unblock-user','unblockUserService','UnblockUserResponse']:operation==='getBlockedUsers'?['get-blocked-users','getBlockedUsersService','GetBlockedUsersResponse']:['get-pending-friendship-requests','getPendingFriendshipRequestsService','PaginatedFriendshipRequestsResponse']
   const handler=load(`controllers/handlers/rpc/${file}.ts`)[factory]({components:{logs,friends}})
   const request={user:{address:operation==='getMutualFriends'?second:owner},pagination:{limit:100,offset:0}}
   const response=await handler(request,{address:caller})
   steps.push({operation,reader:index%2?'B':'A',targets,response,expected_hex:Buffer.from(proto[type].encode(response).finish()).toString('hex'),fetches:[...fetches],cache_keys:[...cache.keys()].sort()})
  }
  cases.push({id,documents,steps})
 }
 fs.writeFileSync(output,JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,yarn_lock_sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,'yarn.lock'))).digest('hex'),sources:['src/adapters/registry.ts','src/logic/profiles.ts','src/logic/friends/friends.ts','src/logic/friends/blocks.ts','src/logic/friends/friendships.ts','src/logic/community/owners.ts',...['get-friends','get-mutual-friends','get-blocked-users','get-pending-friendship-requests','get-sent-friendship-requests','block-user','unblock-user'].map(name=>`src/controllers/handlers/rpc/${name}.ts`)].map(file=>({path:file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')}))},runtime:{node:process.version},scope:'Two actual source registry components share the injected Redis cache and alternate A/B/A lookups, plus real RPC handlers/DTO parsers; friendship/block database dependencies are injected. Captures public profile aliases, source single-vs-list cache key behavior, cached+fetched duplicates and positive normal profiles. Does not alter/authenticate wallet identities.',caller,owner,alias,second,cases},null,2)+'\n')
 process.stdout.write(`Captured ${cases.length} profile scenarios\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
