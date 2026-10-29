import { Transform, engine } from '@dcl/sdk/ecs'
import { BevyApi } from './bevy-api'
import { type LiveSceneInfo } from './bevy-api/interface'
import { sleep } from './utils'

export function getPlayerParcel(): { x: number; y: number } {
  const t = Transform.getOrNull(engine.PlayerEntity)
  if (t === null) return { x: 0, y: 0 }
  return {
    x: Math.floor(t.position.x / 16),
    y: Math.floor(t.position.z / 16)
  }
}

export async function getCurrentInspectableScene(): Promise<
  LiveSceneInfo | undefined
> {
  for (let attempt = 0; attempt < 120; attempt++) {
    const all = (await BevyApi.liveSceneInfo()) ?? []
    const parcel = getPlayerParcel()
    const scene = all.find(
      (s) =>
        !s.isPortable &&
        !s.isSuper &&
        s.parcels.some((p) => p.x === parcel.x && p.y === parcel.y)
    )
    if (scene !== undefined) return scene
    await sleep(500)
  }
  return undefined
}
