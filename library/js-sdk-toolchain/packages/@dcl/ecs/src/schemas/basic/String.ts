import type { ByteBuffer } from '../../serialization/ByteBuffer'
import type { ISchema } from '../ISchema'

/**
 * @internal
 */
export const FlatString: ISchema<string> = {
  serialize(value: string, builder: ByteBuffer): void {
    builder.writeUtf8String(value)
  },
  deserialize(reader: ByteBuffer): string {
    return reader.readUtf8String()
  },
  create() {
    return ''
  },
  jsonSchema: {
    type: 'string',
    serializationType: 'utf8-string'
  }
}

/**
 * @internal
 */
export const EcsString = FlatString
