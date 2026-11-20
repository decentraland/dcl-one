// The HUD's teleport: to a parcel, a realm or both, optionally onto a named spawn point.
//   from: BevyApi.teleport; RestrictedActions.teleportTo on an engine that lacks it.

export interface TeleportDestination {
  parcel?: { x: number; y: number }
  realm?: string
  spawnPoint?: string
}

/** Every teleport the HUD asks for goes through the engine's teleport for the HUD (`hud`). An
 *  engine without it is asked through the scene API's teleportTo (`scene`), which cannot name a
 *  spawn point: a teleport that names one is refused there, not sent to land somewhere else. */
export function teleport(
  to: TeleportDestination,
  hud: ((to: TeleportDestination) => Promise<unknown>) | undefined,
  scene: (body: { worldCoordinates?: { x: number; y: number }; realm?: string }) => Promise<unknown>
): Promise<unknown> {
  if (hud != null) return hud(to)
  if (to.spawnPoint != null) return Promise.reject(new Error('This engine cannot land on a named spawn point.'))
  return scene({ worldCoordinates: to.parcel, realm: to.realm })
}
