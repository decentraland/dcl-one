// Graphics presets (Low/Medium/High): the value each preset gives each setting, and how a preset
// matches the engine's live settings list. Pure data and functions with no bridge imports, so the
// HUD's tests can run them; `settings.ts` wires them into the Graphics Preset setting.
import type { Setting } from '@explorer-core/engine/protocol'

export const PRESET_NAMES = ['Low', 'Medium', 'High']

// Medium is the engine's own default for the setting (`GraphicsSettings::default`, published per
// platform as `Setting.default`), so the two never drift apart.
const DEFAULT = Symbol('engine default')

// A string names an enum variant (resolved against the setting's namedVariants), a number is a raw
// slider value. A setting the engine does not publish (platform-dependent) is skipped. Load
// distance, avatars, videos and props are left to the player.
type PresetValue = number | string | typeof DEFAULT
const PRESET_VALUES: Record<string, [PresetValue, PresetValue, PresetValue]> = {
  'Target Frame Rate': ['30 fps', DEFAULT, '144 fps'],
  VSync: ['Off', DEFAULT, 'On'],
  'Render Resolution': [50, DEFAULT, 100],
  'Anti-aliasing': ['Off', DEFAULT, 'FXAA (High)'],
  Bloom: [DEFAULT, DEFAULT, DEFAULT],
  'Avatar Outline': ['Off', DEFAULT, 'On'],
  // the engine keeps it off by default, so a new install reads as Custom until a preset is picked
  'Jiggle Bones': ['Off', 'On', 'On'],
  'Shadow settings': ['Off', DEFAULT, 'High'],
  'Shadow Distance': [40, DEFAULT, 100],
  'Light Count': [0, DEFAULT, 10],
  'Shadow Caster Count': [0, DEFAULT, 3],
  'Lens Flare': ['Off', DEFAULT, 'On'],
  'Depth of Field': ['High', DEFAULT, 'High'],
  SSAO: ['Off', DEFAULT, 'Off'],
  Fog: ['On', DEFAULT, 'On'],
  'Sky Reflections': ['High', DEFAULT, 'High'],
  // scene loading: one profile per preset; the Performance tab's Scene LODs sliders tune it further
  'Distant Scene Rendering': ['Normal', DEFAULT, 'Ultra'],
  'Live Scene Ring': [1, DEFAULT, 1],
  'Near LOD Ring': [2, DEFAULT, 4],
  'Max Near LODs': [6, DEFAULT, 20],
  'Far LOD Ring': [12, DEFAULT, 30],
  'Max Far LODs': [100, DEFAULT, 400]
}

function resolveValue(setting: Setting, value: PresetValue): number | undefined {
  if (value === DEFAULT) return setting.default
  if (typeof value === 'number') return value
  const ix = setting.namedVariants.findIndex((v) => v.name === value)
  return ix < 0 ? undefined : ix
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
