const test = require('node:test');
const assert = require('node:assert/strict');
const api = require('./CommsApi.js');

function stubOps(ops) {
  global.Deno = { core: { ops } };
}

test('consumeMessages resolves to the bare array the op returns', async () => {
  let topic;
  const messages = [{ sender: '0xabc', data: '{"a":1}' }];
  stubOps({ op_comms_consume_messages: async (t) => { topic = t; return messages; } });
  assert.deepEqual(await api.consumeMessages({ topic: 'room' }), messages);
  assert.equal(topic, 'room');
});

test('publish, subscribe and unsubscribe pass the topic and data and resolve to undefined', async () => {
  const calls = [];
  stubOps({
    op_comms_publish_data: async (...args) => { calls.push(['publish', ...args]); },
    op_comms_subscribe_topic: async (...args) => { calls.push(['subscribe', ...args]); },
    op_comms_unsubscribe_topic: async (...args) => { calls.push(['unsubscribe', ...args]); },
  });
  assert.equal(await api.publishData({ topic: 't', data: 'd' }), undefined);
  assert.equal(await api.subscribeToTopic({ topic: 't' }), undefined);
  assert.equal(await api.unsubscribeFromTopic({ topic: 't' }), undefined);
  assert.deepEqual(calls, [['publish', 't', 'd'], ['subscribe', 't'], ['unsubscribe', 't']]);
});

test('a rejected subscribe rejects the scene call', async () => {
  stubOps({ op_comms_subscribe_topic: async () => { throw new Error('topic is required'); } });
  await assert.rejects(api.subscribeToTopic({}), /topic is required/);
});
