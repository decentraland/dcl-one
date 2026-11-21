import type { Entity } from '@dcl/sdk/ecs'

export function start(src: string, entity: Entity) {
  console.log(`counter script started on entity ${entity} from ${src}`)
}
