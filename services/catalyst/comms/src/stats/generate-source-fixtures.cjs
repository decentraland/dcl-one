// Generate deterministic handler fixtures from an explicit pinned source checkout.
// Usage: node generate-source-fixtures.cjs <upstream-root> <output.json>
// NODE_PATH must resolve the locked protocol, protobufjs and TypeScript packages.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const Module = require('node:module');
const lock = JSON.parse(fs.readFileSync(path.join(__dirname, 'source-runtime-lock.json'), 'utf8'));
const digest = data => crypto.createHash('sha256').update(data).digest('hex');
if (process.version !== lock.node) throw new Error('Wrong fixture Node version');
// Check every dependency before execution, including lazily loaded transitive files.
for (const extension of ['.js', '.json', '.node']) {
  const original = Module._extensions[extension];
  Module._extensions[extension] = (module, filename) => {
    let folder = path.dirname(filename);
    const pinnedPackage = folder => {
      const file = path.join(folder, 'package.json');
      if (!fs.existsSync(file)) return false;
      const manifest = JSON.parse(fs.readFileSync(file));
      return !!lock.packages[`${manifest.name}@${manifest.version}`];
    };
    while (!pinnedPackage(folder)) {
      const parent = path.dirname(folder);
      if (parent === folder) throw new Error(`Unpinned module: ${filename}`);
      folder = parent;
    }
    const manifestBytes = fs.readFileSync(path.join(folder, 'package.json'));
    const manifest = JSON.parse(manifestBytes);
    const pin = lock.packages[`${manifest.name}@${manifest.version}`];
    const member = path.relative(folder, filename).replaceAll('\\', '/');
    if (!pin || digest(manifestBytes) !== pin.files['package.json'] || digest(fs.readFileSync(filename)) !== pin.files[member]) {
      throw new Error(`Unpinned dependency bytes: ${manifest.name}/${member}`);
    }
    return original(module, filename);
  };
}
const ts = require('typescript');
const [root, output] = process.argv.slice(2);
if (!root || !output) throw new Error('Expected upstream root and output filename');
function version(name) {
  let folder = path.dirname(require.resolve(name));
  while (folder !== path.dirname(folder)) {
    const file = path.join(folder, 'package.json');
    if (fs.existsSync(file)) {
      const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
      if (manifest.name === name) return manifest.version;
    }
    folder = path.dirname(folder);
  }
  throw new Error(`Missing manifest: ${name}`);
}
const versions = Object.fromEntries(['@dcl/protocol', 'protobufjs', 'typescript', 'dcl-catalyst-client', 'long'].map(name => [name, name === 'long' ? version(name) : require(`${name}/package.json`).version]));
if (versions['@dcl/protocol'] !== '1.0.0-35089025179.commit-5810768' || versions.protobufjs !== '7.2.4') throw new Error('Wrong protocol runtime');
const sources = {};
const sourcesLf = {};
require.extensions['.ts'] = (module, filename) => {
  const source = fs.readFileSync(filename, 'utf8').replaceAll('\r\n', '\n');
  const relative = path.relative(root, filename).replaceAll('\\', '/');
  if (digest(source) !== lock.sources[relative]) throw new Error(`Unpinned application bytes: ${relative}`);
  sources[relative] = crypto.createHash('sha256').update(source).digest('hex');
  sourcesLf[relative] = crypto.createHash('sha256').update(source.replaceAll('\r\n', '\n')).digest('hex');
  module._compile(ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText, filename);
};
const load = name => require(path.resolve(root, 'stats/src', name));
const protocol = require('@dcl/protocol/out-js/decentraland/kernel/comms/v3/archipelago.gen');
const stats = load('adapters/stats.ts').createStatsComponent();
let now = 1000;
const coreStatus = load('adapters/core-status.ts').createCoreStatusComponent({ clock: { now: () => now } });
const callbacks = new Map();
const errors = [];
load('logic/subscriptions.ts').registerSubscriptions({stats, coreStatus, nats: {subscribe: (subject, callback) => callbacks.set(subject, callback)}, logs: {getLogger: () => ({error: e => errors.push(String(e))})}});
const handlers = Object.assign({}, ...['peers','parcels','islands','core-status','hot-scenes'].map(name => load(`controllers/handlers/${name}-handler.ts`)));
const events = [];
let topology = [];
let discovery = [];
const cases = [];
const baseUrl = 'https://content.example/content/';
let documents = [];
let fetchCount = 0;
const pointerCalls = [];
async function main() {
  const content = await load('adapters/content.ts').createContentComponent({config: {getString: async () => baseUrl}, fetch: {fetch: async (_url, init) => { fetchCount++; pointerCalls.push(JSON.parse(init.body).pointers); return { json: async () => documents }; }}});
  function event(subject, bytes, at) {
    now = at;
    const old = Date.now;
    Date.now = () => now;
    try { callbacks.get(subject.startsWith('peer.') ? `peer.*.${subject.split('.')[2]}` : subject)(null, {subject, data: Uint8Array.from(bytes)}); } finally { Date.now = old; }
    events.push({subject, bytes: Array.from(bytes), time: at});
    if (subject === 'engine.islands' && bytes[0] !== 255) topology = Array.from(bytes);
    if (subject === 'engine.discovery' && bytes[0] !== 255) discovery = Array.from(bytes);
  }
  const hb = (id, position, at) => event(`peer.${id}.heartbeat`, protocol.Heartbeat.encode(protocol.Heartbeat.fromPartial({position})).finish(), at);
  async function capture(name) {
    const context = {components: {stats, coreStatus, content}, url: new URL('https://stats.example/peers'), params: {id: 'B'}};
    const reads = [];
    for (const query of ['', '?id=a&id=B&id=a', '?id=', '?id=A,B', '?ID=a', '?%69d=a', '?id=%FF']) {
      context.url = new URL(`https://stats.example/peers${query}`);
      reads.push({path: `/peers${query}`, status:200, body:JSON.stringify((await handlers.peersHandler(context)).body)});
    }
    for (const id of ['B','b','A/B+%']) {
      context.params.id = id;
      const result = await handlers.peerHandler(context);
      reads.push({path:`/peers/${encodeURIComponent(id)}`, status: result.status, body: JSON.stringify(result.body)});
    }
    for (const [route, fn] of [['parcels','parcelsHandler'],['islands','islandsHandler'],['core-status','coreStatusHandler'],['hot-scenes','hotScenesHandler']]) {
      reads.push({path:`/${route}`,status:200,body:JSON.stringify((await handlers[fn](context)).body)});
    }
    for (const id of ['C1','c1','missing']) {
      context.params.id=id;
      const result=await handlers.islandHandler(context);
      reads.push({path:`/islands/${id}`,status:result.status,body:result.body===undefined?'':JSON.stringify(result.body)});
    }
    const peers = Array.from(stats.getPeers().values()).map(p => ({address:p.address,time:p.time,positionBits:[p.x,p.y,p.z].map(n=>{const b=Buffer.alloc(8);b.writeDoubleLE(n);return b.readBigUInt64LE().toString();})}));
    cases.push({name, events:events.splice(0), now, peers, topology, discovery, documents, reads});
  }
  await capture('cold');
  hb('B',{x:-0.1,y:2,z:32},123);
  hb('a',{x:16,y:0,z:0},124);
  hb('A',{x:-0.01,y:0,z:32},125);
  hb('B',{x:-16.01,y:0.1,z:32},90);
  hb('A/B+%',{x:1e22,y:-0,z:1e-8},126);
  event('engine.islands',protocol.IslandStatusMessage.encode(protocol.IslandStatusMessage.fromPartial({data:[{id:'skip'},{id:'C1',maxPeers:42,center:{x:0.1,y:2,z:3},radius:1.0000000000000002,peers:['a','missing','A','a']},{id:'C1',center:{}}]})).finish(),127);
  event('engine.discovery',protocol.ServiceDiscoveryMessage.encode(protocol.ServiceDiscoveryMessage.fromPartial({status:{currentTime:1000,userCount:17}})).finish(),1000);
  documents=[{id:'z',metadata:{scene:{base:' -12x,+3,wat,0x10',parcels:['1,0','1,0']},display:{title:null,navmapThumbnail:'thumb',description:false},contact:{name:7},source:{projectId:['raw']}},content:[{file:'thumb',hash:'first'},{file:'thumb',hash:'second'}]},{id:'a',metadata:{scene:{base:'4',parcels:[]}}}];
  await capture('populated');
  event('peer.B.disconnect',[],1001);
  hb('B',{x:Infinity,y:NaN,z:-Infinity},1002);
  hb('no-position',undefined,1003);
  event('engine.islands',[255],1004);
  now=91000;
  await capture('disconnect_reinsert_nonfinite_stale');
  now=-88999;
  await capture('future_clock_no_expiry');
  event('engine.islands',[],92000);
  await capture('empty_topology');
  const wire = [];
  for (const [type, label, value] of [
    ['ClientPacket','challenge', {message: {$case:'challengeRequest', challengeRequest:{address:'0xAb'}}}],
    ['ClientPacket','signed', {message: {$case:'signedChallenge', signedChallenge:{authChainJson:'[{"type":"SIGNER"}]'}}}],
    ['ClientPacket','heartbeat', {message: {$case:'heartbeat', heartbeat:{position:{x:0.1,y:-16.01,z:1e22},desiredRoom:''}}}],
    ['ClientPacket','missing-position', {message: {$case:'heartbeat', heartbeat:{}}}],
    ['ServerPacket','challenge-response', {message: {$case:'challengeResponse', challengeResponse:{challengeToSign:'dcl-'+'ab'.repeat(32),alreadyConnected:true}}}],
    ['ServerPacket','welcome', {message: {$case:'welcome', welcome:{peerId:'0xab'}}}],
    ['ServerPacket','kicked', {message: {$case:'kicked', kicked:{reason:0}}}],
    ['ServerPacket','island', {message: {$case:'islandChanged', islandChanged:{islandId:'C1',connStr:'livekit:wss://example.invalid?access_token=fixture',fromIslandId:'',peers:{a:{x:0.1,y:0,z:2}}}}}]
  ]) wire.push({type,label,bytes:Array.from(protocol[type].encode(protocol[type].fromPartial(value)).finish())});
  fs.writeFileSync(output,JSON.stringify({provenance:{repository:'decentraland/archipelago-workers',revision:'64dc515987175c679e4009fe557ea60fdf50bc6c',kind:'local source-handler harness; not deployed HTTP capture',node:process.version,versions,sources,sourcesLf,fetchCount,pointerCalls,containedDecodeErrors:errors.length},baseUrl,cases,wire},null,2)+'\n');
}
main().catch(error=>{console.error(error);process.exitCode=1;});
