// UI sounds: the page decides when a sound is due (for a chat line: the conversation's
// notification ping setting, whether the line mentions the user, which conversation is selected)
// and names it; this scene plays the clip from its own content, through a scene audio source with
// no position (`global`).
//   to: AudioSource on an entity per clip
import { AudioSource, engine, type Entity } from '@dcl/sdk/ecs'
import type { Ctx } from '../bridge'
import type { UiSound } from '@explorer-core/engine/protocol'

interface Clip {
  url: string
  volume: number
  /** Each play is pitched by a random amount up to this, either way. */
  pitchVariation: number
}

const clip = (file: string, volume = 1, pitchVariation = 0.05): Clip => ({ url: `sounds/${file}`, volume, pitchVariation })

const CLIPS: Record<UiSound, Clip> = {
  chatMessage: clip('chat_message_receive.wav'),
  chatMention: clip('chat_mention_receive.wav'),
  chatUnfocused: clip('chat_unfocused_receive.wav'),
  chatSend: clip('chat_message_send.wav'),
  chatInput: clip('chat_input_text.wav'),
  chatEmoji: clip('chat_swish_in.wav'),
  buttonPress: clip('button_press.wav'),
  buttonHover: clip('button_hover.wav'),
  buttonClose: clip('button_close.wav', 0.7, 0)
}

export function registerUiSound(ctx: Ctx): void {
  const players = new Map<UiSound, Entity>()
  const player = (sound: UiSound, c: Clip): Entity => {
    let entity = players.get(sound)
    if (entity == null) {
      entity = engine.addEntity()
      AudioSource.create(entity, { audioClipUrl: c.url, playing: false, loop: false, global: true, volume: c.volume })
      players.set(sound, entity)
    }
    return entity
  }
  ctx.on('playUiSound', (msg) => {
    const c = CLIPS[msg.sound]
    if (c == null) return
    const entity = player(msg.sound, c)
    AudioSource.getMutable(entity).pitch = 1 + (Math.random() * 2 - 1) * c.pitchVariation
    // from the start, even while the previous play of the same clip is still sounding
    AudioSource.playSound(entity, c.url, true)
  })
}
