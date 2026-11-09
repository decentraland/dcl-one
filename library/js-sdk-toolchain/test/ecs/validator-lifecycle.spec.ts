import { Engine, Entity, Schemas } from '../../packages/@dcl/ecs/src'
import { ReadWriteByteBuffer } from '../../packages/@dcl/ecs/src/serialization/ByteBuffer'
import { DeleteEntity } from '../../packages/@dcl/ecs/src/serialization/crdt/deleteEntity'

describe('validators registered for one entity', () => {
  const engine = Engine()
  const Position = engine.defineComponent('test::position', { x: Schemas.Int })
  const Marks = engine.defineValueSetComponentFromSchema('test::marks', Schemas.Map({ x: Schemas.Int }), {
    maxElements: 10,
    timestampFunction: (value) => value.x
  })
  const transport = { send: async () => {}, filter: () => true, type: 'test' as const }
  engine.addTransport(transport)

  function refuses(entity: Entity): boolean {
    return (
      !Position.__run_validateBeforeChange(entity, { x: 1 }, 'peer', 'creator') &&
      !Marks.__run_validateBeforeChange(entity, { x: 1 }, 'peer', 'creator')
    )
  }

  function guard(entity: Entity) {
    Position.create(entity, { x: 0 })
    Position.validateBeforeChange(entity, () => false)
    Marks.validateBeforeChange(entity, () => false)
  }

  describe('when the entity is removed by the scene', () => {
    let entity: Entity

    beforeEach(async () => {
      entity = engine.addEntity()
      guard(entity)
      expect(refuses(entity)).toBe(true)
      engine.removeEntity(entity)
      await engine.update(1)
    })

    it('should forget the validators with the entity', () => {
      expect(refuses(entity)).toBe(false)
    })
  })

  describe('when the entity is deleted by a CRDT message', () => {
    let entity: Entity

    beforeEach(async () => {
      entity = engine.addEntity()
      guard(entity)
      await engine.update(1)
      const buffer = new ReadWriteByteBuffer()
      DeleteEntity.write(entity, buffer)
      transport.onmessage!(buffer.toBinary())
      await engine.update(1)
    })

    it('should forget the validators with the entity', () => {
      expect(Position.has(entity)).toBe(false)
      expect(refuses(entity)).toBe(false)
    })
  })

  describe('when only the component is removed', () => {
    let entity: Entity

    beforeEach(() => {
      entity = engine.addEntity()
      guard(entity)
      Position.deleteFrom(entity)
    })

    it('should keep guarding the entity', () => {
      expect(refuses(entity)).toBe(true)
    })
  })

  describe('when another entity is removed', () => {
    let entity: Entity

    beforeEach(async () => {
      entity = engine.addEntity()
      guard(entity)
      const other = engine.addEntity()
      Position.create(other, { x: 0 })
      engine.removeEntity(other)
      await engine.update(1)
    })

    it('should keep its own validators', () => {
      expect(refuses(entity)).toBe(true)
    })
  })
})
