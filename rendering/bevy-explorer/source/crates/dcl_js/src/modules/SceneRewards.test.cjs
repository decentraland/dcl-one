const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function loadApi(ops = {}) {
  const context = { module: { exports: {} }, Deno: { core: { ops } } };
  vm.runInNewContext(fs.readFileSync(require.resolve('./SceneRewards.js'), 'utf8'), context);
  return context.module.exports;
}

// Objects made inside the module's context have that context's prototypes; compare their plain data.
function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

// scene_rewards.proto's GrantRewardError, plus ts-proto's UNRECOGNIZED.
const PROTOCOL_ENUM = [
  ['GRE_UNSPECIFIED', 0],
  ['GRE_NOT_AVAILABLE', 1],
  ['GRE_INVALID_REQUEST', 2],
  ['GRE_RATE_LIMITED', 3],
  ['GRE_UNAVAILABLE', 4],
  ['GRE_NOT_AUTHORIZED', 5],
  ['GRE_RECIPIENT_NOT_CONNECTED', 6],
  ['GRE_PROGRAM_PAUSED', 7],
  ['GRE_OUTSIDE_SCHEDULE', 8],
  ['GRE_BUDGET_EXHAUSTED', 9],
  ['GRE_PROGRAM_CAP_REACHED', 10],
  ['GRE_PLAYER_CAP_REACHED', 11],
  ['GRE_REQUIREMENT_NOT_MET', 12],
  ['GRE_RECIPIENT_NOT_ELIGIBLE', 13],
  ['UNRECOGNIZED', -1],
];

const REQUEST = {
  programId: '00000000-0000-4000-8000-000000000000',
  recipient: '0x' + 'ab'.repeat(20),
  outcomeId: '2026-12-03',
};

test('GrantRewardError has the protocol names and numbers, with the reverse mapping', () => {
  const { GrantRewardError } = loadApi();
  for (const [name, value] of PROTOCOL_ENUM) {
    assert.equal(GrantRewardError[name], value, name);
    assert.equal(GrantRewardError[value], name, String(value));
  }
  assert.equal(Object.keys(GrantRewardError).length, PROTOCOL_ENUM.length * 2);
});

test('grantReward hands the three fields to the engine, in order', async () => {
  let args;
  const { grantReward } = loadApi({
    op_grant_reward: async (...received) => {
      args = received;
      return { ok: false, error: 1, replayed: null };
    },
  });
  await grantReward(REQUEST);
  assert.deepEqual(plain(args), [REQUEST.programId, REQUEST.recipient, REQUEST.outcomeId]);
});

test('grantReward sends a field that is not a string as an empty one, for the engine to judge', async () => {
  let args;
  const { grantReward } = loadApi({
    op_grant_reward: async (...received) => {
      args = received;
      return { ok: false, error: 2, replayed: null };
    },
  });
  await grantReward({ programId: 7, recipient: { toString: () => REQUEST.recipient } });
  assert.deepEqual(plain(args), ['', '', '']);
  await grantReward(undefined);
  assert.deepEqual(plain(args), ['', '', '']);
});

function paid(replayed) {
  return { ok: true, error: null, replayed, receiptId: '9f1c-r', credits: '5' };
}

test('grantReward resolves a first payment with its whole receipt', async () => {
  const { grantReward } = loadApi({ op_grant_reward: async () => paid(false) });
  assert.deepEqual(plain(await grantReward(REQUEST)), {
    ok: true, replayed: false, receiptId: '9f1c-r', credits: '5',
  });
});

test('grantReward resolves a recovered payment with the same receipt and amount', async () => {
  const { grantReward } = loadApi({ op_grant_reward: async () => paid(true) });
  assert.deepEqual(plain(await grantReward(REQUEST)), {
    ok: true, replayed: true, receiptId: '9f1c-r', credits: '5',
  });
});

test('grantReward reads a payment missing part of its receipt as GRE_UNAVAILABLE', async () => {
  for (const partial of [
    { ok: true },
    { ...paid(false), replayed: null },
    { ...paid(false), receiptId: null },
    { ...paid(false), receiptId: '' },
    { ...paid(false), credits: null },
    { ...paid(false), credits: 5 },
    { ...paid(false), credits: '1.5' },
    { ...paid(false), credits: '0' },
    { ...paid(false), credits: '05' },
    { ...paid(false), receiptId: ' ' },
    { ...paid(false), receiptId: 'a b' },
    { ...paid(false), receiptId: 'r'.repeat(129) },
    { ...paid(false), ok: 1 },
    { ...paid(false), ok: 'true' },
  ]) {
    const { grantReward } = loadApi({ op_grant_reward: async () => partial });
    assert.deepEqual(plain(await grantReward(REQUEST)), { ok: false, error: 4 }, JSON.stringify(partial));
  }
});

test('grantReward resolves a refusal with the engine error number', async () => {
  for (const error of [1, 4, 11, 13]) {
    const { grantReward } = loadApi({ op_grant_reward: async () => ({ ok: false, error, replayed: null }) });
    assert.deepEqual(plain(await grantReward(REQUEST)), { ok: false, error });
  }
});

test('grantReward reads a refusal without a number as GRE_UNSPECIFIED, never as paid', async () => {
  const { grantReward } = loadApi({ op_grant_reward: async () => ({ ok: false }) });
  assert.deepEqual(plain(await grantReward(REQUEST)), { ok: false, error: 0 });
});

test('grantReward reads a missing answer from the host as GRE_UNAVAILABLE', async () => {
  for (const answer of [null, undefined]) {
    const { grantReward } = loadApi({ op_grant_reward: async () => answer });
    assert.deepEqual(plain(await grantReward(REQUEST)), { ok: false, error: 4 }, String(answer));
  }
});

test('grantReward resolves GRE_NOT_AVAILABLE on a host without the operation', async () => {
  const { grantReward } = loadApi();
  assert.deepEqual(plain(await grantReward(REQUEST)), { ok: false, error: 1 });
});
