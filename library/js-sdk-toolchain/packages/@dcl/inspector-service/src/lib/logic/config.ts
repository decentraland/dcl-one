import { version } from '@dcl/smart-items/package.json';

/** What the Node side of the editor reads from its host; the browser UI keeps its own config. */
export type InspectorConfig = {
  /** The builder content server the catalog and its assets are fetched from. */
  contentUrl: string;
  /** With `projectId`, recorded as the `source` of the scenes this editor writes. */
  segmentAppId: string | null;
  projectId: string | null;
};

export type GlobalWithConfig = typeof globalThis & {
  InspectorConfig?: Partial<InspectorConfig>;
};

export const CONTENT_URL = version.includes('commit')
  ? 'https://builder-items.decentraland.zone'
  : 'https://builder-items.decentraland.org';

export function getConfig(): InspectorConfig {
  const config = (globalThis as GlobalWithConfig).InspectorConfig;
  return {
    contentUrl: config?.contentUrl || CONTENT_URL,
    segmentAppId: config?.segmentAppId || null,
    projectId: config?.projectId || null,
  };
}
