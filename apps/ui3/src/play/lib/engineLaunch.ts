import type { EngineLaunch } from '@explorer-core/hostContract'
import { launchOptionsFromUrl } from '@explorer-core/lib/webParams'
import { trackAudioContexts } from './audioUnlock'
import { engineServiceOptions } from './baseDomain'
import { bootMode } from './bootMode'
import { PAGE_DIR } from './publicUrl'
import { SYSTEM_SCENE } from './systemScene'

export function seedBridgeSession(): void {
  trackAudioContexts()
}

export function engineLaunch(params: URLSearchParams): EngineLaunch {
  return {
    options: {
      ...launchOptionsFromUrl(params, ['autopilot']),
      ...engineServiceOptions(),
      systemScene: bootMode().systemScene ?? SYSTEM_SCENE
    },
    engineDir: new URL('engine', PAGE_DIR).href
  }
}
