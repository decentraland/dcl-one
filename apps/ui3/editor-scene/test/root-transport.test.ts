import assert from 'node:assert/strict'
import { test } from 'node:test'
import { Engine, Schemas } from '@dcl/ecs'
import { NetworkEntity, SyncComponents } from '@dcl/ecs/dist/components'
import { syncFilter } from '@dcl/sdk/network/filter'
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer'
import { CrdtMessageProtocol, PutComponentOperation, PutNetworkComponentOperation } from '@dcl/ecs/dist/serialization/crdt'
import { CrdtMessageType } from '@dcl/ecs/dist/serialization/crdt/types'

for (const bytes of [12 * 1024, 12 * 1024 + 1, 17906, 18204]) {
  test(`root payload of ${bytes} bytes reaches the renderer; peer filtering precedes size validation`, async () => {
    const engine = Engine()
    const component = engine.defineComponent('audit::RootPayload', { value: Schemas.String })
    const payload = 'x'.repeat(bytes - 28)
    component.createOrReplace(engine.RootEntity, { value: payload })
    const peer = engine.addEntity()
    NetworkEntity(engine).createOrReplace(peer, { entityId: peer, networkId: 7 })
    SyncComponents(engine).createOrReplace(peer, { componentIds: [component.componentId] })
    component.createOrReplace(peer, { value: 'peer update' })
    const renderer: Uint8Array[] = []
    const packets: Uint8Array[] = []
    engine.addTransport({ type: 'renderer', filter: () => true, send: async data => { renderer.push(...[data].flat().map(chunk => Uint8Array.from(chunk))) } })
    engine.addTransport({ type: 'network', filter: syncFilter(engine), send: async data => { packets.push(...[data].flat().map(chunk => Uint8Array.from(chunk))) } })
    const errors: unknown[][] = []
    const original = console.error
    console.error = (...args) => { errors.push(args) }
    try { await engine.update(1 / 60) } finally { console.error = original }
    assert.deepEqual(errors, [])
    function values(chunks: Uint8Array[]) {
      const result: { entity: number; value: string }[] = []
      for (const chunk of chunks) {
        const buffer = new ReadWriteByteBuffer(chunk)
        let header
        while ((header = CrdtMessageProtocol.getHeader(buffer))) {
          const message = header.type === CrdtMessageType.PUT_COMPONENT ? PutComponentOperation.read(buffer)
            : header.type === CrdtMessageType.PUT_COMPONENT_NETWORK ? PutNetworkComponentOperation.read(buffer) : null
          if (!message) { buffer.incrementReadOffset(header.length); continue }
          if (message.componentId === component.componentId) result.push({ entity: message.entityId, value: component.schema.deserialize(new ReadWriteByteBuffer(message.data)).value })
        }
      }
      return result
    }
    assert.ok(values(renderer).some(value => value.entity === 0 && value.value === payload))
    assert.ok(packets.length > 0)
    assert.ok(packets.every(packet => packet.byteLength <= 12 * 1024))
    assert.deepEqual(values(packets), [{ entity: peer, value: 'peer update' }])
  })
}
