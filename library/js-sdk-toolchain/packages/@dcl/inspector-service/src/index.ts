// The service entry: what @dcl/sdk-commands loads in Node. Keep it free of React, the DOM and
// any renderer.
export * from './lib/data-layer/remote-data-layer';
export * from './lib/logic/storage';
export * from './lib/data-layer/host/utils/engine-to-composite';
export * from './lib/data-layer/host/utils/engine';
export * from './lib/data-layer/types';
export * from './lib/sdk/components';

export {
  DIRECTORY,
  EXTENSIONS,
  getParentPath,
  transformBase64ResourceToBinary,
  transformBinaryToBase64Resource,
  withAssetDir,
} from './lib/data-layer/host/fs-utils';
export {
  fromSceneSpawnPoint,
  getValidParcels,
  isSceneSpawnAxis,
  toSceneSpawnPoint,
} from './lib/data-layer/host/utils/component';
export type { SceneJsonSpawnPoint } from './lib/data-layer/host/utils/component';
export { DEFAULT_UNDO_MAX_SIZE } from './lib/data-layer/host/utils/undo-redo-array';
export {
  removeAssetFiles,
  removeEmptiedFolders,
  removeEmptyFolders,
  waitForLegacyUndoCommit,
} from './lib/data-layer/remove-assets';
export type { RemovalResult } from './lib/data-layer/remove-assets';
export {
  ANIMATED_PREVIEW_PATH,
  fetchLatestCatalog,
  getAnimatedPreviewUrl,
  getAssetById,
  getAssetByModel,
  getAssetsByCategory,
  getBuiltInEmote,
  getCatalogSource,
  getContentsUrl,
  isFileOnly,
  isGround,
  isSmart,
} from './lib/logic/catalog';
export type { Asset, AssetData, AssetPack, CustomAsset } from './lib/logic/catalog';
export { getConfig } from './lib/logic/config';
export type { InspectorConfig } from './lib/logic/config';
export { consumeAllMessagesInto } from './lib/logic/consume-stream';
export { formatAddress, isAddress, normalizeAddress } from './lib/logic/ethereum';
export { createFileSystemInterface } from './lib/logic/file-system-interface';
export { getDefaultInspectorPreferences } from './lib/logic/preferences/types';
export type { InspectorPreferences } from './lib/logic/preferences/types';
export type { ConfigComponentType } from './lib/sdk/components/Config';
export type { InspectorUIStateType } from './lib/sdk/components/InspectorUIState';
export { ParticleSystemSchema } from './lib/sdk/components/ParticleSystem';
export type {
  ParticleSystemBurst,
  ParticleSystemComponentType,
  ParticleSystemShape,
  ParticleSystemTexture,
} from './lib/sdk/components/ParticleSystem';
export { TransitionMode } from './lib/sdk/components/SceneMetadata';
export type { TransformConfig } from './lib/sdk/components/TransformConfig';
export { resolveActiveSceneComponent } from './lib/sdk/components/scene-metadata-version';
export { AllComponents } from './lib/sdk/components/types';
export { VERSIONS_REGISTRY } from './lib/sdk/components/versioning/registry';
export { createEnumEntityId } from './lib/sdk/enum-entity';
export type { EnumEntity } from './lib/sdk/enum-entity';
export { renameLegacyComponentName } from './lib/sdk/legacy';
export {
  getAncestors,
  getNodes,
  getParent,
  getRoot,
  insertChildAfterInNodes,
  isAncestor,
  mapNodes,
  pushChild,
  pushChildToNodes,
  removeChild,
  removeNode,
} from './lib/sdk/nodes';
export { getScriptTemplateClass } from './lib/sdk/script-templates';
export {
  CAMERA,
  EDITOR_ENTITIES,
  PLAYER,
  ROOT,
  findParent,
  getEmptyTree,
  getTreeFromEngine,
} from './lib/sdk/tree';
export { allEqualTo, cleanPush, intersection, move, partitionByFrequency } from './lib/utils/array';
export { GizmoType } from './lib/utils/gizmo';
export { getGltfResourceUris, parseGltfJson } from './lib/utils/gltf';
export { inBounds } from './lib/utils/layout';
export type { Coords, Layout } from './lib/utils/layout';
export {
  getRelativeResourcePath,
  getResourcesBasePath,
  joinPath,
  normalizePath,
} from './lib/utils/path-utils';
export { PARCEL_SIZE } from './lib/utils/scene';
export { capitalize, transformPropertyToLabel } from './lib/utils/strings';
export type { Asset as DataLayerAsset } from './lib/data-layer/proto/gen/data-layer.gen';
export type { AssetData as DataLayerAssetData } from './lib/data-layer/proto/gen/data-layer.gen';
