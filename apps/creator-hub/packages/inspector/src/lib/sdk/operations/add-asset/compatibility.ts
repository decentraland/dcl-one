import type { IEngine } from '@dcl/ecs';
import type { AssetData } from '@dcl/asset-packs';
import { findUnsupportedParams, parseLayout } from '../../script-params';

export type IncompatibleComponent = {
  name: string;
  reason: 'missing' | 'outdated-definition' | 'unsupported-script-param' | 'sdk-too-old';
};

/** The `@dcl/sdk` an asset needs (`minSdkVersion` in its data.json) and the one the scene has. */
export type SdkRequirement = {
  required?: string;
  /** Null or absent when the host can't tell, as in the standalone inspector. */
  installed?: string | null;
};

export type CompatibilityResult =
  | { compatible: true }
  | { compatible: false; incompatibleComponents: IncompatibleComponent[] };

/**
 * Checks whether all components referenced in an asset's composite.json are available
 * in the current engine with the exact schema version expected.
 *
 * Two failure modes are detected:
 * - 'missing': the component name does not exist in the engine at all.
 * - 'outdated-definition': a versioned variant of the component exists (e.g. `asset-packs::Actions`)
 *   but the exact versioned name from the composite (e.g. `asset-packs::Actions-v1`) is not registered,
 *   meaning the installed SDK schema is behind what the asset requires.
 *
 * Those two look at the editor's own engine. A script can also need components that only the
 * scene's `@dcl/sdk` has to provide, which no composite names; an asset declares that with
 * `minSdkVersion`, checked against `sdk.installed` ('sdk-too-old').
 */
export function checkAssetCompatibility(
  composite: AssetData['composite'],
  engine: IEngine,
  sdk: SdkRequirement = {},
): CompatibilityResult {
  const incompatible: IncompatibleComponent[] = [];

  if (sdk.required && isSdkTooOld(sdk)) {
    incompatible.push({ name: `@dcl/sdk ${sdk.required}`, reason: 'sdk-too-old' });
  }

  for (const component of composite?.components ?? []) {
    const name = component.name;
    try {
      // Attempt exact lookup of the component name as written in composite.json.
      // The versioning framework registers each versioned name individually:
      //   asset-packs::Actions       → schema v0
      //   asset-packs::Actions-v1    → schema v0+v1
      // If the engine doesn't have the exact versioned name the composite expects,
      // getComponent() throws.
      engine.getComponent(name);
    } catch {
      // Distinguish: does any variant (base name) of this component exist?
      const baseName = name.replace(/-v\d+$/, '');
      let baseExists = false;
      try {
        engine.getComponent(baseName);
        baseExists = true;
      } catch {
        // base component doesn't exist at all
      }

      incompatible.push({
        name,
        reason: baseExists ? 'outdated-definition' : 'missing',
      });
    }
  }

  incompatible.push(...findUnsupportedScriptParams(composite));

  return incompatible.length === 0
    ? { compatible: true }
    : { compatible: false, incompatibleComponents: incompatible };
}

export function isSdkTooOld({ required, installed }: SdkRequirement): boolean {
  return !!required && !!installed && isOlderVersion(installed, required);
}

const VERSION = /^v?(\d+)\.(\d+)\.(\d+)/;

/**
 * Compares `major.minor.patch` only. A pre-release of the required version (a PR build such as
 * `7.32.0-123.commit-abc`) counts as that version, and a version that can't be read is never
 * older: an asset is refused only when the scene's SDK is known to be too old.
 */
function isOlderVersion(installed: string, required: string): boolean {
  const a = VERSION.exec(installed);
  const b = VERSION.exec(required);
  if (!a || !b) return false;
  for (let i = 1; i <= 3; i++) {
    const diff = Number(a[i]) - Number(b[i]);
    if (diff !== 0) return diff < 0;
  }
  return false;
}

const SCRIPT_COMPONENT = /^asset-packs::Script(-v\d+)?$/;

/** Script params in the composite whose type this editor can't render. */
function findUnsupportedScriptParams(composite: AssetData['composite']): IncompatibleComponent[] {
  const result: IncompatibleComponent[] = [];
  try {
    for (const component of composite?.components ?? []) {
      if (!SCRIPT_COMPONENT.test(component.name)) continue;
      for (const entry of Object.values(component.data ?? {})) {
        const scripts = entry?.json?.value;
        for (const script of Array.isArray(scripts) ? scripts : []) {
          const layout =
            typeof script?.layout === 'string' ? parseLayout(script.layout) : undefined;
          if (!layout) continue;
          const file = String(script.path ?? '')
            .split('/')
            .pop();
          for (const param of findUnsupportedParams(layout)) {
            result.push({ name: `${file}: ${param}`, reason: 'unsupported-script-param' });
          }
        }
      }
    }
  } catch (error) {
    console.warn('Failed to read the script params of an asset:', error);
    return [];
  }
  return result;
}
