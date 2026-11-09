// Photo mode (the camera reel's in-world camera): the engine owns the camera, the capture and the
// upload; this relays its state to React and turns the HUD's buttons into the engine's console
// commands (/photo_mode, /take_photo), which are also the automation surface.
import { BevyApi } from '../bevy-api'
import { relay } from '../system-helpers'
import type { Ctx } from '../bridge'

export function registerPhotoMode(ctx: Ctx): void {
  if (BevyApi.getPhotoModeStream != null) {
    const open = BevyApi.getPhotoModeStream
    relay('photoMode', async () => await open(), (state) => {
      ctx.send({ kind: 'photoMode', state })
    })
  }

  const run = async (command: string, args: string[]): Promise<void> => {
    if (BevyApi.consoleCommand == null) return
    await BevyApi.consoleCommand(command, args).catch((e: unknown) => {
      console.error(`[photoMode] /${command} ${args.join(' ')}`.trim(), e)
    })
  }
  ctx.on('setPhotoMode', async (msg) => {
    await run('photo_mode', [msg.enable ? 'on' : 'off', msg.source])
  })
  ctx.on('takePhoto', async () => {
    await run('take_photo', ['button'])
  })
}
