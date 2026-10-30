import type { Vector2 } from '@dcl/sdk/math'

export type LiveSceneInfo = {
  hash: string
  base_url?: string
  title: string
  parcels: Vector2[]
  isPortable: boolean
  isBroken: boolean
  isBlocked: boolean
  isSuper: boolean
  sdkVersion: string
}

export type BevyApiInterface = {
  getPreviousLogin: () => Promise<{ userId: string | null }>
  loginPrevious: () => Promise<void>
  loginGuest: () => void

  liveSceneInfo: () => Promise<LiveSceneInfo[]>

  consoleCommand: (cmd: string, args?: string[]) => Promise<string>


  getSystemActionStream: () => Promise<
    AsyncIterable<{ action: string; pressed: boolean }>
  >
}
