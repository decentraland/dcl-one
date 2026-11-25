import type { IEngine, LastWriteWinElementSetComponentDefinition } from '../../engine'
import type { DeepReadonly } from '../../engine/readonly'
import type { ByteBuffer } from '../../serialization/ByteBuffer'
import { AvatarEquippedDataSchema } from '../generated/AvatarEquippedData.gen'
import type { PBAvatarEquippedData } from '../generated/pb/decentraland/sdk/components/avatar_equipped_data.gen'

/**
 * @public
 * AvatarEquippedData with `forceRender` optional: the wire type is a repeated field (always present, so the
 * generated type requires it), but a scene rarely needs it and shouldn't have to pass `[]`.
 */
export type AvatarEquippedDataType = Omit<PBAvatarEquippedData, 'forceRender'> & {
  forceRender?: string[] | undefined
}

/**
 * @public
 */
export type AvatarEquippedDataComponentDefinitionExtended =
  LastWriteWinElementSetComponentDefinition<AvatarEquippedDataType>

export function defineAvatarEquippedDataComponent(
  engine: Pick<IEngine, 'defineComponentFromSchema'>
): AvatarEquippedDataComponentDefinitionExtended {
  const patchedSchema = { ...AvatarEquippedDataSchema }
  const origSerialize = patchedSchema.serialize
  // `??` also covers a `null` written through getMutable
  patchedSchema.serialize = (value: DeepReadonly<AvatarEquippedDataType>, builder: ByteBuffer) => {
    // a composite written before `wearables` existed carries none
    origSerialize({ ...value, wearables: value.wearables ?? [], forceRender: value.forceRender ?? [] }, builder)
  }

  const theComponent = engine.defineComponentFromSchema('core::AvatarEquippedData', patchedSchema)
  return theComponent as unknown as AvatarEquippedDataComponentDefinitionExtended
}
