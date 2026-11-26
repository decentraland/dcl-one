import type { Entity, IEngine } from '../../engine'
import type { LastWriteWinElementSetComponentDefinition } from '../../engine/component'
import { VideoPlayer } from '../generated/index.gen'
import type { PBVideoPlayer } from '../generated/pb/decentraland/sdk/components/video_player.gen'

// Keep explicit assignments distinguishable from playback settings that retain the last seek.
// Shared factories must reuse the accessor rather than count the same assignment twice.
const positionSetters = new WeakMap<PBVideoPlayer, (position: number | undefined) => void>()

function watchPosition(value: PBVideoPlayer): PBVideoPlayer {
  const setter = positionSetters.get(value)
  if (setter && Object.getOwnPropertyDescriptor(value, 'position')?.set === setter) return value

  let position = value.position
  const setPosition = (next: number | undefined) => {
    position = next
    if (next !== undefined) value.positionRevision = ((value.positionRevision ?? 0) + 1) >>> 0
  }
  Object.defineProperty(value, 'position', {
    enumerable: true,
    configurable: true,
    get: () => position,
    set: setPosition
  })
  positionSetters.set(value, setPosition)
  return value
}

export function defineVideoPlayerComponent(
  engine: Pick<IEngine, 'defineComponentFromSchema'>
): LastWriteWinElementSetComponentDefinition<PBVideoPlayer> {
  const component = VideoPlayer(engine)
  function request(entity: Entity, value?: PBVideoPlayer): PBVideoPlayer | undefined {
    if (value === undefined) return undefined
    if (value.position === undefined) return { ...value }
    const current = component.getOrNull(entity)
    if (
      current &&
      value.positionRevision === current.positionRevision &&
      value.src === current.src &&
      value.position === current.position &&
      (Object.keys({ ...current, ...value }) as (keyof PBVideoPlayer)[]).some(
        (key) => key !== 'positionRevision' && value[key] !== current[key]
      )
    ) {
      return { ...value }
    }
    const previous = current?.positionRevision ?? value.positionRevision ?? 0
    return { ...value, positionRevision: (previous + 1) >>> 0 }
  }
  return {
    ...component,
    create(entity, value) {
      return watchPosition(component.create(entity, request(entity, value)))
    },
    createOrReplace(entity, value) {
      return watchPosition(component.createOrReplace(entity, request(entity, value)))
    },
    getMutable(entity) {
      return watchPosition(component.getMutable(entity))
    },
    getMutableOrNull(entity) {
      const value = component.getMutableOrNull(entity)
      return value === null ? null : watchPosition(value)
    },
    getOrCreateMutable(entity, value) {
      return component.has(entity)
        ? watchPosition(component.getMutable(entity))
        : watchPosition(component.create(entity, request(entity, value)))
    }
  }
}
