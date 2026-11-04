#!/usr/bin/env node
// Optional actual-source captures; no external server/network is invoked.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto')
const {execFileSync}=require('node:child_process'),{createRequire}=require('node:module')
const revision='b6b2cefefdf727b2ed47ddb887e77d226a41eea6',upstream=path.resolve(process.argv[2]||''),output=process.argv[3]
if(!process.argv[2]||!output)throw Error('Usage: node tooling/social-service-auxiliary-cases.cjs <upstream> <output.json>')
const git=(...args)=>execFileSync('git',['-C',upstream,...args],{encoding:'utf8'}).trim()
if(git('rev-parse','HEAD')!==revision||git('status','--porcelain','--untracked-files=no'))throw Error('Pinned clean upstream required')
const from=createRequire(path.join(upstream,'package.json'));from('ts-node').register({transpileOnly:true,project:path.join(upstream,'tsconfig.json')})
const load=file=>from(path.join(upstream,'src',file)),logs={getLogger:()=>({info(){},warn(){},error(){},debug(){}})}
async function main(){
 const cases=[]
 for(const values of [{},{CURRENT_VERSION:'custom-version',COMMIT_HASH:'custom-commit'},{CURRENT_VERSION:'',COMMIT_HASH:'  '}] ){
  const handler=await load('controllers/handlers/uws/status-handler.ts').createStatusHandler({config:{getString:async key=>values[key]}})
  const response=await handler.f();delete response.body.currentTime;cases.push({operation:'status',config:values,response})
 }
 const owner='0x1111111111111111111111111111111111111111'
 for(const [id,address,rows,error] of [['invalid','bad',[]],['absent',owner,[]],['upper',owner.toUpperCase(),[]],['friends',owner,[{private_messages_privacy:'only_friends'}]],['error',owner,[],Error('source failure')],['unknown-error',owner,[],null]]){
  const handler=await load('controllers/handlers/uws/privacy-handler.ts').createPrivacyHandler({logs,friendsDb:{getSocialSettings:async()=>{if(id.includes('error'))throw error;return rows}}})
  cases.push({operation:'privacy',id,address,response:await handler.f({}, {getParameter:()=>address})})
 }
 const {createMetricsHandler}=from('@dcl/uws-http-server')
 for(const [id,values,authorization,abort] of [['open',{},undefined,false],['empty-config',{WKC_METRICS_PUBLIC_PATH:'',WKC_METRICS_BEARER_TOKEN:''},undefined,false],['custom-path',{WKC_METRICS_PUBLIC_PATH:'/custom'},undefined,false],['gated-missing',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},undefined,false],['gated-wrong',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},'Bearer wrong',false],['gated-lowercase',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},'bearer fixture-token',false],['gated-double-space',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},'Bearer  fixture-token',false],['gated-extra-token',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},'Bearer fixture-token extra',false],['gated-ok',{WKC_METRICS_BEARER_TOKEN:'fixture-token'},'Bearer fixture-token',false],['aborted',{},undefined,true]]){
  let onAborted,metricsCalls=0;const response={headers:{}};
  const handler=await createMetricsHandler({config:{getString:async key=>values[key]},metrics:{resetAll(){}}},{contentType:'text/plain; version=0.0.4; charset=utf-8',metrics:async()=>{metricsCalls++;if(abort)onAborted();return 'ws_messages_received 3\n'}})
  const res={onAborted:callback=>{onAborted=callback},writeStatus:status=>{response.status=status;return res},writeHeader:(key,value)=>{response.headers[key]=value;return res},end:body=>{response.body=body||'';return res}}
  await handler.handler(res,{getHeader:()=>authorization});cases.push({operation:'metrics',id,config:values,authorization,path:handler.path,response,metrics_calls:metricsCalls})
 }
 const declarations=load('metrics.ts').metricDeclarations
 const sourceFiles=['src/controllers/handlers/uws/status-handler.ts','src/controllers/handlers/uws/privacy-handler.ts','node_modules/@dcl/uws-http-server/dist/src/metrics.js','src/metrics.ts']
 fs.writeFileSync(output,JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision,yarn_lock_sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,'yarn.lock'))).digest('hex'),sources:sourceFiles.map(file=>({path:file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(upstream,file))).digest('hex')}))},runtime:{node:process.version},scope:'Actual pinned uWS status/privacy factories and installed metrics handler, with injected database, registry and request/response objects. No deployed lifecycle, Prometheus samples or real connection abort claimed.',cases,metric_declarations:declarations},null,2)+'\n')
 process.stdout.write(`Captured ${cases.length} source auxiliary cases and ${Object.keys(declarations).length} metric declarations\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
