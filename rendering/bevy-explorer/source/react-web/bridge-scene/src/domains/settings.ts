// Settings: the explorer settings list + changing one.
//   from: BevyApi.getSettings() / setSetting().
//
// Also injects a synthetic "Graphics Preset" setting (Low/Medium/High/Custom) at the head
// of the list. Presets are defined here, scene-side — the engine only knows the individual
// settings, and its defaults match the Medium preset. Setting the preset fans out to the
// individual setSetting calls; the preset's own value is derived by matching the live
// values against each preset (Custom when none match).
import { BevyApi } from '../bevy-api'
import type { Setting } from '../../../src/engine/protocol'
import type { Ctx } from '../bridge'

const PRESET_SETTING = 'Graphics Preset'
const PRESET_NAMES = ['Low', 'Medium', 'High']
const MEDIUM_INDEX = 1
const CUSTOM_INDEX = PRESET_NAMES.length

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
  Bloom: ['Off', 'Off', 'High'], // Medium's bloom is on without HDR, so invisible
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

/** Which preset the current values match exactly, or CUSTOM_INDEX if none. */
function currentPreset(settings: Setting[]): number {
  for (let ix = 0; ix < PRESET_NAMES.length; ix++) {
    const match = presetTargets(settings, ix).every(
      (t) => settings.find((s) => s.name === t.name)?.value === t.value
    )
    if (match) return ix
  }
  return CUSTOM_INDEX
}

function withPreset(settings: Setting[]): Setting[] {
  const preset: Setting = {
    name: PRESET_SETTING,
    category: 'Graphics',
    description:
      'Overall graphics quality. Picking a preset applies a bundle of the settings below; changing one of those settings individually shows here as Custom.',
    minValue: 0,
    maxValue: PRESET_NAMES.length + 1,
    namedVariants: [
      ...PRESET_NAMES.map((name) => ({ name, description: `Apply the ${name} preset.` })),
      { name: 'Custom', description: 'Individually adjusted settings.' }
    ],
    value: currentPreset(settings),
    default: MEDIUM_INDEX, // Unity's saved default is Medium (SavedQualitySettingsApplier.ReadSavedPreset)
    stepSize: 1
  }
  return [preset, ...settings]
}

export function registerSettings(ctx: Ctx): void {
  const pushSettings = (): void => {
    BevyApi.getSettings()
      .then((settings) => {
        ctx.send({ kind: 'settings', settings: withPreset(settings) })
      })
      .catch((e: unknown) => {
        console.error('[settings] getSettings failed', e)
      })
  }

  ctx.on('getSettings', () => {
    pushSettings()
  })
  ctx.on('setSetting', (msg) => {
    const apply =
      msg.name === PRESET_SETTING
        ? msg.value < PRESET_NAMES.length
          ? BevyApi.getSettings().then(async (settings) => {
              await Promise.all(
                presetTargets(settings, msg.value).map(async (t) => {
                  await BevyApi.setSetting(t.name, t.value)
                })
              )
            })
          : Promise.resolve() // Custom is a derived state, nothing to apply
        : BevyApi.setSetting(msg.name, msg.value)
    Promise.resolve(apply)
      .then(pushSettings)
      .catch((e: unknown) => {
        console.error('[settings] setSetting failed', e)
      })
  })

  pushSettings() // initial snapshot
}
