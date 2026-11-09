import { Entity, Schemas, engine, SyncComponents, NetworkEntity } from '../../../packages/@dcl/ecs/dist'
import { engineToCrdt } from '../../../packages/@dcl/sdk/network/state'
import { serializeCrdtMessages } from '../../../packages/@dcl/sdk/internal/transports/logger'

describe('It should dump the engine to a crdt buffer', () => {
  it('Should dump the state', async () => {
    // const engine = Engine()
    const Component = engine.defineComponent('myComponent', { id: Schemas.String })

    const LocalOnly = engine.defineComponent('localOnly', { id: Schemas.String })

    // Entity that should be added to the dump
    const entity = engine.addEntity()
    NetworkEntity.create(entity, { networkId: 1, entityId: entity })
    Component.create(entity, { id: 'boedo' })
    LocalOnly.create(entity, { id: 'stays home' })
    SyncComponents.create(entity, { componentIds: [Component.componentId] })

    // Networked, but the scene never said what to sync: nothing leaves
    const silent = engine.addEntity()
    NetworkEntity.create(silent, { networkId: 1, entityId: silent })
    Component.create(silent, { id: 'silent' })

    // Ignore this entity
    const fakeEntity = 10 as Entity
    Component.create(fakeEntity)

    // Update dirty values
    await engine.update(1)

    const crdtBuffer = engineToCrdt(engine)
    const messages = Array.from(serializeCrdtMessages('test', crdtBuffer[0], engine))
    expect(messages).toHaveLength(3)
    expect(messages.join('\n')).not.toContain('localOnly')
    expect(messages.join('\n')).not.toContain('silent')
  })
})
