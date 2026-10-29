const { test } = require('node:test')
const assert = require('node:assert/strict')
const protobuf = require('protobufjs')
const { PeerClusterChange, PendingTakeover, TakeoverCompleted, PeerClusterSnapshot } = require('../out-js/decentraland/pulse/pulse_clusters.gen')

const OldChange = protobuf.parse(`syntax = "proto3";
message PeerClusterChange {
  string cluster_id = 1; string realm = 2; string session = 3;
  string displaced_session = 4; string displaced_cluster_id = 5;
}`).root.lookupType('PeerClusterChange')
const legacy = { clusterId: 'c', realm: 'm', session: 's', displacedSession: 'old', displacedClusterId: 'r' }
const takeover = PendingTakeover.fromPartial({ takeoverId: 'unique-1', displacedSession: 'old', displacedClusterId: 'room-a', displacedRealm: 'old-realm' })

test('legacy golden bytes remain unchanged and do not grant new admission authority', () => {
  const bytes = PeerClusterChange.encode(PeerClusterChange.fromPartial(legacy)).finish()
  assert.equal(Buffer.from(bytes).toString('hex'), '0a016312016d1a017322036f6c642a0172')
  assert.deepEqual(PeerClusterChange.decode(bytes), { ...legacy, pendingTakeovers: [], tokenNotBefore: undefined, assignmentVersion: 0, sourceIncarnation: '' })
})

test('a present zero floor differs from an absent floor on wire and in JSON', () => {
  const first = PeerClusterChange.fromPartial({ ...legacy, tokenNotBefore: 0 })
  const bytes = PeerClusterChange.encode(first).finish()
  assert.equal(Buffer.from(bytes).toString('hex'), '0a016312016d1a017322036f6c642a01724000')
  assert.equal(PeerClusterChange.decode(bytes).tokenNotBefore, 0)
  assert.equal(PeerClusterChange.fromJSON(PeerClusterChange.toJSON(first)).tokenNotBefore, 0)
  assert.equal(PeerClusterChange.fromJSON(legacy).tokenNotBefore, undefined)
})

test('multiple pending takeovers and confirmed floor survive wire and JSON round trips', () => {
  const input = PeerClusterChange.fromPartial({
    ...legacy, tokenNotBefore: 1770000001, assignmentVersion: 7, sourceIncarnation: 'pulse-a',
    pendingTakeovers: [takeover, { ...takeover, takeoverId: 'unique-2', displacedClusterId: 'room-b' }]
  })
  assert.deepEqual(PeerClusterChange.decode(PeerClusterChange.encode(input).finish()), input)
  assert.deepEqual(PeerClusterChange.fromJSON(PeerClusterChange.toJSON(input)), input)
})

test('old readers skip new fields and cannot preserve capability when relaying', () => {
  const bytes = PeerClusterChange.encode(PeerClusterChange.fromPartial({
    ...legacy, pendingTakeovers: [takeover], tokenNotBefore: 0, assignmentVersion: 7, sourceIncarnation: 'pulse-a'
  })).finish()
  assert.deepEqual(OldChange.toObject(OldChange.decode(bytes)), legacy)
  const relayed = PeerClusterChange.decode(OldChange.encode(OldChange.decode(bytes)).finish())
  assert.equal(relayed.tokenNotBefore, undefined)
  assert.equal(relayed.assignmentVersion, 0)
  assert.equal(relayed.sourceIncarnation, '')
  assert.deepEqual(relayed.pendingTakeovers, [])
})

test('retired draft field six is ignored and never grants credential authority', () => {
  const oldPlan = protobuf.Writer.create().uint32(10).string('old-epoch').uint32(24).int32(2).finish()
  const bytes = PeerClusterChange.encode(PeerClusterChange.fromPartial(legacy)).uint32(50).bytes(oldPlan).finish()
  const decoded = PeerClusterChange.decode(bytes)
  assert.equal(decoded.tokenNotBefore, undefined)
  assert.deepEqual(decoded.pendingTakeovers, [])
})

test('future fields are skipped without altering retained takeover state', () => {
  const input = PeerClusterChange.fromPartial({ ...legacy, pendingTakeovers: [takeover], tokenNotBefore: 42 })
  const bytes = PeerClusterChange.encode(input).uint32(802).string('future').finish()
  assert.deepEqual(PeerClusterChange.decode(bytes), input)
})

test('pending takeover and completion retain the exact operation identity', () => {
  assert.deepEqual(PendingTakeover.decode(PendingTakeover.encode(takeover).finish()), takeover)
  const completion = { takeoverId: takeover.takeoverId, revokeBefore: 1770000002 }
  assert.deepEqual(TakeoverCompleted.decode(TakeoverCompleted.encode(completion).finish()), completion)
})

test('assignment floor accepts the safe-integer boundary and rejects larger uint64 values', () => {
  const input = PeerClusterChange.fromPartial({ tokenNotBefore: Number.MAX_SAFE_INTEGER })
  assert.equal(PeerClusterChange.decode(PeerClusterChange.encode(input).finish()).tokenNotBefore, Number.MAX_SAFE_INTEGER)
  const bytes = protobuf.Writer.create().uint32(64).uint64('9007199254740992').finish()
  assert.throws(() => PeerClusterChange.decode(bytes), /larger than Number.MAX_SAFE_INTEGER/)
})

test('completion cutoff accepts the safe-integer boundary and rejects larger uint64 values', () => {
  const input = { takeoverId: 'id', revokeBefore: Number.MAX_SAFE_INTEGER }
  assert.deepEqual(TakeoverCompleted.decode(TakeoverCompleted.encode(input).finish()), input)
  const bytes = protobuf.Writer.create().uint32(16).uint64('9007199254740992').finish()
  assert.throws(() => TakeoverCompleted.decode(bytes), /larger than Number.MAX_SAFE_INTEGER/)
})

test('exact-session recovery snapshot retains source, ordering and cleanup authority', () => {
  const input = PeerClusterSnapshot.fromPartial({
    wallet: 'wallet', session: 'session', clusterId: 'cluster', realm: 'realm',
    pass: 12, sourceIncarnation: 'pulse-a', assignmentVersion: 7,
    pendingTakeovers: [takeover], tokenNotBefore: 0
  })
  assert.deepEqual(PeerClusterSnapshot.decode(PeerClusterSnapshot.encode(input).finish()), input)
  assert.deepEqual(PeerClusterSnapshot.fromJSON(PeerClusterSnapshot.toJSON(input)), input)
  const absent = PeerClusterSnapshot.fromPartial({ ...input, tokenNotBefore: undefined })
  assert.equal(PeerClusterSnapshot.decode(PeerClusterSnapshot.encode(absent).finish()).tokenNotBefore, undefined)
})
