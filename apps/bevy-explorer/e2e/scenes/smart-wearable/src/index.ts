import { setupUi } from './ui'

// The smart-wearables spec reads this line through `/scene_logs` and the label through
// `/inspect_component <entity> UiText`, so both strings are part of its contract.
export function main(): void {
  console.log('smart wearable fixture started')
  setupUi()
}
