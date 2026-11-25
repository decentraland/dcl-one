import { engine, VideoPlayer } from '@dcl/sdk/ecs'
import { test } from '@dcl/sdk/src/testing'
import { assert } from '../helpers/assertions'
export * from '@dcl/sdk'

// The controls retain position zero when only playback settings change.
test('repeated video seeks preserve explicit intent', function* () {
  const entity = engine.addEntity()
  VideoPlayer.create(entity, { src: 'video.mp4', playing: true, volume: 1 })
  yield

  for (let revision = 1; revision <= 3; revision++) {
    const value = VideoPlayer.getMutable(entity)
    value.playing = true
    value.position = 0
    yield
    assert(VideoPlayer.get(entity).positionRevision === revision, 'Explicit seek advances revision')
  }
  VideoPlayer.getMutable(entity).playing = false
  yield
  VideoPlayer.getMutable(entity).volume = 0.5
  yield
  VideoPlayer.getMutable(entity).playing = true
  yield
  assert(VideoPlayer.get(entity).positionRevision === 3, 'Playback settings retain revision')
  VideoPlayer.getMutable(entity).playing = false
  yield
  const restart = VideoPlayer.getMutable(entity)
  restart.playing = true
  restart.position = 0
  yield
  assert(VideoPlayer.get(entity).positionRevision === 4, 'Restart after Pause advances revision')
})
