const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

function loadApi(ops = {}) {
  const context = { module: { exports: {} }, Deno: { core: { ops } } };
  vm.runInNewContext(fs.readFileSync(require.resolve('./SceneCredits.js'), 'utf8'), context);
  return context.module.exports;
}

// Objects made inside the module's context have that context's prototypes; compare their plain data.
function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

// scene_credits.proto's GrantCreditsError, plus ts-proto's UNRECOGNIZED.
const PROTOCOL_ENUM = [
  ['GCE_UNSPECIFIED', 0],
  ['GCE_NOT_AVAILABLE', 1],
  ['GCE_INVALID_REQUEST', 2],
  ['GCE_RATE_LIMITED', 3],
  ['GCE_UNAVAILABLE', 4],
  ['GCE_NOT_AUTHORIZED', 5],
  ['GCE_RECIPIENT_NOT_CONNECTED', 6],
  ['GCE_PROGRAM_PAUSED', 7],
  ['GCE_OUTSIDE_SCHEDULE', 8],
  ['GCE_BUDGET_EXHAUSTED', 9],
  ['GCE_PROGRAM_CAP_REACHED', 10],
  ['GCE_PLAYER_CAP_REACHED', 11],
  ['GCE_REQUIREMENT_NOT_MET', 12],
  ['GCE_RECIPIENT_NOT_ELIGIBLE', 13],
  ['UNRECOGNIZED', -1],
];

const REQUEST = {
  programId: '00000000-0000-4000-8000-000000000000',
  recipient: '0x' + 'ab'.repeat(20),
  outcomeId: '2026-12-03',
};

test('GrantCreditsError has the protocol names and numbers, with the reverse mapping', () => {
  const { GrantCreditsError } = loadApi();
  for (const [name, value] of PROTOCOL_ENUM) {
    assert.equal(GrantCreditsError[name], value, name);
    assert.equal(GrantCreditsError[value], name, String(value));
  }
  assert.equal(Object.keys(GrantCreditsError).length, PROTOCOL_ENUM.length * 2);
});

test('grantCredits hands the three fields to the engine, in order', async () => {
  let args;
  const { grantCredits } = loadApi({
    op_grant_credits: async (...received) => {
      args = received;
      return { ok: false, error: 1, replayed: null };
    },
  });
  await grantCredits(REQUEST);
  assert.deepEqual(plain(args), [REQUEST.programId, REQUEST.recipient, REQUEST.outcomeId]);
});

test('grantCredits sends a field that is not a string as an empty one, for the engine to judge', async () => {
  let args;
  const { grantCredits } = loadApi({
    op_grant_credits: async (...received) => {
      args = received;
      return { ok: false, error: 2, replayed: null };
    },
  });
  await grantCredits({ programId: 7, recipient: { toString: () => REQUEST.recipient } });
  assert.deepEqual(plain(args), ['', '', '']);
  await grantCredits(undefined);
  assert.deepEqual(plain(args), ['', '', '']);
});

function paid(replayed) {
  return { ok: true, error: null, replayed, receiptId: '9f1c-r', credits: '5' };
}

test('grantCredits resolves a first payment with its whole receipt', async () => {
  const { grantCredits } = loadApi({ op_grant_credits: async () => paid(false) });
  assert.deepEqual(plain(await grantCredits(REQUEST)), {
    ok: true, replayed: false, receiptId: '9f1c-r', credits: '5',
  });
});

test('grantCredits resolves a recovered payment with the same receipt and amount', async () => {
  const { grantCredits } = loadApi({ op_grant_credits: async () => paid(true) });
  assert.deepEqual(plain(await grantCredits(REQUEST)), {
    ok: true, replayed: true, receiptId: '9f1c-r', credits: '5',
  });
});

test('grantCredits reads a payment missing part of its receipt as GCE_UNAVAILABLE', async () => {
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
    const { grantCredits } = loadApi({ op_grant_credits: async () => partial });
    assert.deepEqual(plain(await grantCredits(REQUEST)), { ok: false, error: 4 }, JSON.stringify(partial));
  }
});

test('grantCredits resolves a refusal with the engine error number', async () => {
  for (const error of [1, 4, 11, 13]) {
    const { grantCredits } = loadApi({ op_grant_credits: async () => ({ ok: false, error, replayed: null }) });
    assert.deepEqual(plain(await grantCredits(REQUEST)), { ok: false, error });
  }
});

test('grantCredits reads a refusal without a number as GCE_UNSPECIFIED, never as paid', async () => {
  const { grantCredits } = loadApi({ op_grant_credits: async () => ({ ok: false }) });
  assert.deepEqual(plain(await grantCredits(REQUEST)), { ok: false, error: 0 });
});

test('grantCredits reads a missing answer from the host as GCE_UNAVAILABLE', async () => {
  for (const answer of [null, undefined]) {
    const { grantCredits } = loadApi({ op_grant_credits: async () => answer });
    assert.deepEqual(plain(await grantCredits(REQUEST)), { ok: false, error: 4 }, String(answer));
  }
});

test('grantCredits resolves GCE_NOT_AVAILABLE on a host without the operation', async () => {
  const { grantCredits } = loadApi();
  assert.deepEqual(plain(await grantCredits(REQUEST)), { ok: false, error: 1 });
});
