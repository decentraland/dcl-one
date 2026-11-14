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

test('web preload exposes legacy modules and subscribes only on first SocialController require', async () => {
  const worker = fs.readFileSync(require.resolve('../../../../deploy/web/engine/sandbox_worker.js'), 'utf8');
  const start = worker.indexOf('const defer =');
  const end = worker.indexOf('// the exports scene code runs against:', start);
  assert.ok(start >= 0 && end > start);
  let subscriptions = 0;
  let pulls = 0;
  const requested = [];
  const context = {
    jsPreamble: 'const Deno = globalThis.Deno; const console = globalThis.console;',
    jsProxy: { Deno: {core: {ops: {
      op_subscribe_legacy_avatar_events:()=>subscriptions++,
      op_pull_legacy_avatar_events:async()=>{pulls++; return [];},
      op_scene_id:()=> 'scene-id',
      op_get_permissions:async items=>items.map(()=>false)
    }}}, console: {log:()=>{}, error:()=>{}} }
  };
  vm.runInNewContext(worker.slice(start, end) + '\nglobalThis.preload = preloadModules; globalThis.loadModule = require;', context);
  const names = ['DevTools','ParcelIdentity','Permissions','SocialController'];
  await context.preload({}, (_state,key)=>{
    requested.push(key);
    if (key === '~system/BevyExplorerApi') throw Error('not a super-user scene');
    const name = key.slice('~system/'.length);
    return names.includes(name) ? fs.readFileSync(require.resolve(`./${name}.js`), 'utf8') : 'module.exports = {}';
  });
  for (const name of names) assert.ok(requested.includes('~system/'+name), name+' was not prefetched');
  assert.equal(subscriptions, 0, 'prefetch must not register an unused scene');
  assert.deepEqual(plain(await context.loadModule('~system/ParcelIdentity').getSceneId()), {sceneId:'scene-id'});
  assert.deepEqual(plain(await context.loadModule('~system/Permissions').hasPermission({permission:2})), {hasPermission:false});
  assert.deepEqual(plain(await context.loadModule('~system/DevTools').event({type:'Other.event',jsonPayload:'{}'})), {});
  const social = context.loadModule('~system/SocialController');
  assert.equal(subscriptions, 1);
  assert.equal(context.loadModule('~system/SocialController'), social);
  assert.equal(subscriptions, 1, 'repeated require must reuse the same lifetime');
  assert.deepEqual(plain(await social.pullAvatarEvents()), {events:[]});
  assert.deepEqual(plain(await social.pullAvatarEvents()), {events:[]});
  assert.equal(pulls, 2);
  assert.throws(()=>context.loadModule('~system/BevyExplorerApi'));
  assert.throws(()=>context.loadModule('~system/Unknown'));
});
