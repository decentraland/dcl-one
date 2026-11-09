import { engine, Name } from '@dcl/sdk/ecs'
import { getPlayerPosition } from '@dcl/asset-packs/dist/helpers'
import { initAssetPacks } from '@dcl/asset-packs/dist/scene-entrypoint'

export function main() {
  initAssetPacks(engine)
  console.log({ name1: engine.getEntityOrNullByName('Magic Cube'), player: getPlayerPosition() })
  for (const [entity] of engine.getEntitiesWith(Name)) {
    console.log({ entity })
  }
}
