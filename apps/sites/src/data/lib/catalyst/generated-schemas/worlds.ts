// GENERATED from library/catalyst-api/worlds by sites/scripts/gen-zod-schemas.mts. Do not edit.
import { z } from "zod";

import type { AboutComms } from "@catalyst-api/worlds/AboutComms";
import type { AboutConfigurations } from "@catalyst-api/worlds/AboutConfigurations";
import type { AboutContentStatus } from "@catalyst-api/worlds/AboutContentStatus";
import type { AboutMap } from "@catalyst-api/worlds/AboutMap";
import type { AboutResponse } from "@catalyst-api/worlds/AboutResponse";
import type { AboutServiceStatus } from "@catalyst-api/worlds/AboutServiceStatus";
import type { AllowListPermission } from "@catalyst-api/worlds/AllowListPermission";
import type { CatalogueCounts } from "@catalyst-api/worlds/CatalogueCounts";
import type { CommsAdapterResponse } from "@catalyst-api/worlds/CommsAdapterResponse";
import type { CommsStatus } from "@catalyst-api/worlds/CommsStatus";
import type { ContentStatus } from "@catalyst-api/worlds/ContentStatus";
import type { DeployErrors } from "@catalyst-api/worlds/DeployErrors";
import type { DeployRejection } from "@catalyst-api/worlds/DeployRejection";
import type { DeploySuccess } from "@catalyst-api/worlds/DeploySuccess";
import type { FederationMirrorResponse } from "@catalyst-api/worlds/FederationMirrorResponse";
import type { FederationPeerOmissionView } from "@catalyst-api/worlds/FederationPeerOmissionView";
import type { FederationPeersResponse } from "@catalyst-api/worlds/FederationPeersResponse";
import type { FederationPeerStatusLine } from "@catalyst-api/worlds/FederationPeerStatusLine";
import type { FederationPeerStatusView } from "@catalyst-api/worlds/FederationPeerStatusView";
import type { FederationPeerView } from "@catalyst-api/worlds/FederationPeerView";
import type { FederationRefreshPeerResult } from "@catalyst-api/worlds/FederationRefreshPeerResult";
import type { FederationRefreshResponse } from "@catalyst-api/worlds/FederationRefreshResponse";
import type { IndexResponse } from "@catalyst-api/worlds/IndexResponse";
import type { IndexSceneSummary } from "@catalyst-api/worlds/IndexSceneSummary";
import type { LiveDataPayload } from "@catalyst-api/worlds/LiveDataPayload";
import type { LiveDataResponse } from "@catalyst-api/worlds/LiveDataResponse";
import type { MinimapConfig } from "@catalyst-api/worlds/MinimapConfig";
import type { MirrorFreshness } from "@catalyst-api/worlds/MirrorFreshness";
import type { MirrorStatus } from "@catalyst-api/worlds/MirrorStatus";
import type { PermissionsResponse } from "@catalyst-api/worlds/PermissionsResponse";
import type { PermissionSummaryEntry } from "@catalyst-api/worlds/PermissionSummaryEntry";
import type { PersonalWorldsStatus } from "@catalyst-api/worlds/PersonalWorldsStatus";
import type { RemoteWorldView } from "@catalyst-api/worlds/RemoteWorldView";
import type { SceneListResponse } from "@catalyst-api/worlds/SceneListResponse";
import type { SetMirrorHiddenRequest } from "@catalyst-api/worlds/SetMirrorHiddenRequest";
import type { SetMirrorHiddenResponse } from "@catalyst-api/worlds/SetMirrorHiddenResponse";
import type { SkyboxConfig } from "@catalyst-api/worlds/SkyboxConfig";
import type { StatusResponse } from "@catalyst-api/worlds/StatusResponse";
import type { WorldIndexEntry } from "@catalyst-api/worlds/WorldIndexEntry";
import type { WorldOccupancy } from "@catalyst-api/worlds/WorldOccupancy";
import type { WorldPermissionsBlock } from "@catalyst-api/worlds/WorldPermissionsBlock";
import type { WorldSceneEntry } from "@catalyst-api/worlds/WorldSceneEntry";
import type { WorldsCount } from "@catalyst-api/worlds/WorldsCount";

export const AboutCommsSchema = z.object({
  healthy: z.boolean(),
  protocol: z.string(),
  adapter: z.string(),
});

export const AboutMapSchema = z.object({
  minimapEnabled: z.boolean(),
  sizes: z.array(z.unknown()),
});

export const MinimapConfigSchema = z.object({
  enabled: z.boolean(),
  dataImage: z.string().optional(),
  estateImage: z.string().optional(),
});

export const SkyboxConfigSchema = z.object({
  fixedHour: z.number().nullable(),
  textures: z.array(z.string()),
});

export const AboutConfigurationsSchema = z.object({
  networkId: z.number(),
  globalScenesUrn: z.array(z.string()),
  scenesUrn: z.array(z.string()),
  minimap: MinimapConfigSchema,
  skybox: SkyboxConfigSchema,
  realmName: z.string(),
  map: AboutMapSchema,
});

export const AboutContentStatusSchema = z.object({
  synchronizationStatus: z.string(),
  healthy: z.boolean(),
  publicUrl: z.string(),
});

export const AboutServiceStatusSchema = z.object({
  healthy: z.boolean(),
  publicUrl: z.string(),
});

export const AboutResponseSchema = z.object({
  healthy: z.boolean(),
  acceptingUsers: z.boolean(),
  spawnCoordinates: z.string().nullable(),
  configurations: AboutConfigurationsSchema,
  content: AboutContentStatusSchema,
  lambdas: AboutServiceStatusSchema,
  comms: AboutCommsSchema,
});

export const AllowListPermissionSchema = z.object({
  type: z.string(),
  wallets: z.array(z.string()),
});

export const CatalogueCountsSchema = z.object({
  listed: z.number(),
  mirrored: z.number(),
  pending: z.number(),
  failed: z.number(),
  unlisted: z.number(),
});

export const CommsAdapterResponseSchema = z.object({
  fixedAdapter: z.string(),
});

export const CommsStatusSchema = z.object({
  adapterType: z.string(),
  statusUrl: z.string().optional(),
  rooms: z.number(),
  users: z.number(),
  timestamp: z.number(),
});

export const WorldsCountSchema = z.object({
  ens: z.number(),
  dcl: z.number(),
});

export const ContentStatusSchema = z.object({
  commitHash: z.string(),
  worldsCount: WorldsCountSchema,
});

export const DeployErrorsSchema = z.object({
  errors: z.array(z.string()),
});

export const DeployRejectionSchema = z.object({
  error: z.string(),
  message: z.string(),
});

export const DeploySuccessSchema = z.object({
  creationTimestamp: z.number(),
  message: z.string(),
});

export const FederationPeerStatusViewSchema = z.object({
  lastAttemptAt: z.string().nullable(),
  lastSuccessAt: z.string().nullable(),
  lastError: z.string().nullable(),
  worldsObserved: z.number(),
  entriesSkipped: z.number(),
  truncated: z.boolean(),
  hasEverSucceeded: z.boolean(),
});

export const FederationPeerStatusLineSchema = z.object({
  peerId: z.string(),
  status: FederationPeerStatusViewSchema,
});

export const RemoteWorldViewSchema = z.object({
  peerId: z.string(),
  name: z.string(),
  title: z.string().nullable(),
  description: z.string().nullable(),
  contentRating: z.string().nullable(),
  categories: z.array(z.string()),
  thumbnailHash: z.string().nullable(),
  deployedScenes: z.number(),
  lastDeployedAt: z.string().nullable(),
  observedAt: z.string(),
});

export const FederationMirrorResponseSchema = z.object({
  worlds: z.array(RemoteWorldViewSchema),
  total: z.number(),
  peers: z.array(FederationPeerStatusLineSchema),
});

export const FederationPeerOmissionViewSchema = z.object({
  peerId: z.string(),
  reason: z.string(),
  detail: z.string(),
});

export const FederationPeerViewSchema = z.object({
  peerId: z.string(),
  worldsUrl: z.string(),
  daoProposal: z.string(),
  addedAt: z.string(),
  insecureLoopback: z.boolean(),
  status: FederationPeerStatusViewSchema,
});

export const FederationPeersResponseSchema = z.object({
  configured: z.boolean(),
  peersFile: z.string(),
  peers: z.array(FederationPeerViewSchema),
  omitted: z.array(FederationPeerOmissionViewSchema),
});

export const FederationRefreshPeerResultSchema = z.object({
  peerId: z.string(),
  ok: z.boolean(),
  worldsObserved: z.number(),
  entriesSkipped: z.number(),
  truncated: z.boolean(),
  localNameCollisions: z.array(z.string()).nullable(),
  localNameCollisionsError: z.string().nullable(),
  error: z.string().nullable(),
});

export const FederationRefreshResponseSchema = z.object({
  polled: z.array(FederationRefreshPeerResultSchema),
});

export const IndexSceneSummarySchema = z.object({
  id: z.string(),
  title: z.string().nullable(),
  description: z.string().nullable(),
  thumbnail: z.string().nullable(),
  pointers: z.array(z.string()),
  runtimeVersion: z.string().nullable(),
  timestamp: z.number().nullable(),
});

export const WorldIndexEntrySchema = z.object({
  name: z.string(),
  scenes: z.array(IndexSceneSummarySchema),
});

export const IndexResponseSchema = z.object({
  data: z.array(WorldIndexEntrySchema),
  lastUpdated: z.string(),
});

export const WorldOccupancySchema = z.object({
  worldName: z.string(),
  users: z.number(),
});

export const LiveDataPayloadSchema = z.object({
  totalUsers: z.number(),
  perWorld: z.array(WorldOccupancySchema),
});

export const LiveDataResponseSchema = z.object({
  data: LiveDataPayloadSchema,
  lastUpdated: z.string(),
});

export const MirrorFreshnessSchema = z.object({
  source: z.string(),
  passNumber: z.number(),
  lastCompletedAt: z.string().nullable(),
  ageSeconds: z.number().nullable(),
  lastAttemptAt: z.string().nullable(),
  inProgress: z.boolean(),
  cursor: z.string().nullable(),
  stats: z.record(z.string(), z.unknown()),
  lastError: z.string().nullable(),
});

export const MirrorStatusSchema = z.object({
  enabled: z.boolean(),
  source: z.string().nullable(),
  intervalSeconds: z.number().nullable(),
  catalogue: MirrorFreshnessSchema.nullable(),
  scenes: MirrorFreshnessSchema.nullable(),
  worlds: CatalogueCountsSchema,
});

export const PermissionSummaryEntrySchema = z.object({
  permission: z.string(),
  world_wide: z.boolean(),
  parcel_count: z.number().nullable().optional(),
});

export const WorldPermissionsBlockSchema = z.object({
  deployment: AllowListPermissionSchema,
  streaming: AllowListPermissionSchema,
  access: z.unknown(),
});

export const PermissionsResponseSchema = z.object({
  permissions: WorldPermissionsBlockSchema,
  owner: z.string().nullable(),
  summary: z.record(z.string(), z.array(PermissionSummaryEntrySchema)),
});

export const PersonalWorldsStatusSchema = z.object({
  maxWorlds: z.number(),
  maxSizeBytes: z.number(),
});

export const WorldSceneEntrySchema = z.object({
  worldName: z.string(),
  deployer: z.string(),
  deploymentAuthChain: z.array(z.unknown()),
  entity: z.record(z.string(), z.unknown()),
  entityId: z.string(),
  parcels: z.array(z.string()),
  size: z.string(),
  status: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const SceneListResponseSchema = z.object({
  scenes: z.array(WorldSceneEntrySchema),
  total: z.number(),
});

export const SetMirrorHiddenRequestSchema = z.object({
  hidden: z.boolean(),
});

export const SetMirrorHiddenResponseSchema = z.object({
  peerId: z.string(),
  worldName: z.string(),
  hidden: z.boolean(),
});

export const StatusResponseSchema = z.object({
  content: ContentStatusSchema,
  comms: CommsStatusSchema,
  personalWorlds: PersonalWorldsStatusSchema.nullable(),
  mirror: MirrorStatusSchema,
});

type AssignableTo<Sub, Sup> = Sub extends Sup ? true : false;
type Mutual<A, B> = AssignableTo<A, B> extends true ? AssignableTo<B, A> : false;
type Assert<T extends true> = T;

export type _AssertAboutComms = Assert<Mutual<AboutComms, z.infer<typeof AboutCommsSchema>>>;
export type _AssertAboutConfigurations = Assert<Mutual<AboutConfigurations, z.infer<typeof AboutConfigurationsSchema>>>;
export type _AssertAboutContentStatus = Assert<Mutual<AboutContentStatus, z.infer<typeof AboutContentStatusSchema>>>;
export type _AssertAboutMap = Assert<Mutual<AboutMap, z.infer<typeof AboutMapSchema>>>;
export type _AssertAboutResponse = Assert<Mutual<AboutResponse, z.infer<typeof AboutResponseSchema>>>;
export type _AssertAboutServiceStatus = Assert<Mutual<AboutServiceStatus, z.infer<typeof AboutServiceStatusSchema>>>;
export type _AssertAllowListPermission = Assert<Mutual<AllowListPermission, z.infer<typeof AllowListPermissionSchema>>>;
export type _AssertCatalogueCounts = Assert<Mutual<CatalogueCounts, z.infer<typeof CatalogueCountsSchema>>>;
export type _AssertCommsAdapterResponse = Assert<Mutual<CommsAdapterResponse, z.infer<typeof CommsAdapterResponseSchema>>>;
export type _AssertCommsStatus = Assert<Mutual<CommsStatus, z.infer<typeof CommsStatusSchema>>>;
export type _AssertContentStatus = Assert<Mutual<ContentStatus, z.infer<typeof ContentStatusSchema>>>;
export type _AssertDeployErrors = Assert<Mutual<DeployErrors, z.infer<typeof DeployErrorsSchema>>>;
export type _AssertDeployRejection = Assert<Mutual<DeployRejection, z.infer<typeof DeployRejectionSchema>>>;
export type _AssertDeploySuccess = Assert<Mutual<DeploySuccess, z.infer<typeof DeploySuccessSchema>>>;
export type _AssertFederationMirrorResponse = Assert<Mutual<FederationMirrorResponse, z.infer<typeof FederationMirrorResponseSchema>>>;
export type _AssertFederationPeerOmissionView = Assert<Mutual<FederationPeerOmissionView, z.infer<typeof FederationPeerOmissionViewSchema>>>;
export type _AssertFederationPeersResponse = Assert<Mutual<FederationPeersResponse, z.infer<typeof FederationPeersResponseSchema>>>;
export type _AssertFederationPeerStatusLine = Assert<Mutual<FederationPeerStatusLine, z.infer<typeof FederationPeerStatusLineSchema>>>;
export type _AssertFederationPeerStatusView = Assert<Mutual<FederationPeerStatusView, z.infer<typeof FederationPeerStatusViewSchema>>>;
export type _AssertFederationPeerView = Assert<Mutual<FederationPeerView, z.infer<typeof FederationPeerViewSchema>>>;
export type _AssertFederationRefreshPeerResult = Assert<Mutual<FederationRefreshPeerResult, z.infer<typeof FederationRefreshPeerResultSchema>>>;
export type _AssertFederationRefreshResponse = Assert<Mutual<FederationRefreshResponse, z.infer<typeof FederationRefreshResponseSchema>>>;
export type _AssertIndexResponse = Assert<Mutual<IndexResponse, z.infer<typeof IndexResponseSchema>>>;
export type _AssertIndexSceneSummary = Assert<Mutual<IndexSceneSummary, z.infer<typeof IndexSceneSummarySchema>>>;
export type _AssertLiveDataPayload = Assert<Mutual<LiveDataPayload, z.infer<typeof LiveDataPayloadSchema>>>;
export type _AssertLiveDataResponse = Assert<Mutual<LiveDataResponse, z.infer<typeof LiveDataResponseSchema>>>;
export type _AssertMinimapConfig = Assert<Mutual<MinimapConfig, z.infer<typeof MinimapConfigSchema>>>;
export type _AssertMirrorFreshness = Assert<Mutual<MirrorFreshness, z.infer<typeof MirrorFreshnessSchema>>>;
export type _AssertMirrorStatus = Assert<Mutual<MirrorStatus, z.infer<typeof MirrorStatusSchema>>>;
export type _AssertPermissionsResponse = Assert<Mutual<PermissionsResponse, z.infer<typeof PermissionsResponseSchema>>>;
export type _AssertPermissionSummaryEntry = Assert<Mutual<PermissionSummaryEntry, z.infer<typeof PermissionSummaryEntrySchema>>>;
export type _AssertPersonalWorldsStatus = Assert<Mutual<PersonalWorldsStatus, z.infer<typeof PersonalWorldsStatusSchema>>>;
export type _AssertRemoteWorldView = Assert<Mutual<RemoteWorldView, z.infer<typeof RemoteWorldViewSchema>>>;
export type _AssertSceneListResponse = Assert<Mutual<SceneListResponse, z.infer<typeof SceneListResponseSchema>>>;
export type _AssertSetMirrorHiddenRequest = Assert<Mutual<SetMirrorHiddenRequest, z.infer<typeof SetMirrorHiddenRequestSchema>>>;
export type _AssertSetMirrorHiddenResponse = Assert<Mutual<SetMirrorHiddenResponse, z.infer<typeof SetMirrorHiddenResponseSchema>>>;
export type _AssertSkyboxConfig = Assert<Mutual<SkyboxConfig, z.infer<typeof SkyboxConfigSchema>>>;
export type _AssertStatusResponse = Assert<Mutual<StatusResponse, z.infer<typeof StatusResponseSchema>>>;
export type _AssertWorldIndexEntry = Assert<Mutual<WorldIndexEntry, z.infer<typeof WorldIndexEntrySchema>>>;
export type _AssertWorldOccupancy = Assert<Mutual<WorldOccupancy, z.infer<typeof WorldOccupancySchema>>>;
export type _AssertWorldPermissionsBlock = Assert<Mutual<WorldPermissionsBlock, z.infer<typeof WorldPermissionsBlockSchema>>>;
export type _AssertWorldSceneEntry = Assert<Mutual<WorldSceneEntry, z.infer<typeof WorldSceneEntrySchema>>>;
export type _AssertWorldsCount = Assert<Mutual<WorldsCount, z.infer<typeof WorldsCountSchema>>>;
