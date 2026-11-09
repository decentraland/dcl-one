import { Engine } from '../../../../packages/@dcl/ecs/src/engine'
import { Entity } from '../../../../packages/@dcl/ecs/src/engine/entity'
import { IEngine, Transport } from '../../../../packages/@dcl/ecs/src'
import * as components from '../../../../packages/@dcl/ecs/src/components'
import { ReadWriteByteBuffer } from '../../../../packages/@dcl/ecs/src/serialization/ByteBuffer'
import { readMessage } from '../../../../packages/@dcl/ecs/src/serialization/crdt/message'
import { PutNetworkComponentOperation } from '../../../../packages/@dcl/ecs/src/serialization/crdt/network/putComponentNetwork'
import { DeleteComponentNetwork } from '../../../../packages/@dcl/ecs/src/serialization/crdt/network/deleteComponentNetwork'
import { DeleteEntityNetwork } from '../../../../packages/@dcl/ecs/src/serialization/crdt/network/deleteEntityNetwork'
import { createServerValidator } from '../../../../packages/@dcl/sdk/network/server'
import { CommsMessage } from '../../../../packages/@dcl/sdk/network/binary-message-bus'

const NETWORK_ID = 7
const ENTITY = 900
const CREATOR = 'creator'
const OTHER = 'someone-else'

describe('when the server receives messages about a peer-created entity', () => {
  let engine: IEngine
  let transport: Transport
  let validator: ReturnType<typeof createServerValidator>
  let Transform: ReturnType<typeof components.Transform>
  let NetworkEntity: ReturnType<typeof components.NetworkEntity>
  let SyncComponents: ReturnType<typeof components.SyncComponents>
  let CreatedBy: ReturnType<typeof components.CreatedBy>
  let emitted: { type: number; data: Uint8Array; to?: string[] }[]

  function serialize<T>(component: { schema: { serialize(value: T, buffer: ReadWriteByteBuffer): void } }, value: T) {
    const data = new ReadWriteByteBuffer()
    component.schema.serialize(value, data)
    return data.toBinary()
  }

  function transformBytes(x: number): Uint8Array {
    return serialize(Transform, {
      position: { x, y: 0, z: 0 },
      rotation: { x: 0, y: 0, z: 0, w: 1 },
      scale: { x: 1, y: 1, z: 1 },
      parent: 0 as Entity
    })
  }

  function put(componentId: number, data: Uint8Array, timestamp = 1, entityId = ENTITY, networkId = NETWORK_ID) {
    const buf = new ReadWriteByteBuffer()
    PutNetworkComponentOperation.write(entityId as Entity, timestamp, componentId, networkId, data, buf)
    return buf.toBinary()
  }

  function deleteComponent(componentId: number, timestamp: number) {
    const buf = new ReadWriteByteBuffer()
    DeleteComponentNetwork.write(ENTITY as Entity, componentId, timestamp, NETWORK_ID, buf)
    return buf.toBinary()
  }

  function deleteEntity() {
    const buf = new ReadWriteByteBuffer()
    DeleteEntityNetwork.write(ENTITY as Entity, NETWORK_ID, buf)
    return buf.toBinary()
  }

  function concat(...chunks: Uint8Array[]): Uint8Array {
    const buf = new ReadWriteByteBuffer()
    for (const chunk of chunks) buf.writeBuffer(chunk, false)
    return buf.toBinary()
  }

  /** What the sync transport does with the validator's output. */
  async function feed(bytes: Uint8Array, sender: string) {
    const applied = validator.processServerMessages(bytes, sender)
    if (applied.byteLength) transport.onmessage!(applied)
    await engine.update(1)
  }

  function localEntity(): Entity | null {
    return validator.findExistingNetworkEntity({ networkId: NETWORK_ID, entityId: ENTITY as Entity } as any)
  }

  /** The component ids of every message relayed to the room since `emitted` was cleared. */
  function relayedComponentIds(): number[] {
    const ids: number[] = []
    for (const message of emitted) {
      if (message.type !== CommsMessage.CRDT) continue
      const buffer = new ReadWriteByteBuffer(message.data)
      let parsed
      while ((parsed = readMessage(buffer))) {
        ids.push('componentId' in parsed ? parsed.componentId : -1)
      }
    }
    return ids
  }

  function relayed(): boolean {
    return emitted.some((message) => message.type === CommsMessage.CRDT)
  }

  beforeEach(async () => {
    engine = Engine()
    transport = { type: 'network', filter: () => false, send: async () => {} }
    engine.addTransport(transport)
    Transform = components.Transform(engine)
    NetworkEntity = components.NetworkEntity(engine)
    SyncComponents = components.SyncComponents(engine)
    CreatedBy = components.CreatedBy(engine)
    components.NetworkParent(engine)
    emitted = []
    validator = createServerValidator({
      engine,
      binaryMessageBus: {
        emit: (type: number, data: Uint8Array, to?: string[]) => emitted.push({ type, data, to }),
        on: () => {}
      } as any
    })
    // The creator's syncEntity flow: the mapping, the sync list and the first component
    // leave in one frame, each with timestamp 1.
    await feed(
      concat(
        put(NetworkEntity.componentId, serialize(NetworkEntity, { entityId: ENTITY as Entity, networkId: NETWORK_ID })),
        put(SyncComponents.componentId, serialize(SyncComponents, { componentIds: [Transform.componentId] })),
        put(Transform.componentId, transformBytes(1))
      ),
      CREATOR
    )
  })

  describe('and they are the creation frame itself', () => {
    it('should map the entity to the creator', () => {
      const entity = localEntity()!
      expect(entity).not.toBeNull()
      expect(CreatedBy.get(entity).address).toBe(CREATOR)
    })

    it('should hold the mapping, the sync list and the component', () => {
      const entity = localEntity()!
      expect(NetworkEntity.get(entity)).toEqual({ entityId: ENTITY, networkId: NETWORK_ID })
      expect(SyncComponents.get(entity).componentIds).toEqual([Transform.componentId])
      expect(Transform.get(entity).position.x).toBe(1)
    })

    it('should relay all three to the room', () => {
      expect(relayedComponentIds()).toEqual([
        NetworkEntity.componentId,
        SyncComponents.componentId,
        Transform.componentId
      ])
    })
  })

  describe('and someone else deletes the entity', () => {
    beforeEach(async () => {
      emitted = []
      await feed(deleteEntity(), OTHER)
    })

    it('should keep the entity and its components', () => {
      const entity = localEntity()!
      expect(entity).not.toBeNull()
      expect(Transform.get(entity).position.x).toBe(1)
    })

    it('should not relay the delete', () => {
      expect(relayed()).toBe(false)
    })
  })

  describe('and the creator deletes the entity', () => {
    beforeEach(async () => {
      emitted = []
      await feed(deleteEntity(), CREATOR)
    })

    it('should remove the entity', () => {
      expect(localEntity()).toBeNull()
    })

    it('should relay the delete', () => {
      expect(relayed()).toBe(true)
    })
  })

  describe('and the creator deletes an entity whose component validator refuses deletes', () => {
    let seen: { newValue: unknown; senderAddress: string; createdBy: string }[]

    beforeEach(async () => {
      seen = []
      Transform.validateBeforeChange(({ newValue, senderAddress, createdBy }) => {
        seen.push({ newValue, senderAddress, createdBy })
        return newValue !== undefined
      })
      emitted = []
      await feed(deleteEntity(), CREATOR)
    })

    it('should ask the validator with an undefined new value', () => {
      expect(seen).toEqual([{ newValue: undefined, senderAddress: CREATOR, createdBy: CREATOR }])
    })

    it('should keep the entity and not relay the delete', () => {
      expect(localEntity()).not.toBeNull()
      expect(relayed()).toBe(false)
    })
  })

  describe('and the creator deletes a component whose validator refuses deletes', () => {
    let seen: unknown[]

    beforeEach(async () => {
      seen = []
      Transform.validateBeforeChange(({ newValue }) => {
        seen.push(newValue)
        return newValue !== undefined
      })
      emitted = []
      await feed(deleteComponent(Transform.componentId, 2), CREATOR)
    })

    it('should ask the validator with an undefined new value, not null', () => {
      expect(seen).toEqual([undefined])
    })

    it('should keep the component and answer the sender with the authoritative state', () => {
      expect(Transform.getOrNull(localEntity()!)).not.toBeNull()
      expect(relayed()).toBe(false)
      expect(emitted.filter((message) => message.type === CommsMessage.CRDT_AUTHORITATIVE)).toHaveLength(1)
    })
  })

  describe('and the creator deletes a component nobody guards', () => {
    beforeEach(async () => {
      emitted = []
      await feed(deleteComponent(Transform.componentId, 2), CREATOR)
    })

    it('should apply and relay the delete', () => {
      expect(Transform.getOrNull(localEntity()!)).toBeNull()
      expect(relayedComponentIds()).toEqual([Transform.componentId])
    })
  })

  describe('and someone else claims the entity through CreatedBy', () => {
    beforeEach(async () => {
      emitted = []
      await feed(put(CreatedBy.componentId, serialize(CreatedBy, { address: OTHER }), 50), OTHER)
    })

    it('should keep the creator on record', () => {
      expect(CreatedBy.get(localEntity()!).address).toBe(CREATOR)
    })

    it('should not relay the claim', () => {
      expect(relayed()).toBe(false)
    })

    it('should answer the sender with the real CreatedBy', () => {
      const corrections = emitted.filter(
        (message) => message.type === CommsMessage.CRDT_AUTHORITATIVE && message.to?.includes(OTHER)
      )
      expect(corrections).toHaveLength(1)
    })
  })

  describe('and a CreatedBy arrives for an entity the server has never seen', () => {
    beforeEach(async () => {
      emitted = []
      await feed(put(CreatedBy.componentId, serialize(CreatedBy, { address: OTHER }), 1, 901), OTHER)
    })

    it('should spend no entity on it', () => {
      expect(validator.findExistingNetworkEntity({ networkId: NETWORK_ID, entityId: 901 } as any)).toBeNull()
      expect(relayed()).toBe(false)
    })
  })

  describe('and someone else points the mapping at another network identity', () => {
    beforeEach(async () => {
      emitted = []
      await feed(
        put(NetworkEntity.componentId, serialize(NetworkEntity, { entityId: 600 as Entity, networkId: 0 }), 50),
        OTHER
      )
    })

    it('should keep the mapping', () => {
      expect(NetworkEntity.get(localEntity()!)).toEqual({ entityId: ENTITY, networkId: NETWORK_ID })
      expect(relayed()).toBe(false)
    })
  })

  describe('and someone else sends an unreadable NetworkEntity or deletes it', () => {
    beforeEach(async () => {
      emitted = []
      await feed(
        concat(
          put(NetworkEntity.componentId, new Uint8Array([1, 2]), 50),
          deleteComponent(NetworkEntity.componentId, 51)
        ),
        OTHER
      )
    })

    it('should keep the mapping', () => {
      expect(NetworkEntity.get(localEntity()!)).toEqual({ entityId: ENTITY, networkId: NETWORK_ID })
      expect(relayed()).toBe(false)
    })
  })

  describe('and someone else rewrites the sync list', () => {
    beforeEach(async () => {
      emitted = []
      await feed(
        put(SyncComponents.componentId, serialize(SyncComponents, { componentIds: [CreatedBy.componentId] }), 2),
        OTHER
      )
    })

    it('should keep the creator sync list and not relay the change', () => {
      expect(SyncComponents.get(localEntity()!).componentIds).toEqual([Transform.componentId])
      expect(relayed()).toBe(false)
    })
  })

  describe('and the creator rewrites the sync list', () => {
    beforeEach(async () => {
      emitted = []
      await feed(
        put(SyncComponents.componentId, serialize(SyncComponents, { componentIds: [Transform.componentId, 99] }), 2),
        CREATOR
      )
    })

    it('should apply and relay the change', () => {
      expect(SyncComponents.get(localEntity()!).componentIds).toEqual([Transform.componentId, 99])
      expect(relayedComponentIds()).toEqual([SyncComponents.componentId])
    })
  })

  describe('and someone else moves it', () => {
    beforeEach(async () => {
      emitted = []
      await feed(put(Transform.componentId, transformBytes(5), 2), OTHER)
    })

    it('should still apply and relay an ordinary component the scene left open to everyone', () => {
      expect(Transform.get(localEntity()!).position.x).toBe(5)
      expect(relayedComponentIds()).toEqual([Transform.componentId])
    })
  })
})
