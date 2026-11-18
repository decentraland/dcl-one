export const LOBBY_STAGE_PRESETS = [
  { id: 'halloween-2026', name: 'Halloween 2026' },
  { id: 'game-world', name: 'Game World' }
] as const

export type LobbyStagePreset = (typeof LOBBY_STAGE_PRESETS)[number]
export type LobbyStagePresetId = LobbyStagePreset['id']

/** An unknown or missing id falls back to the first look, as the engine does. */
export function lobbyStagePreset(id: string | null | undefined): LobbyStagePreset {
  return LOBBY_STAGE_PRESETS.find((p) => p.id === id) ?? LOBBY_STAGE_PRESETS[0]
}
