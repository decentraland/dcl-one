// Read-only client for the realm provider's hot-scenes feed, the "what creators are building right now"
// rail of the overview page.
import { config } from '~/config'
import { HttpError } from '~/lib/http'

export type HotScene = {
  id: string
  name: string
  baseCoords: [number, number]
  usersTotalCount: number
  parcels: Array<[number, number]>
  thumbnail: string
}

const FETCH_TIMEOUT_MS = 10_000

/** The busiest scenes right now; the section limit. */
export const MAX_LIVE_SCENES = 6

export async function fetchHotScenes(): Promise<HotScene[]> {
  const response = await fetch(config.get('HOT_SCENES_URL'), {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS)
  })
  if (!response.ok) {
    await response.body?.cancel()
    throw new HttpError('hot scenes request failed', response.status)
  }
  const scenes: unknown = await response.json()
  return Array.isArray(scenes) ? scenes.filter(isHotScene) : []
}

// The feed is trusted for the rest of the shape; these are the fields a card cannot render without.
function isHotScene(value: unknown): value is HotScene {
  if (typeof value !== 'object' || value === null) return false
  const scene = value as Partial<HotScene>
  return (
    typeof scene.name === 'string' &&
    Array.isArray(scene.baseCoords) &&
    scene.baseCoords.length === 2 &&
    typeof scene.usersTotalCount === 'number'
  )
}

const isGenesisPlaza = (scene: HotScene) => scene.name.toLowerCase().includes('genesis plaza')

/** Occupied scenes by live users, busiest first, without the spawn plaza (it is always full and not a creator's work). */
export function selectLiveScenes(scenes: HotScene[]): HotScene[] {
  return scenes
    .filter(scene => !isGenesisPlaza(scene) && scene.usersTotalCount > 0)
    .sort((a, b) => b.usersTotalCount - a.usersTotalCount)
    .slice(0, MAX_LIVE_SCENES)
}

export const sceneCoordinates = (scene: HotScene) => `${scene.baseCoords[0]},${scene.baseCoords[1]}`

/** The scene's page on the places site. */
export const placeUrl = (scene: HotScene) => `${config.get('SITES_URL')}/places/place/${sceneCoordinates(scene)}`

export const placesUrl = () => `${config.get('SITES_URL')}/places`
