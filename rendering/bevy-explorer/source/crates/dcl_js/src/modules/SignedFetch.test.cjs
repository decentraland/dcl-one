const test = require('node:test');
const assert = require('node:assert/strict');
const api = require('./SignedFetch.js');

test('signedFetch captures request and protects identity headers', async () => {
  let resolveSign, signed, sent;
  global.Deno = { core: { ops: { op_signed_fetch_headers: (...args) => { signed = args; return new Promise(r => resolveSign = r); } } } };
  global.fetch = async (url, init) => { sent = {url, init}; return {ok:true,status:200,statusText:'OK',headers:{},text:async()=>''}; };
  const input = { url: 'https://example.org/Path', init: { method:'POST', body:'before', headers: {'X-Identity-Metadata':'forged', 'x-authoritative-scope':'forged', accept:'text/plain'} } };
  const pending = api.signedFetch(input);
  input.url = 'https://evil.example/'; input.init.method = 'DELETE'; input.init.body = 'after'; input.init.headers.accept = 'changed';
  resolveSign([['x-identity-metadata','trusted'],['x-identity-auth-chain-0','chain']]);
  await pending;
  assert.deepEqual(signed, ['https://example.org/Path','POST','before']);
  assert.equal(sent.url, 'https://example.org/Path'); assert.equal(sent.init.method, 'POST'); assert.equal(sent.init.body, 'before');
  const headers = new Headers(sent.init.headers); assert.equal(headers.get('x-identity-metadata'),'trusted'); assert.equal(headers.get('x-authoritative-scope'),null); assert.equal(headers.get('accept'),'text/plain');
});

test('getHeaders passes present empty body through', async () => {
  let args;
  global.Deno = {core:{ops:{op_signed_fetch_headers: async (...a) => { args=a; return []; }}}};
  await api.getHeaders({url:'https://example.org/', init:{body:''}});
  assert.deepEqual(args, ['https://example.org/', undefined, '']);
});

test('reserved headers are removed for object, pairs and Headers input', async () => {
  for (const headers of [
    {'X-Identity-Auth-Chain-0':'bad', 'X-Identity-Metadata':'bad', 'X-Authoritative-Scope':'bad', accept:'ok'},
    [['X-Identity-Auth-Chain-0','bad'], ['X-Identity-Metadata','bad'], ['X-Authoritative-Scope','bad'], ['accept','ok']],
    new Headers([['X-Identity-Auth-Chain-0','bad'], ['X-Identity-Metadata','bad'], ['X-Authoritative-Scope','bad'], ['accept','ok']]),
  ]) {
    global.Deno = {core:{ops:{op_signed_fetch_headers: async () => [['x-identity-metadata','engine']]}}};
    let sent;
    global.fetch = async (_, init) => { sent = new Headers(init.headers); return {ok:true,status:200,statusText:'OK',headers:{},text:async()=>''}; };
    await api.signedFetch({url:'https://example.org/', init:{headers}});
    assert.equal(sent.get('x-identity-auth-chain-0'), null);
    assert.equal(sent.get('x-authoritative-scope'), null);
    assert.equal(sent.get('x-identity-metadata'), 'engine');
    assert.equal(sent.get('accept'), 'ok');
  }
});

test('works in a scene global without a Headers constructor', async () => {
  const vm = require('node:vm');
  const fs = require('node:fs');
  let sent;
  const context = { module:{exports:{}}, Deno:{core:{ops:{op_signed_fetch_headers:async()=>[['x-identity-metadata','engine']]}}},
    fetch:async (url,init) => { sent = {url,init}; return {ok:true,status:200,statusText:'OK',headers:{},text:async()=>''}; } };
  vm.runInNewContext(fs.readFileSync(require.resolve('./SignedFetch.js'), 'utf8'), context);
  await context.module.exports.signedFetch({url:'https://example.org/',init:{headers:{'X-Identity-Metadata':'bad',accept:'ok'}}});
  assert.equal(sent.init.headers.find(([key]) => key.toLowerCase() === 'x-identity-metadata')[1], 'engine');
});
