#!/usr/bin/env node
// Executes pinned lifecycle/pubsub/RPC startup with controlled Redis promises.
const fs=require('node:fs')
const path=require('node:path')
const {req,load,logs,revision,upstream}=require('./social-service-voice-source-cases.cjs')
const {Lifecycle}=req('@well-known-components/interfaces')
const {createRecordConfigComponent}=req('@well-known-components/env-config-provider')
const {createPubSubComponent}=load('adapters/pubsub.ts')
const {createRpcServerComponent}=load('adapters/rpc-server/rpc-server.ts')
const ts=req('typescript')
const ast=ts.createSourceFile('components.ts',fs.readFileSync(path.join(upstream,'src/components.ts'),'utf8'),ts.ScriptTarget.Latest,true)
const initializer=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='initComponents')
const returned=initializer.body.statements.find(ts.isReturnStatement).expression
const order=returned.properties.map(n=>n.name.getText(ast))
const deferred=()=>{let resolve;const promise=new Promise(r=>{resolve=r});return{promise,resolve}}
async function prepare(delayed,failSubscribe){
  const subConnect=deferred(),pubConnect=deferred(),subscriptions=deferred()
  const stages=[],channels=[],listens=[]
  let closes=0
  const clients=[0,1].map(index=>({isReady:false,handlers:{},on(name,cb){this.handlers[name]=cb},
    async connect(){stages.push(index?'pub-connect-start':'sub-connect-start');if(delayed)await(index?pubConnect:subConnect).promise;this.isReady=true;stages.push(index?'pub-connected':'sub-connected')},
    async subscribe(channel){channels.push(channel);if(failSubscribe&&channel==='block.updates')throw Error('synthetic subscription failure');if(delayed)await subscriptions.promise},
    async publish(){if(!this.isReady)throw Error('synthetic disconnected publish')},async unsubscribe(){},async disconnect(){this.isReady=false}
  }))
  let next=0
  const pubsub=createPubSubComponent({logs,redis:{client:{duplicate:()=>clients[next++]}}})
  const uwsServer={app:{listen(port,cb){listens.push(port);cb()},close(){closes++}},start(){stages.push('uws-server-start')},stop(){}}
  const updateHandler=Object.fromEntries(['friendshipUpdateHandler','friendshipAcceptedUpdateHandler','friendConnectivityUpdateHandler','communityMemberConnectivityUpdateHandler','communityMemberStatusHandler','communityDeletedUpdateHandler','blockUpdateHandler','privateVoiceChatUpdateHandler','communityVoiceChatUpdateHandler'].map(name=>[name,()=>{}]))
  const rpcServer=await createRpcServerComponent({logs,pubsub,config:createRecordConfigComponent({}),uwsServer,subscribersContext:{},metrics:{increment(){},observe(){}},voice:{},updateHandler})
  rpcServer.setServiceCreators({})
  const components=Object.fromEntries(order.filter(name=>['pubsub','rpcServer','uwsServer'].includes(name)).map(name=>[name,{pubsub,rpcServer,uwsServer}[name]]))
  const snapshot=()=>({stages:[...stages],sub_ready:clients[0].isReady,pub_ready:clients[1].isReady,subscription_attempts:channels.length,listen_count:listens.length,close_count:closes})
  return{clients,pubsub,rpcServer,components,snapshot,subConnect,pubConnect,subscriptions,channels}
}
async function main(){
  const delayed=await prepare(true,false)
  const running=Lifecycle.run({initComponents:async()=>delayed.components,main:async program=>program.startComponents()})
  await new Promise(setImmediate)
  const cases=[{id:'sub-connect-pending',state:delayed.snapshot()}]
  delayed.subConnect.resolve();await new Promise(setImmediate)
  cases.push({id:'pub-connect-pending',state:delayed.snapshot()})
  delayed.pubConnect.resolve();await new Promise(setImmediate)
  cases.push({id:'subscriptions-pending',state:delayed.snapshot()})
  delayed.subscriptions.resolve();const program=await running
  cases.push({id:'ready',state:delayed.snapshot(),channels:delayed.channels})
  for(const client of delayed.clients){client.isReady=false;client.handlers.error(Error('synthetic disconnect'))}
  await delayed.pubsub.publishInChannel('block.updates',{address:'test'})
  cases.push({id:'outage-after-ready',state:delayed.snapshot()})
  await program.stop()
  const failure=await prepare(false,true)
  await failure.pubsub.start()
  try{await failure.rpcServer.start();throw Error('expected startup failure')}catch(error){cases.push({id:'subscription-failure',state:failure.snapshot(),error:error.message})}
  await failure.pubsub.stop()
  fs.writeFileSync(process.argv[3],JSON.stringify({baseline:{repository:'decentraland/social-service-ea',revision},runtime:{node:process.version},scope:'Actual source component-key order, official sequential lifecycle, pubsub.start/subscribe/publish and RPC start with controlled Redis promises and listen callback. No real Redis or TCP listener; standalone source listener admission differs from the target monolith empty503 guard.',component_key_order:order,cases},null,2)+'\n')
  process.stdout.write(`Captured ${cases.length} source startup stages\n`)
}
main().catch(error=>{console.error(error);process.exitCode=1})
