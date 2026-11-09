import test from 'node:test';
import assert from 'node:assert/strict';
import { room_drop } from './livekit_web_bindings.js';

test('dropping a cancelled successful room disconnects it and releases listeners', () => {
  const operations = [];
  room_drop({
    disconnect() { operations.push('disconnect'); return Promise.resolve(); },
    removeAllListeners() { operations.push('release'); }
  });
  assert.deepEqual(operations, ['disconnect', 'release']);
});

test('dropping a room still releases listeners if asynchronous disconnect fails', async () => {
  let released = false;
  room_drop({
    disconnect() { return Promise.reject(new Error('already disconnected')); },
    removeAllListeners() { released = true; }
  });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(released, true);
});
