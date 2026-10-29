import { engine } from '@dcl/sdk/ecs'
import { type ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer'

export type CameraLayersValue = { layers: number[] }

function pushVarint(out: number[], value: number): void {
  let v = value >>> 0
  while (v > 0x7f) {
    out.push((v & 0x7f) | 0x80)
    v >>>= 7
  }
  out.push(v)
}

function readVarint(reader: ReadWriteByteBuffer): number {
  let value = 0
  let shift = 0
  while (reader.remainingBytes() > 0) {
    const byte = reader.readUint8()
    value |= (byte & 0x7f) << shift
    if ((byte & 0x80) === 0) break
    shift += 7
  }
  return value >>> 0
}

const CameraLayersSchema = {
  serialize(value: CameraLayersValue, builder: ReadWriteByteBuffer): void {
    const packed: number[] = []
    for (const layer of value.layers) pushVarint(packed, layer)
    if (packed.length === 0) return
    const bytes: number[] = [0x0a]
    pushVarint(bytes, packed.length)
    builder.writeBuffer(new Uint8Array([...bytes, ...packed]), false)
  },
  deserialize(reader: ReadWriteByteBuffer): CameraLayersValue {
    const layers: number[] = []
    while (reader.remainingBytes() > 0) {
      const tag = readVarint(reader)
      const field = tag >>> 3
      const wire = tag & 7
      if (field === 1 && wire === 2) {
        const end = reader.currentReadOffset() + readVarint(reader)
        while (reader.currentReadOffset() < end) layers.push(readVarint(reader))
      } else if (field === 1 && wire === 0) {
        layers.push(readVarint(reader))
      } else if (wire === 2) {
        reader.incrementReadOffset(readVarint(reader))
      } else if (wire === 0) {
        readVarint(reader)
      } else {
        break
      }
    }
    return { layers }
  },
  create(): CameraLayersValue {
    return { layers: [] }
  },
  jsonSchema: {
    type: 'object',
    properties: {},
    serializationType: 'protocol-buffer',
    protocolBuffer: 'PBCameraLayers'
  } as const
}

export const CameraLayers = engine.defineComponentFromSchema(
  'one::CameraLayers',
  CameraLayersSchema
)
