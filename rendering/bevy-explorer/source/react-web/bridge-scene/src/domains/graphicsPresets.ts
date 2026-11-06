// Graphics presets (Low/Medium/High): the per-setting values each preset applies and the
// resolution of those values against the engine's live settings list. Pure data and functions,
// kept free of bridge imports so react-web's tests can type-check and run them directly;
// `settings.ts` wires them into the Graphics Preset setting.
import type { Setting } from '../../../src/engine/protocol'

export const PRESET_NAMES = ['Low', 'Medium', 'High']
export const MEDIUM_INDEX = 1

// Per-setting values for [Low, Medium, High], from Unity's presets:
// unity:Explorer/Assets/DCL/Settings/Settings/QualityPreset{Low,Medium,High}.asset,
// QualityPresetsAsset.asset and unity:Explorer/Assets/DCL/Quality/Settings/QualityPresetData.cs.
// Strings name an enum variant (resolved against the setting's namedVariants), numbers are raw
// slider values, a list is a fallback chain: the first variant the live setting publishes wins
// (MSAA is native only, so web lands on FXAA (High)). Settings missing from the engine's list
// (platform-dependent) are skipped. Loading and population settings are not part of the presets.
type PresetValue = number | string | string[]
const PRESET_VALUES: Record<string, [PresetValue, PresetValue, PresetValue]> = {
  'Target Frame Rate': ['30 fps', '60 fps', '144 fps'],
  'Render Resolution': [50, 100, 100],
  'Anti-aliasing': ['Off', ['MSAA 2x', 'FXAA (High)'], ['MSAA 8x', 'FXAA (High)']],
  Bloom: ['Off', 'High', 'High'], // High is Unity's bloom; Medium and High both enable it
  'Avatar Outline': ['Off', 'On', 'On'],
  'Shadow settings': ['Off', 'Low', 'High'],
  'Shadow Distance': [40, 60, 100],
  'Light Count': [0, 6, 10],
  'Shadow Caster Count': [0, 3, 3],
  'Lens Flare': ['Off', 'On', 'On'],
  'Depth of Field': ['Off', 'Off', 'Off'], // Unity has none
  SSAO: ['Off', 'Off', 'Off'], // Unity has none
  Fog: ['On', 'On', 'On'],
  'Sky Reflections': ['High', 'High', 'High']
}

function resolveValue(setting: Setting, value: PresetValue): number | undefined {
  if (typeof value === 'number') return value
  for (const name of Array.isArray(value) ? value : [value]) {
    const ix = setting.namedVariants.findIndex((v) => v.name === name)
    if (ix >= 0) return ix
  }
  return undefined
}

/** The (name, value) pairs preset `presetIx` would apply, given the live settings list. */
export function presetTargets(settings: Setting[], presetIx: number): Array<{ name: string; value: number }> {
  const targets: Array<{ name: string; value: number }> = []
  for (const [name, values] of Object.entries(PRESET_VALUES)) {
    const setting = settings.find((s) => s.name === name)
    if (!setting) continue
    const value = resolveValue(setting, values[presetIx])
    if (value !== undefined) targets.push({ name, value })
  }
  return targets
}
