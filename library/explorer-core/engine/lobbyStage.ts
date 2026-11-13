// The lobby stage's looks. The engine builds the 3D stage for a look's id (the bridge names it); the
// HUD lists them in Settings and shows the chosen one's backdrop as the stage's stand-in.

export const LOBBY_STAGE_PRESETS = [
  { id: 'halloween-2026', name: 'Halloween 2026' },
  { id: 'game-world', name: 'Game World' }
] as const

export type LobbyStagePreset = (typeof LOBBY_STAGE_PRESETS)[number]
export type LobbyStagePresetId = LobbyStagePreset['id']

/** The look with this id; an unknown or missing id is the first look, as the engine reads it. */
export function lobbyStagePreset(id: string | null | undefined): LobbyStagePreset {
  return LOBBY_STAGE_PRESETS.find((p) => p.id === id) ?? LOBBY_STAGE_PRESETS[0]
}
