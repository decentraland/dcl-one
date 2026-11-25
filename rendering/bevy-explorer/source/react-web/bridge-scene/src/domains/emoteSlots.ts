// The emote wheel's 10 slots: which emotes a profile's wheel shows, and which an equip edits.
// Pure, so the bridge's tests can check them without the scene runtime.

const SLOT_COUNT = 10 // the emote wheel has 10 slots
export const BASE_EMOTE_PREFIX = 'urn:decentraland:off-chain:base-emotes:'
export const BASE_EMOTES = ['handsair', 'wave', 'fistpump', 'dance', 'raisehand', 'clap', 'money', 'kiss', 'headexplode', 'shrug'].map(
  (n) => `${BASE_EMOTE_PREFIX}${n}`
)

// The 10 wheel slots to show for a profile: its equipped emotes positionally, or — when the wheel is
// entirely empty (a fresh profile) — the base emotes in order (slot i = BASE_EMOTES[i]). The bevy
// runtime doesn't seed the defaults into getPlayer().emotes (bevy-ui-scene hits the same empty array),
// so the HUD fills them here, mirroring Unity's SelfProfile empty-wheel fill: it's all-or-nothing, so
// once any slot is equipped the remaining empties stay empty.
export function equippedSlots(emotes: readonly unknown[] | undefined): string[] {
  const slots = Array.from({ length: SLOT_COUNT }, (_, i) => String((emotes ?? [])[i] ?? ''))
  return slots.every((u) => u === '') ? [...BASE_EMOTES] : slots
}

// The 10 slots of the look the Backpack shows (`emotes`: the draft, else the live player), given
// the live profile's wheel (`liveEmotes`, getPlayer().emotes). An entirely empty look takes the
// defaults (`equippedSlots`) only while the live wheel is empty too: a fresh profile, or one whose
// emptied wheel was already deployed. A look the player emptied over a live wheel that still has an
// emote stays empty, so the next equip fills only its own slot rather than refilling the other 9
// with the defaults beside it (an emote in two slots).
export function lookSlots(emotes: readonly unknown[] | undefined, liveEmotes: readonly unknown[] | undefined): string[] {
  const slots = Array.from({ length: SLOT_COUNT }, (_, i) => String((emotes ?? [])[i] ?? ''))
  const liveEquipped = (liveEmotes ?? []).some((urn) => String(urn ?? '') !== '')
  return liveEquipped ? slots : equippedSlots(slots)
}
