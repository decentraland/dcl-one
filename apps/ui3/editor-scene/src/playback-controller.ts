export interface PlaybackDependencies {
  pin(): Promise<boolean>
  freeze(): Promise<unknown>
  unfreeze(): Promise<unknown>
  visuals(playing: boolean, paused: boolean): Promise<void>
  commit(playing: boolean, paused: boolean): void
}

export function createPlaybackController(deps: PlaybackDependencies) {
  return async (playing: boolean, paused: boolean): Promise<void> => {
    if (paused && !playing) throw new Error('Only a playing scene can be paused')
    if (!await deps.pin()) throw new Error('The project scene could not be selected')
    try {
      if (paused) await deps.freeze()
      else await deps.unfreeze()
    } catch (error) {
      const alreadyApplied = paused ? 'already frozen' : 'not frozen'
      if (!String(error).includes(alreadyApplied)) throw error
    }
    await deps.visuals(playing, paused)
    deps.commit(playing, paused)
  }
}
