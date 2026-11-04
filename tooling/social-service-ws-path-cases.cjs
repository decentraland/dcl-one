#!/usr/bin/env node
// Signs with fresh ephemeral test identities; stores no keys or signatures.
const fs=require('node:fs')
const path=require('node:path')
const {req,load,upstream,revision}=require('./social-service-voice-source-cases.cjs')
const {createTestIdentity,createAuthHeaders}=req(path.join(upstream,'test/integration/utils/auth.ts'))
const {verifyWsIdentity,verifyWsAuthChain}=load('utils/ws-session.ts')
async function main(){
  const identity=await createTestIdentity()
  const cases=[]
  const fetcher={fetch:async()=>{throw Error('EOA path fixture must not fetch')}}
  const signedPaths=['/','/social-rpc','/social-rpc/','/social-rpc//','/edge/social-rpc','/v1/stream','/social-rpc/v1/stream','/edge/v1/stream','/v1/stream/']
  for(const protocol of ['rpc','json'])for(const mode of protocol==='rpc'?['first_frame']:['first_frame','headers'])for(const signedPath of signedPaths)for(const originalPath of [null,protocol==='rpc'?'/social-rpc':'/social-rpc/v1/stream',protocol==='rpc'?'/social-rpc/?x=1':'/edge/v1/stream?x=1']){
    const headers=createAuthHeaders('get',signedPath,{},identity)
    if(originalPath!==null)headers['x-original-path']=originalPath
    let accepted=false,error
    try{
      const expiration=300000
      if(mode==='headers')await verifyWsAuthChain({fetcher},headers,expiration,'/v1/stream')
      else{
        const data=new TextEncoder().encode(JSON.stringify(headers)).buffer
        if(protocol==='rpc')await verifyWsIdentity({fetcher},data,expiration)
        else await verifyWsIdentity({fetcher},data,expiration,'/v1/stream')
      }
      accepted=true
    }catch(failure){error={name:failure.constructor.name,status:failure.statusCode??null}}
    cases.push({protocol,mode,signed_path:signedPath,original_path:originalPath,accepted,...(error?{error}:{})})
  }
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},runtime:{node:process.version},scope:'Actual verifyWsIdentity / verifyWsAuthChain and crypto middleware with freshly generated source test identities. Header variants are supplied in the auth bag; the source verifiers take an explicit protocol path, independently of mounted URLs or x-original-path. No keys or signed envelopes retained.',cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} actual verifier path decisions\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
