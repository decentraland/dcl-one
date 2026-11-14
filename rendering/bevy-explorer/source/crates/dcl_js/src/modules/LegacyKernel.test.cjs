const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function load(name, ops = {}, console = {}) {
  const context = { module: { exports: {} }, Deno: { core: { ops } }, console, Uint8Array };
  vm.runInNewContext(fs.readFileSync(require.resolve(`./${name}.js`), 'utf8'), context);
  return context.module.exports;
}
const plain = value => JSON.parse(JSON.stringify(value));

test('DevTools translates CDP values and exceptions into scene logging', async () => {
  const entries = [];
  const api = load('DevTools', {}, { log: (...args) => entries.push(['log', ...args]), error: (...args) => entries.push(['error', ...args]) });
  assert.deepEqual(plain(await api.event({ type: 'Runtime.consoleAPICalled', jsonPayload: JSON.stringify([{ args: [{value:0}, {value:null}, {unserializableValue:'NaN'}, {value:{x:1}}] }]) })), {});
  await api.event({ type: 'Runtime.exceptionThrown', jsonPayload: JSON.stringify([{exceptionDetails:{text:'Unhandled',exception:{value:'failure'}}}]) });
  await api.event({ type: 'Runtime.exceptionThrown', jsonPayload: JSON.stringify([{exceptionDetails:{text:'Only text'}}]) });
  await api.event({ type: 'Runtime.exceptionThrown', jsonPayload: JSON.stringify([{exceptionDetails:{text:'Fallback',exception:{value:0,unserializableValue:'unknown'}}}]) });
  assert.deepEqual(plain(entries), [['log','',0,null,'NaN',{x:1}],['error','Unhandled','failure'],['error','Only text'],['error','Fallback','unknown']]);
  await api.event({type:'Other.event',jsonPayload:'{}'});
  assert.equal(entries.length, 4);
  await assert.rejects(api.event({type:'Other.event',jsonPayload:'broken'}));
});

test('ParcelIdentity reports the assigned scene metadata and content paths', async () => {
  const scene = { urn:'scene-hash', metadataJson:'{"scene":{"base":"1,2"}}', baseUrl:'https://realm.example/content/contents/', content:[{file:'main.js',hash:'script-hash'}] };
  const api = load('ParcelIdentity', {op_scene_information:async()=>scene, op_scene_id:()=>scene.urn});
  assert.deepEqual(plain(await api.getParcel()), {cid:'scene-hash',land:{sceneId:'scene-hash',sceneJsonData:scene.metadataJson,baseUrl:scene.baseUrl,baseUrlBundles:'',mappingsResponse:{parcelId:'scene-hash',rootCid:'scene-hash',contents:scene.content}}});
  assert.deepEqual(plain(await api.getSceneId()), {sceneId:'scene-hash'});
  assert.deepEqual(plain(await api.getIsEmpty()), {isEmpty:false});
  scene.metadataJson = '';
  assert.equal((await api.getParcel()).land.sceneJsonData, '{}');
});

test('ParcelIdentity cannot invent scene information when the engine rejects it', async () => {
  const api = load('ParcelIdentity', {op_scene_information:async()=>{throw Error('Scene hash not found?!');}});
  await assert.rejects(api.getParcel(), /Scene hash not found/);
});

test('Permissions preserves enum values, input order and protobuf response names', async () => {
  const calls = [];
  const api = load('Permissions', {op_get_permissions:async values=>{calls.push(plain(values));return values.map(value=>value===0);}});
  assert.equal(api.PermissionItem.PI_USE_WEB3_API, 2);
  assert.equal(api.PermissionItem[5], 'PI_ALLOW_MEDIA_HOSTNAMES');
  assert.equal(api.PermissionItem.UNRECOGNIZED, -1);
  assert.deepEqual(plain(await api.hasPermission({permission:0})), {hasPermission:true});
  assert.deepEqual(plain(await api.hasManyPermissions({permissions:[2,0,2,-1]})), {hasManyPermission:[false,true,false,false]});
  assert.deepEqual(plain(await api.hasPermission()), {hasPermission:true});
  assert.deepEqual(plain(await api.hasManyPermissions({})), {hasManyPermission:[]});
  assert.deepEqual(calls, [[0],[2,0,2,-1],[0],[]]);
});

test('deprecated CRDT receive requests and returns the real renderer byte batches', async () => {
  const sent = [];
  const api = load('EngineApi', {op_crdt_send_to_renderer:data=>sent.push(data), op_crdt_recv_from_renderer:async()=>[[1,2],[3]]});
  const response = await api.crdtGetMessageFromRenderer({});
  assert.equal(sent.length, 1);
  assert.ok(sent[0] instanceof Uint8Array);
  assert.equal(sent[0].length, 0);
  assert.deepEqual(response.data.map(value=>Array.from(value)), [[1,2],[3]]);
});

test('SocialController initializes its lifetime and awaits the draining native pull', async () => {
  let subscribed = 0;
  let events = [{event:'USER_REMOVED',payload:'{"type":"USER_REMOVED","userId":"0xabc"}'}];
  const api = load('SocialController', {op_subscribe_legacy_avatar_events:()=>subscribed++, op_pull_legacy_avatar_events:async()=>{const current=events; events=[]; return current;}});
  assert.equal(subscribed, 1);
  assert.deepEqual(plain(await api.pullAvatarEvents({})), {events:[{event:'USER_REMOVED',payload:'{"type":"USER_REMOVED","userId":"0xabc"}'}]});
  assert.deepEqual(plain(await api.pullAvatarEvents({})), {events:[]});
});

test('sandbox runtime failure waits for retained terminal error and reports only once', () => {
  const worker = fs.readFileSync(require.resolve('../../../../deploy/web/engine/sandbox_worker.js'), 'utf8');
  const start = worker.indexOf('  let failureReported = false;');
  const end = worker.indexOf('  const logStats =', start);
  assert.ok(start >= 0 && end > start);
  let reports = 0;
  let logs = 0;
  let killed = false;
  const context = { failure: undefined, sceneId: 1, relayLog: {error:()=>logs++}, enter: f=>f(), relay: {relay_fail:()=>reports++, relay_killed:()=>killed} };
  vm.runInNewContext(worker.slice(start, end) + '\nglobalThis.check = checkFailure;', context);
  assert.equal(context.check(), false);
  context.failure = Error('runtime failed');
  for (let i=0;i<3;i++) assert.equal(context.check(), false);
  assert.equal(reports, 1);
  assert.equal(logs, 1);
  killed = true;
  assert.equal(context.check(), true);
  assert.equal(reports, 1);
});
