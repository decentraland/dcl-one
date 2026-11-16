import { engine, type IEngine, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// {{TITLE}} — a library scaffolded by dcl-scene-cli init --project library.
// A scene passes its engine in, so the library and the scene share one engine
// and one set of components even when two copies of the SDK are installed.
export function initLibrary(scene: IEngine) {
  scene.addSystem(circularSystem(scene))
}

export function circularSystem(scene: IEngine) {
  const TransformOf = scene.getComponent(Transform.componentId) as typeof Transform
  return function circularSystem(dt: number) {
    for (const [entity] of scene.getEntitiesWith(TransformOf)) {
      const transform = TransformOf.getMutable(entity)
      transform.rotation = Quaternion.multiply(
        transform.rotation,
        Quaternion.fromAngleAxis(dt * 100, Vector3.Up())
      )
    }
  }
}

// `dcl-scene-cli start` previews the library as a scene through this entry.
export function main() {
  const cube = engine.addEntity()
  Transform.create(cube, { position: Vector3.create(8, 1, 8) })
  MeshRenderer.setBox(cube)
  initLibrary(engine)
}
