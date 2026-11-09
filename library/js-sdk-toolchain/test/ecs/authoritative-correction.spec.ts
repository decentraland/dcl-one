import { Engine } from '../../packages/@dcl/ecs/src/engine'
import { Entity } from '../../packages/@dcl/ecs/src/engine/entity'
import { Transport } from '../../packages/@dcl/ecs/src'
import {
  GrowOnlyValueSetComponentDefinition,
  LastWriteWinElementSetComponentDefinition
} from '../../packages/@dcl/ecs/src/engine/component'
import { Schemas } from '../../packages/@dcl/ecs/src/schemas'
import { ReadWriteByteBuffer } from '../../packages/@dcl/ecs/src/serialization/ByteBuffer'
import {
  AuthoritativePutComponentOperation,
  CrdtMessageProtocol,
  CrdtMessageType,
  DeleteComponent,
  PutComponentOperation
} from '../../packages/@dcl/ecs/src/serialization/crdt'

type Seen = { type: CrdtMessageType; timestamp?: number; value?: number }

/**
 * A server correction arrives through the network transport as an
 * AUTHORITATIVE_PUT_COMPONENT. The renderer transport must see its outcome as a
 * PUT_COMPONENT whose timestamp beats every one it holds, since renderers parse message
 * types 1 to 4 only and keep their own last-write-wins timestamps.
 */
describe('when the server corrects a component the scene changed', () => {
  const schema = Schemas.Map({ x: Schemas.Int })

  let engine: ReturnType<typeof Engine>
  let Position: LastWriteWinElementSetComponentDefinition<{ x: number }>
  let entity: Entity
  let rendererSaw: Seen[]
  let networkSaw: Seen[]
  let network: Transport

  // A transport that records every message type, with timestamp and value for PUTs.
  function recorder(type: string, log: Seen[]): Transport {
    return {
      type,
      filter: () => true,
      send: async (message) => {
        const buffer = new ReadWriteByteBuffer(message as Uint8Array)
        let header
        while ((header = CrdtMessageProtocol.getHeader(buffer))) {
          if (header.type === CrdtMessageType.PUT_COMPONENT) {
            const put = PutComponentOperation.read(buffer)!
            log.push({
              type: header.type,
              timestamp: put.timestamp,
              value: schema.deserialize(new ReadWriteByteBuffer(put.data)).x
            })
          } else if (header.type === CrdtMessageType.DELETE_COMPONENT) {
            log.push({ type: header.type, timestamp: DeleteComponent.read(buffer)!.timestamp })
          } else {
            log.push({ type: header.type })
            buffer.incrementReadOffset(header.length)
          }
        }
      }
    }
  }

  function serialize(x: number): Uint8Array {
    const buffer = new ReadWriteByteBuffer()
    schema.serialize({ x }, buffer)
    return buffer.toBinary()
  }

  function correction(componentId: number, timestamp: number, data: Uint8Array) {
    const frame = new ReadWriteByteBuffer()
    AuthoritativePutComponentOperation.write(entity, timestamp, componentId, data, frame)
    network.onmessage!(frame.toBinary())
  }

  beforeEach(async () => {
    engine = Engine()
    Position = engine.defineComponentFromSchema('test::position', schema)
    rendererSaw = []
    networkSaw = []
    network = recorder('network', networkSaw)
    engine.addTransport(recorder('renderer', rendererSaw))
    engine.addTransport(network)
    entity = engine.addEntity()
    Position.create(entity, { x: 1 })
    await engine.update(1)
    // The write the server will reject: the renderer now holds timestamp 2.
    Position.getMutable(entity).x = 2
    await engine.update(1)
    rendererSaw.length = 0
    networkSaw.length = 0
  })

  describe('and the correction carries the lower timestamp the server holds', () => {
    beforeEach(async () => {
      correction(Position.componentId, 1, serialize(7))
      await engine.update(1)
    })

    it('should hold the server value', () => {
      expect(Position.get(entity).x).toBe(7)
    })

    it('should send the renderer the value as a PUT_COMPONENT, never the type-8 message', () => {
      expect(rendererSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 3, value: 7 }])
    })

    it('should echo the corrected value to the network as well', () => {
      expect(networkSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 3, value: 7 }])
    })

    describe('and the same correction arrives again', () => {
      beforeEach(async () => {
        rendererSaw.length = 0
        correction(Position.componentId, 1, serialize(7))
        await engine.update(1)
      })

      it('should send nothing more', () => {
        expect(rendererSaw).toEqual([])
      })
    })

    describe('and the scene writes the component again', () => {
      beforeEach(async () => {
        rendererSaw.length = 0
        Position.getMutable(entity).x = 9
        await engine.update(1)
      })

      it('should keep counting above the correction, so the renderer accepts the write', () => {
        expect(rendererSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 4, value: 9 }])
      })
    })
  })

  describe('and the correction carries a higher timestamp than the local row', () => {
    beforeEach(async () => {
      correction(Position.componentId, 10, serialize(7))
      await engine.update(1)
    })

    it('should continue from the server timestamp', () => {
      expect(rendererSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 11, value: 7 }])
    })
  })

  describe('and the scene had deleted the component', () => {
    beforeEach(async () => {
      Position.deleteFrom(entity)
      await engine.update(1)
      expect(rendererSaw).toEqual([{ type: CrdtMessageType.DELETE_COMPONENT, timestamp: 3 }])
      rendererSaw.length = 0
      correction(Position.componentId, 1, serialize(7))
      await engine.update(1)
    })

    it('should restore it on the renderer above the delete timestamp', () => {
      expect(Position.get(entity).x).toBe(7)
      expect(rendererSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 4, value: 7 }])
    })
  })

  describe('and the correction names an entity this engine never wrote', () => {
    let other: Entity

    beforeEach(async () => {
      other = engine.addEntity()
      const frame = new ReadWriteByteBuffer()
      AuthoritativePutComponentOperation.write(other, 5, Position.componentId, serialize(7), frame)
      network.onmessage!(frame.toBinary())
      await engine.update(1)
    })

    it('should create the row and send it on', () => {
      expect(Position.get(other).x).toBe(7)
      expect(rendererSaw).toEqual([{ type: CrdtMessageType.PUT_COMPONENT, timestamp: 6, value: 7 }])
    })
  })

  describe('and the correction is for a grow-only set', () => {
    let Stream: GrowOnlyValueSetComponentDefinition<{ x: number }>

    beforeEach(async () => {
      Stream = engine.defineValueSetComponentFromSchema('test::stream', schema, {
        timestampFunction: (value) => value.x,
        maxElements: 10
      })
      correction(Stream.componentId, 1, serialize(7))
      await engine.update(1)
    })

    it('should ignore it', () => {
      expect(Stream.get(entity).size).toBe(0)
      expect(rendererSaw).toEqual([])
    })
  })

  describe('and the correction payload is unreadable', () => {
    let error: jest.SpyInstance

    beforeEach(async () => {
      error = jest.spyOn(console, 'error').mockImplementation(() => {})
      correction(Position.componentId, 1, new Uint8Array([1]))
      await engine.update(1)
    })

    afterEach(() => {
      error.mockRestore()
    })

    it('should report it and keep the local value', () => {
      expect(error).toHaveBeenCalled()
      expect(Position.get(entity).x).toBe(2)
      expect(rendererSaw).toEqual([])
    })
  })
})
