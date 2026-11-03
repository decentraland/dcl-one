#!/usr/bin/env node
// Runs the actual pinned transport with a controlled socket and virtual timers.
// The socket outcomes are component fixtures, not kernel-buffer measurements.
const fs=require('node:fs')
const {req,load,logs,revision}=require('./social-service-voice-source-cases.cjs')
const FakeTimers=req('@sinonjs/fake-timers')
const mitt=req('mitt').default||req('mitt')
const {createRecordConfigComponent}=req('@well-known-components/env-config-provider')
const {createUWebSocketTransport}=load('utils/UWebSocketTransport.ts')
async function capture(id,config,actions,script=[],threshold=false) {
  const clock=FakeTimers.install({now:0,toFake:['setTimeout','clearTimeout','Date']})
  let active=true,buffered=0,index=0,api
  const sends=[],errors=[],closes=[],received=[],futures={},metrics=[]
  let capacity
  const dependencies={config:createRecordConfigComponent(config),logs:{getLogger:()=>({
    ...logs.getLogger(),debug(message,fields){if(message==='Queue size configuration')capacity=fields.finalMaxQueueSize}
  })},metrics:{increment:(name,labels={})=>metrics.push({name,labels}),observe(){}}}
  const emitter=mitt()
  const socket={
    getUserData:()=>({isConnected:active}),getBufferedAmount:()=>buffered,
    end:(code,reason)=>{closes.push({code,reason});active=false},close:()=>{active=false},
    send(bytes){
      let result
      if(threshold){result=buffered>Number(config.WS_MAX_BACKPRESSURE)?2:0;if(result!==2)buffered+=bytes.length}
      else{result=script[Math.min(index++,script.length-1)]??1}
      sends.push({at:clock.now,label:bytes[0],bytes:bytes.length,result})
      if(result==='throw')throw Error('synthetic send failure')
      return result
    }
  }
  try {
    api=await createUWebSocketTransport(socket,emitter,dependencies)
    // The RPC server closes on transport error; ws-handler's close listener
    // then ends the connected socket with its source code and reason.
    api.on('error',error=>{errors.push(error.message);api.close()})
    api.on('close',()=>{if(active)socket.end(1011,'RPC transport closed')})
    api.on('message',bytes=>received.push([...bytes]))
    const names={max_queue_size:['WS_TRANSPORT_MAX_QUEUE_SIZE',1000],min_queue_size:['WS_TRANSPORT_MIN_QUEUE_SIZE',100],
      max_queue_size_limit:['WS_TRANSPORT_MAX_QUEUE_SIZE_LIMIT',5000],estimated_message_size:['WS_TRANSPORT_ESTIMATED_MESSAGE_SIZE',1024],
      retry_delay_ms:['WS_TRANSPORT_RETRY_DELAY_MS',1000],max_retry_attempts:['WS_TRANSPORT_MAX_RETRY_ATTEMPTS',5],
      max_backoff_delay_ms:['WS_TRANSPORT_MAX_BACKOFF_DELAY_MS',30000],max_backpressure:['WS_MAX_BACKPRESSURE',131072]}
    const configuration=Object.fromEntries(await Promise.all(Object.entries(names).map(async([field,[name,fallback]])=>[field,(await dependencies.config.getNumber(name))??fallback])))
    const checkpoints=[]
    for(const action of actions){
      if(action.op==='send'){
        const bytes=new Uint8Array(action.bytes??1);bytes[0]=action.label
        futures[action.label]='pending'
        const result=api.sendMessage(bytes)
        Promise.resolve(result).then(()=>{futures[action.label]='resolved'},error=>{futures[action.label]=`rejected:${error.message}`})
      }else if(action.op==='tick'){await clock.tickAsync(action.ms)}
      else if(action.op==='drain'){buffered=action.buffered??0;emitter.emit('drain')}
      else if(action.op==='buffer'){buffered=action.bytes}
      else if(action.op==='disconnect'){active=false}
      else if(action.op==='close'){api.close()}
      else if(action.op==='receive'){emitter.emit('message',Uint8Array.from(action.bytes).buffer)}
      await clock.tickAsync(0)
      const values=Object.values(futures)
      checkpoints.push({at:clock.now,sends:sends.length,errors:errors.length,closed:closes.length,
        pending:values.filter(v=>v==='pending').length,resolved:values.filter(v=>v==='resolved').length,rejected:values.filter(v=>v.startsWith('rejected:')).length})
    }
    return JSON.parse(JSON.stringify({id,config,script,threshold,capacity,configuration,actions,checkpoints,sends,errors,closes,received,metrics,futures},(_,value)=>typeof value==='number'&&!Number.isFinite(value)?String(value):value))
  }finally{api?.close();clock.uninstall()}
}
async function main(){
  const config={WS_TRANSPORT_MAX_QUEUE_SIZE:'3',WS_TRANSPORT_MIN_QUEUE_SIZE:'3',WS_TRANSPORT_MAX_QUEUE_SIZE_LIMIT:'3',
    WS_TRANSPORT_ESTIMATED_MESSAGE_SIZE:'1024',WS_TRANSPORT_RETRY_DELAY_MS:'10',WS_TRANSPORT_MAX_RETRY_ATTEMPTS:'3',
    WS_TRANSPORT_MAX_BACKOFF_DELAY_MS:'1000',WS_MAX_BACKPRESSURE:'1024'}
  const send=label=>({op:'send',label})
  const cases=[]
  cases.push(await capture('default-capacity-overflow',{},Array.from({length:129},(_,n)=>send(n+1)),[2]))
  cases.push(await capture('burst-keeps-scheduled-backoff',config,[send(1),send(2),send(3),{op:'tick',ms:19},{op:'tick',ms:1}],[2,1,1,1]))
  cases.push(await capture('drain-advances-retry',config,[send(1),{op:'tick',ms:5},{op:'drain'},{op:'tick',ms:100}],[2,1]))
  cases.push(await capture('backpressure-accepts-once',config,[send(1),send(2),{op:'tick',ms:10}],[0,1]))
  cases.push(await capture('max-attempts-close',config,[send(1),{op:'tick',ms:139},{op:'tick',ms:1}],[2]))
  cases.push(await capture('send-error-close',config,[send(1)],["throw"]))
  cases.push(await capture('unexpected-result-retains-head',config,[send(1),{op:'tick',ms:10}],[99,1]))
  cases.push(await capture('closed-socket-skips-and-close-is-idempotent',config,[{op:'disconnect'},send(1),{op:'close'},{op:'close'},{op:'receive',bytes:[9]}],[1]))
  cases.push(await capture('message-callback-detaches-on-close',config,[{op:'receive',bytes:[1,2]},{op:'close'},{op:'receive',bytes:[3]}],[1]))
  cases.push(await capture('existing-buffer-threshold-allows-crossing-frame',config,[{op:'buffer',bytes:1024},{op:'send',label:1,bytes:2048},send(2),{op:'tick',ms:10},{op:'tick',ms:5},{op:'drain'}],[],true))
  cases.push(await capture('backoff-ceiling',{...config,WS_TRANSPORT_MAX_BACKOFF_DELAY_MS:'25'},[send(1),{op:'tick',ms:69},{op:'tick',ms:1}],[2]))
  cases.push(await capture('fractional-queue-threshold',{...config,WS_TRANSPORT_MAX_QUEUE_SIZE:'1.5'},[send(1),send(2),send(3)],[2]))
  cases.push(await capture('zero-queue-rejects-before-first-send',{...config,WS_TRANSPORT_MAX_QUEUE_SIZE:'0'},[send(1)],[1]))
  cases.push(await capture('huge-finite-capacity-without-proportional-allocation',{...config,WS_TRANSPORT_MAX_QUEUE_SIZE:'1e300',WS_TRANSPORT_MIN_QUEUE_SIZE:'1e300',WS_TRANSPORT_MAX_QUEUE_SIZE_LIMIT:'1e300'},[send(1)],[1]))
  cases.push(await capture('numeric-prefix-config',{...config,WS_TRANSPORT_MAX_QUEUE_SIZE:'1.5messages',WS_TRANSPORT_RETRY_DELAY_MS:'10ms'},[send(1),send(2),send(3)],[2]))
  cases.push(await capture('zero-backpressure-target-rejects',{...config,WS_MAX_BACKPRESSURE:'0'},[],[1]))
  cases.push(await capture('nonfinite-capacity-target-rejects',{...config,WS_TRANSPORT_MAX_QUEUE_SIZE:'Infinity',WS_TRANSPORT_MIN_QUEUE_SIZE:'Infinity'},[],[1]))
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},
    metric_declarations:Object.fromEntries(['ws_messages_sent','ws_backpressure_events','ws_unexpected_send_result_events','ws_message_size_bytes','ws_queue_vs_backpressure_ratio','ws_drain_events'].map(name=>[name,load('metrics.ts').metricDeclarations[name]])),
    runtime:{node:process.version},scope:'Executed UWebSocketTransport with controlled send outcomes, mitt callbacks and virtual timers. RPC error-close and ws-handler close listeners are represented by their synchronous teardown contract. Does not exercise kernel or native socket buffering.',cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} upstream ordinary transport traces\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
