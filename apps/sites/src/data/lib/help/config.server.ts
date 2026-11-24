import { parseStatusPageUrl, parseStatusServices, type Service } from "./status";

/** The Intercom workspace production Sites opens on /help. */
export const DEFAULT_INTERCOM_APP_ID = "z0h94kay";

export type HelpConfig = {
  readonly services: readonly Service[];
  readonly statusPageUrl: string;
  /** Empty: the deployment turned the chat widget off. */
  readonly intercomAppId: string;
};

/**
 * What /help reaches outside this node. Each value has a production default, and a deployment overrides it:
 * HELP_STATUS_SERVICES (JSON list of `{name, url}`), HELP_STATUS_PAGE_URL and INTERCOM_APP_ID (set it empty to
 * turn the chat widget off).
 */
export function helpConfig(env: Record<string, string | undefined> = process.env): HelpConfig {
  return {
    services: parseStatusServices(env.HELP_STATUS_SERVICES),
    statusPageUrl: parseStatusPageUrl(env.HELP_STATUS_PAGE_URL),
    intercomAppId: env.INTERCOM_APP_ID === undefined ? DEFAULT_INTERCOM_APP_ID : env.INTERCOM_APP_ID.trim(),
  };
}
