import { getSceneClient } from '../../../rpc/scene';
import { isSdkTooOld, type SdkRequirement } from './compatibility';

/**
 * Pairs the `@dcl/sdk` an asset needs with the one the scene has, asking the host for it. An
 * asset with no `minSdkVersion` costs no request. With no host (the standalone inspector), or one
 * that can't answer, `installed` stays unknown and the asset is let through.
 */
export async function getSdkRequirement(
  minSdkVersion: string | undefined,
): Promise<SdkRequirement> {
  if (!minSdkVersion) return {};
  const client = getSceneClient();
  if (!client) return { required: minSdkVersion };
  try {
    const { version } = await client.getSdkVersion();
    return { required: minSdkVersion, installed: version };
  } catch (error) {
    console.warn('Failed to read the SDK version of the scene:', error);
    return { required: minSdkVersion };
  }
}

/** Whether the host can update the scene's SDK from here. */
export function canUpdateSceneSdk(): boolean {
  return !!getSceneClient();
}

/**
 * Updates the scene's SDK through the host. True only when the scene then has an SDK that
 * satisfies `minSdkVersion`: the host reports success for an update that changed nothing.
 */
export async function updateSceneSdk(minSdkVersion: string): Promise<boolean> {
  const client = getSceneClient();
  if (!client) return false;
  try {
    const { ok } = await client.updateSdk();
    if (!ok) return false;
    const requirement = await getSdkRequirement(minSdkVersion);
    return !!requirement.installed && !isSdkTooOld(requirement);
  } catch (error) {
    console.warn('Failed to update the SDK of the scene:', error);
    return false;
  }
}
