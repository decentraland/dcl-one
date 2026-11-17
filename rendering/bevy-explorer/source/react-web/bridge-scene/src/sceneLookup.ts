// When the scene-title lookup asks the engine for the live scenes, and when it publishes the
// answer. No engine call is made here, so a test can drive it poll by poll.

// A scene the player walked or teleported into is not in the live list until it registers, so a
// miss means "not yet" for this many polls. Then the parcel is published as empty.
export const SCENE_LOOKUP_ATTEMPTS = 3
// A parcel published as empty is asked again with a growing gap, so a scene that loads late gets
// its title and a player who stays costs one engine call each 30 s (at the 0.3 s poll). The
// values are a choice: neither liveSceneInfo's cost nor a late scene's delay is measured.
const EMPTY_RECHECK_FIRST_POLLS = 6
const EMPTY_RECHECK_MAX_POLLS = 96

export type SceneLookup = {
  /**
   * One poll with the player on parcel `key`; each call advances the schedule. True when the
   * engine is to be asked now.
   */
  poll: (key: string) => boolean
  /** The engine's answer for `key`: true when it is to be published. */
  settle: (key: string, found: boolean) => boolean
  /** Back to the first state, so the next poll asks again. */
  reset: () => void
}

export function createSceneLookup(): SceneLookup {
  let published = ''
  let empty = false
  let pending = ''
  let attempts = 0
  let gap = EMPTY_RECHECK_FIRST_POLLS
  let idle = 0
  return {
    poll(key) {
      if (key === published) {
        if (!empty || ++idle < gap) return false
        idle = 0
        gap = Math.min(gap * 2, EMPTY_RECHECK_MAX_POLLS)
        return true
      }
      if (key !== pending) {
        pending = key
        attempts = 0
      }
      attempts++
      return true
    },
    settle(key, found) {
      // While it retries, the header keeps the title it has: a recheck that finds nothing
      // publishes nothing, and neither does a miss inside the first budget.
      if (!found && (key === published || attempts < SCENE_LOOKUP_ATTEMPTS)) return false
      published = key
      empty = !found
      idle = 0
      gap = EMPTY_RECHECK_FIRST_POLLS
      return true
    },
    reset() {
      published = ''
      empty = false
      pending = ''
      attempts = 0
      gap = EMPTY_RECHECK_FIRST_POLLS
      idle = 0
    }
  }
}
