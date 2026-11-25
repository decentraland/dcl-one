const { test } = require('node:test')
const assert = require('node:assert/strict')
const protobuf = require('protobufjs')
const { PBVideoPlayer } = require('../out-js/decentraland/sdk/components/video_player.gen')

const legacy = { src: 'video.mp4', playing: true, position: 0 }
const golden = '0a09766964656f2e6d703410011d00000000'

test('players without a seek revision retain their wire bytes', () => {
  const bytes = PBVideoPlayer.encode(PBVideoPlayer.fromPartial(legacy)).finish()
  assert.equal(Buffer.from(bytes).toString('hex'), golden)
  assert.equal(PBVideoPlayer.decode(bytes).positionRevision, undefined)
})

test('a present zero revision survives wire and JSON round trips', () => {
  const value = PBVideoPlayer.fromPartial({ ...legacy, positionRevision: 0 })
  const bytes = PBVideoPlayer.encode(value).finish()
  assert.equal(Buffer.from(bytes).toString('hex'), golden + '5000')
  assert.deepEqual(PBVideoPlayer.decode(bytes), value)
  assert.deepEqual(PBVideoPlayer.fromJSON(PBVideoPlayer.toJSON(value)), value)
})

test('legacy readers skip the additive revision and preserve playback fields', () => {
  const old = protobuf.parse(`syntax = "proto3";
    message VideoPlayer { string src = 1; optional bool playing = 2; optional float position = 3; }
  `).root.lookupType('VideoPlayer')
  const bytes = PBVideoPlayer.encode(PBVideoPlayer.fromPartial({ ...legacy, positionRevision: 7 })).finish()
  const decoded = old.decode(bytes)
  assert.deepEqual(old.toObject(decoded), legacy)
  const relayed = PBVideoPlayer.decode(old.encode(decoded).finish())
  assert.equal(relayed.positionRevision, undefined)
  assert.equal(relayed.position, 0)
})
