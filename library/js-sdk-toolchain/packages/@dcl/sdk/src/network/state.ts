import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer'
import {
  CrdtMessageHeader,
  CrdtMessageProtocol,
  CrdtMessageType,
  IEngine,
  PutComponentOperation,
  PutNetworkComponentOperation,
  SyncComponents as _SyncComponents,
  NetworkEntity as _NetworkEntity,
  NetworkParent as _NetworkParent,
  INetowrkEntity,
  VideoEvent,
  AudioEvent,
  EngineInfo,
  GltfContainerLoadingState,
  PhysicsCombinedForce,
  PhysicsCombinedImpulse,
  PointerEventsResult,
  RaycastResult,
  RealmInfo,
  TweenState,
  UiDropdown,
  UiDropdownResult,
  UiInput,
  UiInputResult,
  UiText,
  UiTransform,
  TriggerAreaResult,
  ComponentDefinition
} from '@dcl/ecs'
import { LIVEKIT_MAX_SIZE } from './server'

export const NOT_SYNC_COMPONENTS: ComponentDefinition<unknown>[] = [
  VideoEvent,
  TweenState,
  AudioEvent,
  EngineInfo,
  GltfContainerLoadingState,
  PhysicsCombinedForce,
  PhysicsCombinedImpulse,
  PointerEventsResult,
  RaycastResult,
  RealmInfo,
  UiDropdown,
  UiDropdownResult,
  UiInput,
  UiInputResult,
  UiTransform,
  UiText,
  TriggerAreaResult
]

export const NOT_SYNC_COMPONENTS_IDS = NOT_SYNC_COMPONENTS.map(($) => $.componentId)
export const NOT_SYNC_COMPONENTS_NAMES: string[] = [
  'smart-items::Script', // BaseComponentNames from ../smart-items/constants.ts
  'smart-items::ActionTypes',
  'asset-packs::Script', // the same components under the legacy prefix (../smart-items/legacy.ts)
  'asset-packs::ActionTypes'
]

export function shouldSyncComponent(component: ComponentDefinition<unknown>): boolean {
  return !(
    NOT_SYNC_COMPONENTS_IDS.includes(component.componentId) ||
    NOT_SYNC_COMPONENTS_NAMES.includes(component.componentName)
  )
}

export function getDesyncedComponents(engine: IEngine): ComponentDefinition<unknown>[] {
  return [...NOT_SYNC_COMPONENTS, ...NOT_SYNC_COMPONENTS_NAMES.map(($) => engine.getComponentOrNull($))].filter(
    Boolean
  ) as ComponentDefinition<unknown>[]
}

/**
 * The state a joining peer receives: for every networked entity, the components its
 * SyncComponents lists plus the three that describe the network entity itself, the same
 * selection the sync transport's filter makes for live changes. Components the scene
 * never marked for sync stay where they are.
 */
export function engineToCrdt(engine: IEngine): Uint8Array[] {
  const crdtBuffer = new ReadWriteByteBuffer()
  const networkBuffer = new ReadWriteByteBuffer()
  const NetworkEntity = engine.getComponent(_NetworkEntity.componentId) as INetowrkEntity
  const SyncComponents = engine.getComponent(_SyncComponents.componentId) as typeof _SyncComponents
  const networkDescription = [NetworkEntity.componentId, SyncComponents.componentId, _NetworkParent.componentId]
  const chunks: Uint8Array[] = []

  for (const itComponentDefinition of engine.componentsIter()) {
    if (!shouldSyncComponent(itComponentDefinition)) {
      continue
    }

    const describesNetworkEntity = networkDescription.includes(itComponentDefinition.componentId)
    itComponentDefinition.dumpCrdtStateToBuffer(crdtBuffer, (entity) => {
      if (!NetworkEntity.has(entity)) return false
      const sync = SyncComponents.getOrNull(entity)
      if (!sync) return false
      return describesNetworkEntity || sync.componentIds.includes(itComponentDefinition.componentId)
    })
  }

  let header: CrdtMessageHeader | null
  while ((header = CrdtMessageProtocol.getHeader(crdtBuffer))) {
    if (header.type === CrdtMessageType.PUT_COMPONENT) {
      const message = PutComponentOperation.read(crdtBuffer)!
      const networkEntity = NetworkEntity.getOrNull(message.entityId)

      // Check if adding this message would exceed the size limit
      const currentBufferSize = networkBuffer.toBinary().byteLength
      const messageSize = message.data.byteLength

      if ((currentBufferSize + messageSize) / 1024 > LIVEKIT_MAX_SIZE) {
        // If the current buffer has content, save it as a chunk
        if (currentBufferSize > 0) {
          chunks.push(networkBuffer.toCopiedBinary())
          networkBuffer.resetBuffer()
        }

        // If the message itself is larger than the limit, we need to handle it specially
        if (messageSize / 1024 > LIVEKIT_MAX_SIZE) {
          console.error(
            `Message too large (${messageSize} bytes), skipping component ${message.componentId} for entity ${message.entityId}`
          )
          continue
        }
      }

      if (networkEntity) {
        PutNetworkComponentOperation.write(
          networkEntity.entityId,
          message.timestamp,
          message.componentId,
          networkEntity.networkId,
          message.data,
          networkBuffer
        )
      }
    } else {
      crdtBuffer.incrementReadOffset(header.length)
    }
  }

  // Add any remaining data as the final chunk
  if (networkBuffer.currentWriteOffset() > 0) {
    chunks.push(networkBuffer.toBinary())
  }

  return chunks
}
