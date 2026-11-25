import { Engine, components, engine, VideoPlayer } from '../../../packages/@dcl/ecs/src'
import { ReadWriteByteBuffer } from '../../../packages/@dcl/ecs/src/serialization/ByteBuffer'
import { CrdtMessageType } from '../../../packages/@dcl/ecs/src/serialization/crdt/types'

function setup() {
  const engine = Engine()
  const player = components.VideoPlayer(engine)
  const entity = engine.addEntity()
  const flush = () =>
    Array.from(player.getCrdtUpdates()).map((message) => {
      if (message.type !== CrdtMessageType.PUT_COMPONENT) throw new Error('Expected a player update')
      return {
        timestamp: message.timestamp,
        value: player.schema.deserialize(new ReadWriteByteBuffer(message.data))
      }
    })
  player.create(entity, { src: 'video.mp4', playing: true, volume: 1 })
  flush()
  return { engine, player, entity, flush }
}

describe('VideoPlayer explicit seek intent', () => {
  it('tracks repeated requests through the default engine export used by scenes', () => {
    const entity = engine.addEntity()
    VideoPlayer.create(entity, { src: 'video.mp4' })
    Array.from(VideoPlayer.getCrdtUpdates())
    for (let revision = 1; revision <= 3; revision++) {
      VideoPlayer.getMutable(entity).position = 0
      expect(Array.from(VideoPlayer.getCrdtUpdates())).toHaveLength(1)
      expect(VideoPlayer.get(entity).positionRevision).toBe(revision)
    }
    engine.removeEntity(entity)
  })
  it('sends repeated Restarts and distinguishes Restart after Pause from Resume', () => {
    const { player, entity, flush } = setup()
    let timestamp = 0
    for (let revision = 1; revision <= 3; revision++) {
      const value = player.getMutable(entity)
      value.playing = true
      value.position = 0
      const updates = flush()
      expect(updates).toHaveLength(1)
      expect(updates[0].timestamp).toBeGreaterThan(timestamp)
      timestamp = updates[0].timestamp
      expect(updates[0].value).toMatchObject({ position: 0, positionRevision: revision, playing: true })
    }
    player.getMutable(entity).playing = false
    expect(flush()[0].value).toMatchObject({ position: 0, positionRevision: 3, playing: false })
    player.getMutable(entity).volume = 0.5
    expect(flush()[0].value).toMatchObject({ position: 0, positionRevision: 3, volume: 0.5 })
    player.getMutable(entity).playing = true
    expect(flush()[0].value).toMatchObject({ position: 0, positionRevision: 3, playing: true })
    player.getMutable(entity).playing = false
    flush()
    const restart = player.getMutable(entity)
    restart.playing = true
    restart.position = 0
    expect(flush()[0].value).toMatchObject({ position: 0, positionRevision: 4, playing: true })
  })

  it('preserves pending intent across mutable accesses and suppresses configuration no-ops', () => {
    const { player, entity, flush } = setup()
    player.getMutable(entity).position = 0
    player.getMutable(entity).volume = 0.5
    expect(flush()[0].value.positionRevision).toBe(1)
    expect(player.get(entity).position).toBe(0)
    player.getMutable(entity).volume = 0.5
    expect(flush()).toEqual([])
    expect(player.get(entity).positionRevision).toBe(1)
  })

  it('tracks every mutable entry point and explicit replacement', () => {
    const { player, entity, flush } = setup()
    player.getMutableOrNull(entity)!.position = 0
    expect(flush()[0].value.positionRevision).toBe(1)
    player.getOrCreateMutable(entity).position = 0
    expect(flush()[0].value.positionRevision).toBe(2)
    player.createOrReplace(entity, { src: 'video.mp4', position: 0 })
    expect(flush()[0].value.positionRevision).toBe(3)
    player.createOrReplace(entity, { src: 'video.mp4', position: 0 })
    expect(flush()[0].value.positionRevision).toBe(4)
  })

  it('starts a new request after clearing or deleting position', () => {
    const { player, entity, flush } = setup()
    player.getMutable(entity).position = 0
    flush()
    player.getMutable(entity).position = undefined
    expect(flush()[0].value).toMatchObject({ position: undefined, positionRevision: 1 })
    player.getMutable(entity).position = 0
    expect(flush()[0].value.positionRevision).toBe(2)
    delete player.getMutable(entity).position
    flush()
    player.getMutable(entity).position = 0
    expect(flush()[0].value.positionRevision).toBe(3)
  })

  it('wraps the revision as uint32 and does not retain it after entity deletion', () => {
    const { player, entity, flush } = setup()
    player.createOrReplace(entity, { src: 'video.mp4', positionRevision: 0xffffffff })
    flush()
    player.getMutable(entity).position = 0
    expect(flush()[0].value.positionRevision).toBe(0)
    player.deleteFrom(entity)
    Array.from(player.getCrdtUpdates())
    player.getOrCreateMutable(entity, { src: 'video.mp4', position: 0 })
    expect(flush()[0].value.positionRevision).toBe(1)
  })

  it('does not double-count when the component factory is called again', () => {
    const { engine, player, entity, flush } = setup()
    const samePlayer = components.VideoPlayer(engine)
    player.getMutable(entity).position = 0
    samePlayer.getMutable(entity).position = 0
    expect(flush()[0].value.positionRevision).toBe(2)
    expect(player.get(entity).positionRevision).toBe(2)
  })
})
