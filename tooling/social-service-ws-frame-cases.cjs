#!/usr/bin/env node
// Executes the authenticated handler -> transport -> actual dRPC server path.
// Authentication and the native socket are controlled component boundaries.
const fs=require('node:fs')
const {req,load,logs,caller,revision}=require('./social-service-voice-source-cases.cjs')
const mitt=req('mitt').default||req('mitt')
const {createRecordConfigComponent}=req('@well-known-components/env-config-provider')
const {registerWsHandler}=load('controllers/handlers/uws/ws-handler.ts')
const {createUWebSocketTransport}=load('utils/UWebSocketTransport.ts')
const {createRpcServer}=req('@dcl/rpc')
const protocol=req('@dcl/rpc/dist/protocol')
const {calculateMessageIdentifier}=req('@dcl/rpc/dist/protocol/helpers')
async function main(){
  let behavior
  const frames=[],forwarded=[],errors=[]
  const emitter=mitt()
  const data={auth:true,authenticating:false,isConnected:true,protocol:'rpc',address:caller,wsConnectionId:'fixture',eventEmitter:emitter}
  const socket={getUserData:()=>data,getBufferedAmount:()=>0,send:(message,isBinary)=>{
    frames.push(typeof message==='string'?{text:message}:{hex:Buffer.from(message).toString('hex'),binary:isBinary===true})
    return 1
  },end:(code,reason)=>{data.isConnected=false;frames.push({close:{code,reason}})},close:()=>{data.isConnected=false}}
  const components={logs,config:createRecordConfigComponent({}),metrics:{increment(){},observe(){}},
    uwsServer:{app:{ws:(_,callbacks)=>{behavior=callbacks}}},fetcher:{},rpcServer:{detachUser(){}},wsPool:{},wsAdmission:{},tracing:{captureException:error=>errors.push(error.message)}}
  await registerWsHandler(components)
  data.transport=await createUWebSocketTransport(socket,emitter,components)
  emitter.on('message',buffer=>forwarded.push(Buffer.from(buffer).toString('hex')))
  const server=createRpcServer({logger:logs.getLogger()})
  server.setHandler(async()=>{})
  server.attachTransport(data.transport,{})
  await new Promise(setImmediate)
  const ready=frames.splice(0)
  const cases=[]
  for(const [index,binary] of [true,false].entries()){
    const request=protocol.CreatePort.encode({messageIdentifier:calculateMessageIdentifier(protocol.RpcMessageTypes.RpcMessageTypes_CREATE_PORT,index+1),portName:binary?'binary-port':'text-port'}).finish()
    const buffer=Uint8Array.from(request).buffer
    const text=Buffer.from(request).toString('utf8')
    if(!Buffer.from(text,'utf8').equals(Buffer.from(request)))throw Error('fixture must be valid UTF-8 for a real text frame')
    await behavior.message(socket,buffer,binary)
    await new Promise(setImmediate)
    cases.push({binary,input_hex:Buffer.from(request).toString('hex'),text,forwarded:forwarded.splice(0),responses:frames.splice(0)})
  }
  data.isConnected=false
  await behavior.message(socket,Uint8Array.from([1]).buffer,false)
  cases.push({id:'disconnected-skips',forwarded:forwarded.splice(0),responses:frames.splice(0)})
  data.isConnected=true
  emitter.on('message',()=>{throw Error('synthetic callback failure')})
  await behavior.message(socket,Uint8Array.from([1]).buffer,false)
  cases.push({id:'forward-callback-failure',responses:frames.splice(0),tracing:errors})
  data.transport.close()
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},runtime:{node:process.version,rpc:req('@dcl/rpc/package.json').version},
    scope:'Actual registerWsHandler, UWebSocketTransport and installed dRPC server; pre-authenticated user data and controlled native socket. Exercises the ignored isBinary callback argument. Authentication, native frame decoding and media are outside this fixture.',ready,cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} authenticated frame scenarios\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
