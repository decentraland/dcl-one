import { engine } from '@dcl/sdk/ecs'
import { consumeMessages, publishData, subscribeToTopic } from '~system/CommsApi'

const topic = 'scene-comms-smoke'

async function exchange(sequence: number) {
  await publishData({ topic, data: `ping ${sequence}` })
  const { messages } = await consumeMessages({ topic })
  for (const { sender, data } of messages) {
    console.log(`[scene-comms] received ${data} from ${sender}`)
  }
}

export async function main() {
  await subscribeToTopic({ topic })
  const { messages } = await consumeMessages({ topic })
  console.log(`[scene-comms] subscribed and drained ${messages.length} initial messages`)

  let elapsed = 0
  let sequence = 0
  let exchanging = false
  engine.addSystem((dt) => {
    elapsed += dt
    if (elapsed < 1 || exchanging) return
    elapsed = 0
    exchanging = true
    void exchange(++sequence)
      .catch((error) => console.error('[scene-comms] exchange failed', error))
      .finally(() => {
        exchanging = false
      })
  })
}
