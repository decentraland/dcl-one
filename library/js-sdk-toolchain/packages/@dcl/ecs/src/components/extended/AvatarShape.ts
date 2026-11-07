import { IEngine, LastWriteWinElementSetComponentDefinition } from '../../engine'
import { DeepReadonly } from '../../engine/readonly'
import { ByteBuffer } from '../../serialization/ByteBuffer'
import { AvatarShapeSchema } from '../generated/AvatarShape.gen'
import { PBAvatarShape } from '../generated/pb/decentraland/sdk/components/avatar_shape.gen'

/**
 * @public
 * AvatarShape with `forceRender` optional: the wire type is a repeated field (always present, so the
 * generated type requires it), but a scene rarely needs it and shouldn't have to pass `[]`.
 */
export type AvatarShapeType = Omit<PBAvatarShape, 'forceRender'> & {
  forceRender?: string[] | undefined
}

/**
 * @public
 */
export type AvatarShapeComponentDefinitionExtended = LastWriteWinElementSetComponentDefinition<AvatarShapeType>

export function defineAvatarShapeComponent(
  engine: Pick<IEngine, 'defineComponentFromSchema'>
): AvatarShapeComponentDefinitionExtended {
  const patchedSchema = { ...AvatarShapeSchema }
  const origSerialize = patchedSchema.serialize
  // `??` also covers a `null` written through getMutable
  patchedSchema.serialize = (value: DeepReadonly<AvatarShapeType>, builder: ByteBuffer) => {
    origSerialize({ ...value, forceRender: value.forceRender ?? [] }, builder)
  }

  const theComponent = engine.defineComponentFromSchema('core::AvatarShape', patchedSchema)
  return theComponent as unknown as AvatarShapeComponentDefinitionExtended
}
