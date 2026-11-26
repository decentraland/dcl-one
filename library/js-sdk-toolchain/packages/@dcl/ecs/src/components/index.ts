import type {
  GrowOnlyValueSetComponentDefinition,
  LastWriteWinElementSetComponentDefinition
} from '../engine/component'
import type { IEngine } from '../engine/types'
import type { AnimatorComponentDefinitionExtended } from './extended/Animator'
import { defineAnimatorComponent } from './extended/Animator'
import type { AudioSourceComponentDefinitionExtended } from './extended/AudioSource'
import { defineAudioSourceComponent } from './extended/AudioSource'
import { defineAudioAnalysisComponent } from './extended/AudioAnalysis'
import type { AudioAnalysisView, AudioAnalysisComponentDefinitionExtended } from './extended/AudioAnalysis'
import type { MaterialComponentDefinitionExtended } from './extended/Material'
import { defineMaterialComponent } from './extended/Material'
import type { AvatarEquippedDataComponentDefinitionExtended } from './extended/AvatarEquippedData'
import { defineAvatarEquippedDataComponent } from './extended/AvatarEquippedData'
import type { AvatarShapeComponentDefinitionExtended } from './extended/AvatarShape'
import { defineAvatarShapeComponent } from './extended/AvatarShape'
import type { MeshColliderComponentDefinitionExtended } from './extended/MeshCollider'
import { defineMeshColliderComponent } from './extended/MeshCollider'
import type { MeshRendererComponentDefinitionExtended } from './extended/MeshRenderer'
import { defineMeshRendererComponent } from './extended/MeshRenderer'
import type { TweenComponentDefinitionExtended } from './extended/Tween'
import { defineTweenComponent } from './extended/Tween'
import type { LwwComponentGetter, GSetComponentGetter } from './generated/index.gen'
import type { NameType } from './manual/Name'
import defineNameComponent from './manual/Name'
import type { ISyncComponentsType } from './manual/SyncComponents'
import defineSyncComponent from './manual/SyncComponents'
import type { INetowrkEntityType } from './manual/NetworkEntity'
import defineNetworkEntity from './manual/NetworkEntity'
import type { INetowrkParentType } from './manual/NetworkParent'
import defineNetworkParent from './manual/NetworkParent'
import type { TransformComponentExtended } from './manual/Transform'
import { defineTransformComponent } from './manual/Transform'
import type { AudioStreamComponentDefinitionExtended } from './extended/AudioStream'
import { defineAudioStreamComponent } from './extended/AudioStream'
import { MediaState } from './generated/pb/decentraland/sdk/components/common/media_state.gen'
import type { VirtualCameraComponentDefinitionExtended } from './extended/VirtualCamera'
import { defineVirtualCameraComponent } from './extended/VirtualCamera'
import type { InputModifierComponentDefinitionExtended } from './extended/InputModifier'
import { defineInputModifierComponent } from './extended/InputModifier'
import type { LightSourceComponentDefinitionExtended } from './extended/LightSource'
import { defineLightSourceComponent } from './extended/LightSource'
import type { TriggerAreaComponentDefinitionExtended } from './extended/TriggerArea'
import { defineTriggerAreaComponent } from './extended/TriggerArea'
import type { ParticleSystemComponentDefinitionExtended } from './extended/ParticleSystem'
import { defineParticleSystemComponent } from './extended/ParticleSystem'
import type { TouchScreenControlsComponentDefinitionExtended } from './extended/TouchScreenControls'
import { defineTouchScreenControlsComponent } from './extended/TouchScreenControls'
import type { TagsComponentDefinitionExtended } from './manual/Tags'
import defineTagsComponent from './manual/Tags'
import type { ICreatedByType } from './manual/CreatedBy'
import defineCreatedBy from './manual/CreatedBy'

export * from './generated/index.gen'

export type {
  GrowOnlyValueSetComponentDefinition,
  LastWriteWinElementSetComponentDefinition,
  LwwComponentGetter,
  GSetComponentGetter
}

/* @__PURE__ */
export const Transform: LwwComponentGetter<TransformComponentExtended> = (engine) => defineTransformComponent(engine)

/* @__PURE__ */
export const Material: LwwComponentGetter<MaterialComponentDefinitionExtended> = (engine) =>
  defineMaterialComponent(engine)

/* @__PURE__ */
export const Animator: LwwComponentGetter<AnimatorComponentDefinitionExtended> = (engine) =>
  defineAnimatorComponent(engine)

/* @__PURE__ */
export const AudioSource: LwwComponentGetter<AudioSourceComponentDefinitionExtended> = (engine) =>
  defineAudioSourceComponent(engine)

/* @__PURE__ */
export const AudioAnalysis: LwwComponentGetter<AudioAnalysisComponentDefinitionExtended> = (engine) =>
  defineAudioAnalysisComponent(engine)

/* @__PURE__ */
export const AudioStream: (
  engine: Pick<IEngine, 'defineComponentFromSchema' | 'defineValueSetComponentFromSchema'>
) => AudioStreamComponentDefinitionExtended = (engine) => defineAudioStreamComponent(engine)

/* @__PURE__ */
export const MeshRenderer: LwwComponentGetter<MeshRendererComponentDefinitionExtended> = (engine) =>
  defineMeshRendererComponent(engine)

/* @__PURE__ */
export const MeshCollider: LwwComponentGetter<MeshColliderComponentDefinitionExtended> = (engine) =>
  defineMeshColliderComponent(engine)

/* @__PURE__ */
export const AvatarShape: LwwComponentGetter<AvatarShapeComponentDefinitionExtended> = (engine) =>
  defineAvatarShapeComponent(engine)

/* @__PURE__ */
export const AvatarEquippedData: LwwComponentGetter<AvatarEquippedDataComponentDefinitionExtended> = (engine) =>
  defineAvatarEquippedDataComponent(engine)

/* @__PURE__ */
export const Tween: LwwComponentGetter<TweenComponentDefinitionExtended> = (engine) => defineTweenComponent(engine)

/* @__PURE__ */
export const VirtualCamera: LwwComponentGetter<VirtualCameraComponentDefinitionExtended> = (engine) =>
  defineVirtualCameraComponent(engine)

/* @__PURE__*/
export const InputModifier: LwwComponentGetter<InputModifierComponentDefinitionExtended> = (engine) =>
  defineInputModifierComponent(engine)

/* @__PURE__ */
export const LightSource: LwwComponentGetter<LightSourceComponentDefinitionExtended> = (engine) =>
  defineLightSourceComponent(engine)

/* @__PURE__ */
export const TriggerArea: LwwComponentGetter<TriggerAreaComponentDefinitionExtended> = (engine) =>
  defineTriggerAreaComponent(engine)

/* @__PURE__ */
export const ParticleSystem: LwwComponentGetter<ParticleSystemComponentDefinitionExtended> = (engine) =>
  defineParticleSystemComponent(engine)

/* @__PURE__ */
export const TouchScreenControls: LwwComponentGetter<TouchScreenControlsComponentDefinitionExtended> = (engine) =>
  defineTouchScreenControlsComponent(engine)

/**
 * @alpha
 */
/* @__PURE__ */
export const Name: (engine: Pick<IEngine, 'defineComponent'>) => LastWriteWinElementSetComponentDefinition<NameType> = (
  engine
) => defineNameComponent(engine)

/* @__PURE__ */
export const Tags: (engine: Pick<IEngine, 'defineComponent'>) => TagsComponentDefinitionExtended = (engine) =>
  defineTagsComponent(engine)

/**
 * @alpha
 */
/* @__PURE__ */
export const SyncComponents: (
  engine: Pick<IEngine, 'defineComponent'>
) => LastWriteWinElementSetComponentDefinition<ISyncComponentsType> = (engine) => defineSyncComponent(engine)

/**
 * @alpha
 */
export const NetworkEntity: (
  engine: Pick<IEngine, 'defineComponent'>
) => LastWriteWinElementSetComponentDefinition<INetowrkEntityType> = (engine) => defineNetworkEntity(engine)

/**
 * @alpha
 */
export const NetworkParent: (
  engine: Pick<IEngine, 'defineComponent'>
) => LastWriteWinElementSetComponentDefinition<INetowrkParentType> = (engine) => defineNetworkParent(engine)

/**
 * @public
 */
export const CreatedBy: (
  engine: Pick<IEngine, 'defineComponent'>
) => LastWriteWinElementSetComponentDefinition<ICreatedByType> = (engine) => defineCreatedBy(engine)

export { MediaState }

export type { AudioAnalysisView }
