// Graphics presets (Low/Medium/High): the value each preset gives each setting, and how a preset
// matches the engine's live settings list. Pure data and functions with no bridge imports, so the
// HUD's tests can run them; `settings.ts` wires them into the Graphics Preset setting.
//
// The engine's defaults (`GraphicsSettings::default`) are Medium, and a config saved before them
// migrates to Medium, so Medium here and those defaults change together.
import type { Setting } from '@explorer-core/engine/protocol'

export const PRESET_NAMES = ['Low', 'Medium', 'High']

// A string names an enum variant (resolved against the setting's namedVariants), a number is a raw
// slider value, and a list is a fallback chain: the first variant the setting publishes wins (MSAA
// is native only, so the web build takes FXAA (High)). A setting the engine does not publish
// (platform-dependent) is skipped. Load distance, avatars, videos and props are left to the player.
type PresetValue = number | string | string[]
const PRESET_VALUES: Record<string, [PresetValue, PresetValue, PresetValue]> = {
  'Target Frame Rate': ['30 fps', '60 fps', '144 fps'],
  VSync: ['Off', 'On', 'On'],
  'Render Resolution': [50, 100, 100],
  'Anti-aliasing': ['Off', ['MSAA 2x', 'FXAA (High)'], ['MSAA 8x', 'FXAA (High)']],
  Bloom: ['Off', 'High', 'High'],
  'Avatar Outline': ['Off', 'On', 'On'],
  'Jiggle Bones': ['Off', 'On', 'On'],
  'Shadow settings': ['Off', 'Low', 'High'],
  'Shadow Distance': [40, 60, 100],
  'Light Count': [0, 6, 10],
  'Shadow Caster Count': [0, 3, 3],
  'Lens Flare': ['Off', 'On', 'On'],
  'Depth of Field': ['Off', 'Off', 'Off'],
  SSAO: ['Off', 'Off', 'Off'],
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

/** Which preset the current values match exactly, or `PRESET_NAMES.length` (Custom) if none. */
export function currentPreset(settings: Setting[]): number {
  for (let ix = 0; ix < PRESET_NAMES.length; ix++) {
    const match = presetTargets(settings, ix).every((t) => settings.find((s) => s.name === t.name)?.value === t.value)
    if (match) return ix
  }
  return PRESET_NAMES.length
}
