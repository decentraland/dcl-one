// UI sounds: the page decides when a chat line warrants a ping (the conversation's notification
// ping setting, whether the line mentions the user, a debounce) and names the sound; this scene
// plays the clip from its own content, through a scene audio source with no position (`global`).
//   to: AudioSource on an entity per clip
import { AudioSource, engine, type Entity } from '@dcl/sdk/ecs'
import type { Ctx } from '../bridge'
import type { UiSound } from '@hud/engine/protocol'

const CLIPS: Record<UiSound, string> = {
  chatMessage: 'sounds/chat_message_receive.wav',
  chatMention: 'sounds/chat_mention_receive.wav'
}

export function registerUiSound(ctx: Ctx): void {
  const players = new Map<UiSound, Entity>()
  const player = (sound: UiSound): Entity => {
    let entity = players.get(sound)
    if (entity == null) {
      entity = engine.addEntity()
      AudioSource.create(entity, { audioClipUrl: CLIPS[sound], playing: false, loop: false, global: true })
      players.set(sound, entity)
    }
    return entity
  }
  ctx.on('playUiSound', (msg) => {
    const clip = CLIPS[msg.sound]
    // from the start, even while the previous play of the same clip is still sounding
    if (clip != null) AudioSource.playSound(player(msg.sound), clip, true)
  })
}
