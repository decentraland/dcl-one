// UI sounds: the page decides when a chat line warrants a ping (its per-channel setting, mentions,
// whether the line is on screen); the engine plays the clip.
//   to: BevyApi.playUiSound (the engine's embedded sounds/ui clips, by file name)
import { BevyApi } from '../bevy-api'
import type { Ctx } from '../bridge'
import type { UiSound } from '../../../src/engine/protocol'

const CLIPS: Record<UiSound, string> = {
  chatMessage: 'chat_message_receive.wav',
  chatMention: 'chat_mention_receive.wav'
}

export function registerUiSound(ctx: Ctx): void {
  ctx.on('playUiSound', (msg) => {
    const clip = CLIPS[msg.sound]
    if (clip != null) BevyApi.playUiSound(clip)
  })
}
