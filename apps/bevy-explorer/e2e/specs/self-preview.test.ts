// explorer-automation `BackpackWearablePreviewTests` (2, Category SelfPreview): the client is
// launched with `--self-preview-wearables <urns>`, which adds wearables the account does not own to
// the Backpack, and the test equips one and checks its slot thumbnail.
//
// | Unity test                      | Here                                                       | Class |
// | TestL1WearableSlotThumbnail     | the Bevy client has no self-preview launch option          | Gap   |
// | TestL2WearableSlotThumbnail     | the Bevy client has no self-preview launch option          | Gap   |
import { spec } from '../driver/index.ts'

const scenario = spec(import.meta.url)

const REASON = 'the Bevy client has no --self-preview-wearables launch option, so no wearable the account does not own can be put in the Backpack to equip'

scenario('an L1 (Ethereum) self-preview wearable can be equipped and its slot shows a thumbnail', async () => {}, { todo: REASON })

scenario('an L2 (Polygon) self-preview wearable can be equipped and its slot shows a thumbnail', async () => {}, { todo: REASON })
