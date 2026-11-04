// Runs only against the materialized, pinned social-service-ea source.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <source-root> <output-json>
const fs=require('fs');
const upstream=process.argv[2];
const {createCorsMiddleware}=require(upstream+'/node_modules/@dcl/http-server/dist/cors.js');
const {normalizeResponseBody}=require(upstream+'/node_modules/@dcl/http-server/dist/logic.js');
const middleware=createCorsMiddleware({methods:['GET','HEAD','OPTIONS','DELETE','POST','PUT','PATCH'],maxAge:86400});
(async()=>{
const inputs=[
 {name:'preflight-reflected',method:'OPTIONS',path:'/unknown',headers:{origin:'https://decentraland.org','access-control-request-headers':'authorization,x-custom'}},
 {name:'preflight-without-origin',method:'OPTIONS',path:'/unknown',headers:{'access-control-request-headers':'x-a'}},
 {name:'preflight-without-requested-headers',method:'OPTIONS',path:'/v1/communities',headers:{origin:'null'}},
 {name:'actual-origin',method:'GET',path:'/unknown',headers:{origin:'https://decentraland.org'}},
 {name:'actual-without-origin',method:'GET',path:'/unknown',headers:{}}
];
const cases=[];
for(const input of inputs){const request=new Request('http://fixture.invalid'+input.path,{method:input.method,headers:input.headers});const response=normalizeResponseBody(request,await middleware({request},async()=>({status:404,body:'Not found'})));cases.push({...input,status:response.status,body:response.body?.toString()||'',responseHeaders:Object.fromEntries(new Headers(response.headers).entries())});}
fs.writeFileSync(process.argv[3],JSON.stringify({source:'decentraland/social-service-ea',revision:'b6b2cefefdf727b2ed47ddb887e77d226a41eea6',adapter:'@dcl/http-server/dist/cors.js',cases},null,2)+'\n');
})().catch(error=>{console.error(error);process.exitCode=1;});
