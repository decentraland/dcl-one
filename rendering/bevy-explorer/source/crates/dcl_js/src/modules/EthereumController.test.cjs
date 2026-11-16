// Legacy API response envelopes follow ethereum_controller.proto.
const test = require('node:test');
const assert = require('node:assert/strict');
const api = require('./EthereumController.js');

function ops(value) { global.Deno = { core: { ops: value } }; }

test('legacy signing encodes UTF-8 and delegates to the engine wallet RPC', async () => {
  let sent;
  ops({
    op_sign_legacy_message: async (...args) => { sent = args; return '0xsigned'; },
  });
  const body = { message: { title: 'piñata 🦊', nonce: '42' } };
  const pending = api.signMessage(body);
  body.message.title = 'changed';
  const message = '# DCL Signed message\ntitle: piñata 🦊\nnonce: 42';
  const hexEncodedMessage = '0x' + Buffer.from(message).toString('hex');
  assert.deepEqual(await pending, { message, hexEncodedMessage, signature: '0xsigned' });
  assert.deepEqual(sent, ['title: piñata 🦊\nnonce: 42']);
});

test('wallet denial returns the legacy empty signature without retrying', async () => {
  let attempts = 0;
  ops({
    op_sign_legacy_message: async () => { attempts++; throw new Error('permission denied'); },
  });
  const result = await api.signMessage({ message: {} });
  assert.equal(result.message, '# DCL Signed message\n');
  assert.equal(result.signature, '');
  assert.equal(attempts, 1);
});

test('unavailable signing returns an empty signature and invalid input rejects', async () => {
  ops({ op_sign_legacy_message: async () => { throw new Error('wallet not connected'); } });
  assert.equal((await api.signMessage({ message: { x: 'y' } })).signature, '');
  await assert.rejects(api.signMessage({ message: null }), TypeError);
});

test('account and parsed dictionary retain protobuf response envelopes', async () => {
  ops({ op_get_user_account: async () => '0xabc' });
  assert.deepEqual(await api.getUserAccount({}), { address: '0xabc' });
  ops({ op_get_user_account: async () => null });
  assert.deepEqual(await api.getUserAccount({}), {});
  assert.deepEqual(await api.convertMessageToObject({
    message: '# DCL Signed message\nname: first\nname: last\nurl: https://example.org',
  }), { dict: { name: 'last', url: 'https' } });
  assert.deepEqual(await api.convertMessageToObject({ message: 'name: Alice' }), { dict: { name: 'Alice' } });
  await assert.rejects(api.convertMessageToObject({ message: 'malformed' }), TypeError);
});

test('dictionary keys do not modify the result prototype', async () => {
  const { dict } = await api.convertMessageToObject({ message: '__proto__: text\nconstructor: other' });
  assert.equal(Object.getPrototypeOf(dict), Object.prototype);
  assert.equal(Object.hasOwn(dict, '__proto__'), true);
  assert.equal(dict.__proto__, 'text');
  assert.equal(dict.constructor, 'other');
});

test('legacy payment returns an empty response and issues no wallet RPC', async () => {
  ops({ op_send_async: async () => { throw new Error('unexpected payment'); } });
  assert.deepEqual(await api.requirePayment({ toAddress: '0xabc', amount: 1, currency: 'ETH' }), { jsonAnyResponse: '{}' });
});

test('sendAsync retains JSON-RPC envelope and rejected methods do not dispatch', async () => {
  let sent;
  ops({ op_send_async: async (...args) => { sent = args; return '0xreceipt'; } });
  assert.deepEqual(await api.sendAsync({ id: 7, method: 'eth_sendTransaction', jsonParams: '[]' }), {
    jsonAnyResponse: '{"id":7,"jsonrpc":"2.0","result":"0xreceipt"}',
  });
  assert.deepEqual(sent, ['eth_sendTransaction', '[]']);
  for (const method of ['eth_sign', 'personal_sign']) {
    await assert.rejects(api.sendAsync({ id: 7, method, jsonParams: '[]' }), /not allowed/);
  }
  assert.deepEqual(sent, ['eth_sendTransaction', '[]']);
});

test('legacy signing snapshots own entries in JavaScript order with colon and prototype keys', async () => {
  let sent;
  ops({ op_sign_legacy_message: async fields => { sent = fields; return '0xsigned'; } });
  const fields = Object.assign(Object.create({ inherited: 'ignored' }), { z: 'a:b', 2: 'two', 1: 'one' });
  Object.defineProperty(fields, '__proto__', { value: 'literal', enumerable: true });
  const signed = await api.signMessage({ message: fields });
  assert.equal(sent, '1: one\n2: two\nz: a:b\n__proto__: literal');
  assert.equal(signed.message, '# DCL Signed message\n' + sent);
  assert.equal(signed.hexEncodedMessage, '0x' + Buffer.from(signed.message).toString('hex'));
  assert.equal(signed.signature, '0xsigned');
});

test('non-string wallet responses retain the empty signature envelope', async () => {
  for (const result of [null, undefined, {}, 42]) {
    ops({ op_sign_legacy_message: async () => result });
    assert.equal((await api.signMessage({ message: {} })).signature, '');
  }
});
