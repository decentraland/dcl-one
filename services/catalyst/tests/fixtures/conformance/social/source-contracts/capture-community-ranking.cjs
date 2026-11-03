// Execute the pinned ranking component without network or a provider.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <source-root> <output-directory>
const fs = require('fs');
const path = require('path');
const upstream = process.argv[2];
const out = process.argv[3];
require(path.join(upstream, 'node_modules/ts-node/register/transpile-only'));
const logger = { info(){}, error(){}, warn(){}, debug(){} };
const logs = { getLogger(){ return logger; } };
const zero = {eventsCount:0,photosCount:0,streamsCount:0,placesCount:0,postsCount:0,newMembersCount:0,eventsTotalAttendees:0,streamsTotalParticipants:0,hasThumbnail:0,hasDescription:0,ageInDays:0};
const cases = [
 {case:'zero', ...zero},
 {case:'source-unit-medium',...zero,eventsCount:2,hasThumbnail:1,photosCount:3,hasDescription:1,placesCount:3,newMembersCount:5,postsCount:8,streamsCount:1,eventsTotalAttendees:50,streamsTotalParticipants:25,ageInDays:5},
 {case:'source-unit-sparse',...zero,eventsCount:1,placesCount:1,newMembersCount:2,postsCount:3,eventsTotalAttendees:10,ageInDays:3},
 ...[-3,7,8,29,30,365].map(ageInDays=>({case:`age-${ageInDays}`,...zero,hasDescription:1,ageInDays})),
 {case:'caps',...zero,eventsCount:50,photosCount:100,streamsCount:20,placesCount:10,postsCount:100,newMembersCount:50,eventsTotalAttendees:1000,streamsTotalParticipants:500,hasThumbnail:1,hasDescription:1},
 {case:'beyond-caps',...zero,eventsCount:5000,photosCount:10000,streamsCount:2000,placesCount:1000,postsCount:10000,newMembersCount:5000,eventsTotalAttendees:100000,streamsTotalParticipants:50000,hasThumbnail:1,hasDescription:1},
 {case:'negative',...zero,eventsCount:-1,photosCount:-1,streamsCount:-1,placesCount:-1,postsCount:-1,newMembersCount:-1,eventsTotalAttendees:-1,streamsTotalParticipants:-1,hasThumbnail:-1,hasDescription:-1}
];
(async()=>{
 let scores;
 const {createCommunityRankingComponent} = require(path.join(upstream,'src/logic/community/ranking.ts'));
 await createCommunityRankingComponent({logs,communitiesDb:{getAllCommunitiesWithRankingMetrics:async({offset})=>offset===0?cases.map((row,i)=>({...row,communityId:String(i)})):[], updateCommunitiesRankingScores:async values=>{scores=values;}}}).calculateRankingScoreForAllCommunities();
 fs.writeFileSync(path.join(out,'community-ranking.json'),JSON.stringify({source:'decentraland/social-service-ea',revision:'b6b2cefefdf727b2ed47ddb887e77d226a41eea6',path:'src/logic/community/ranking.ts',cases:cases.map((row,i)=>({name:row.case,metrics:Object.fromEntries(Object.entries(row).filter(([k])=>k!=='case')),score:scores.get(String(i))}))},null,2)+'\n');
})().catch(error=>{console.error(error);process.exitCode=1;});
