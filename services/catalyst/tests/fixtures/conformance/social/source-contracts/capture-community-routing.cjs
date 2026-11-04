// Runs only against the materialized, pinned social-service-ea source.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <source-root> <output-json>
const fs=require('fs');const root=process.argv[2];require(root+'/node_modules/ts-node/register/transpile-only');
const {setupHttpRoutes}=require(root+'/src/controllers/routes/http.routes.ts');
const http=require('http');
const {createServerComponent}=require(root+'/node_modules/@dcl/http-server/dist/server.js');
const {getUnderlyingServer}=require(root+'/node_modules/@dcl/http-server/dist/injectors.js');
(async()=>{
const router=await setupHttpRoutes({components:{config:{getString:async key=>key==='API_ADMIN_TOKEN'?'fixture-admin':undefined},schemaValidator:{withSchemaValidatorMiddleware:()=>async(_ctx,next)=>next()},fetcher:{fetch(){throw Error('no network in route capture');}}}});
// Preserve the registered method/path set while avoiding domain dependencies:
// none of these method cases calls a registered handler.
const logger={info(){},error(){},warn(){},debug(){}};
const server=await createServerComponent({config:{requireNumber:async()=>0,requireString:async()=>"127.0.0.1"},logs:{getLogger:()=>logger}},{cors:{methods:['GET','HEAD','OPTIONS','DELETE','POST','PUT','PATCH'],maxAge:86400}});
server.use(router.middleware());server.use(router.allowedMethods());await server.start();
const port=(await getUnderlyingServer(server)).address().port;
const inputs=[
 {method:'TRACE',path:'/v1/communities'}, {method:'TRACK',path:'/unknown'},
 {method:'PROPFIND',path:'/v1/communities'}, {method:'PURGE',path:'/unknown'},
 {method:'PUT',path:'/v1/communities'}, {method:'DELETE',path:'/v1/communities/abc/members'},
 {method:'POST',path:'/v1/communities/abc/members/xyz'}, {method:'PUT',path:'/v1/referral-progress'},
 {method:'GET',path:'/v1/community-voice-chats/abc/speakers/xyz'},
 {method:'POST',path:'/unknown'}, {method:'HEAD',path:'/unknown'},
 {method:'OPTIONS',path:'/unknown'}
];const cases=[];
for(const input of inputs){const response=await new Promise((resolve,reject)=>{const request=http.request({hostname:'127.0.0.1',port,path:input.path,method:input.method,headers:{origin:'https://decentraland.org',connection:'close'}},response=>{const chunks=[];response.on('data',chunk=>chunks.push(chunk));response.on('end',()=>resolve({status:response.statusCode,body:Buffer.concat(chunks).toString(),headers:Object.fromEntries(Object.entries(response.headers).filter(([key])=>!['date','connection','keep-alive','transfer-encoding'].includes(key)))}));});request.on('error',reject);request.end();});cases.push({...input,...response});}
await server.stop();
fs.writeFileSync(process.argv[3],JSON.stringify({source:'decentraland/social-service-ea',revision:'b6b2cefefdf727b2ed47ddb887e77d226a41eea6',path:'src/controllers/routes/http.routes.ts',transport:'@dcl/http-server/dist/server.js on loopback ephemeral test listener',cases},null,2)+'\n');
})().catch(error=>{console.error(error);process.exitCode=1;});
