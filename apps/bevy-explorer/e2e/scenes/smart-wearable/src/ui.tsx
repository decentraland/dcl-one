import ReactEcs, { ReactEcsRenderer, UiEntity } from '@dcl/sdk/react-ecs'

export const LABEL = 'Smart wearable running'

function FixtureLabel() {
  return (
    <UiEntity
      uiTransform={{ width: 360, height: 48, positionType: 'absolute', position: { top: 16, left: 16 } }}
      uiBackground={{ color: { r: 0, g: 0, b: 0, a: 0.6 } }}
      uiText={{ value: LABEL, fontSize: 20, color: { r: 1, g: 1, b: 1, a: 1 } }}
    />
  )
}

export function setupUi(): void {
  ReactEcsRenderer.setUiRenderer(FixtureLabel)
}
