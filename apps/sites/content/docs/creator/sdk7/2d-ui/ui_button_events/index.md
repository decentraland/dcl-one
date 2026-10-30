> Handle button events on UI entities.

# UI Button Events

To make a button in your UI, create a `Button` UI element with the following properties:

* `value`: A string with the text to display on the button.
* `onMouseDown`: A callback function that runs every time the user pushes the pointer button on the entity.
* `uiTransform`: Positioning properties of the UI element.

The following example shows how to create a clickable UI button.

_**ui.tsx file:**_

```tsx
import { Button } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on the UI')
		}}
	/>
)
```

_**index.ts file:**_

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 Note**: All the following snippets in this page assume that you have a `.ts` similar to the above, running the `ReactEcsRenderer.setUiRenderer()` function.


You can also write the function that is executed by the click outside the UI definition, and reference it by name. This helps keep the UI code more readable, and is also useful if multiple clickable UI entities need to call the same function.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

function handleClick() {
	// Do something onClick
	console.log('Clicked on the UI')
}
export const uiMenu = () => (
	<Button
		value="Click me"
		uiTransform={{ width: 100 }}
		onMouseDown={handleClick}
	/>
)
```

The following fields can be added to a `Button` UI element:

* `onMouseDown`: A callback function that runs every time the user pushes the pointer button on the entity.
* `onMouseUp`: A callback function that runs every the pointer button is raised while pointing at the entity.
* `onMouseEnter`: A callback function that runs every time the pointer starts hovering over the button.
* `onMouseLeave`: A callback function that runs every time the pointer stops hovering over the button.
* `color`: Color of the text on the button.
* `font`: Font of the text on the button.
* `textAlign`: Alignment of the text inside the button
* `uiTransform`: Positioning properties of the UI element.
* `uiBackground`: Set the color or texture of the UI element.
* `variant`: Use this property to set the style of the button as one of the defaults. `primary` and `secondary` are available.
* `disabled`: Boolean to set a button disabled. When `disabled` is set to _true_, the `onMouseDown` and `onMouseUp` actions are no longer called, and the button stops advertising any pointer interaction. The button is also drawn "grayed-out": both the text and the background are rendered at half their `alpha` value. This is a display change only. The `Color4` values your scene passes in are never modified, so you can safely reuse a shared palette object across many elements.

## Button styling

Set the variant to `primary` or `secondary` to take advantage of the default styling options for buttons. `primary` makes your button red with white text, `secondary` makes your button white with red text.

```tsx
import { UiEntity, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: 500,
			height: 230,
			margin: '16px 0 8px 270px',
			padding: 4,
			alignSelf: 'center',
		}}
		uiBackground={{ color: Color4.Gray() }}
	>
		<Button
			value="Click Me"
			variant="primary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
		<Button
			value="Click Me"
			variant="secondary"
			uiTransform={{ width: 80, height: 20, margin: 4 }}
			onMouseDown={() => {
				console.log('Clicked on the UI')
			}}
		/>
	</UiEntity>
)
```

You're also free to use all of the properties on background freely. You can also set a variant and then override some of its properties. This example uses the `primary` variant, but overrides the color to be green:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<Button
		value="My Button!"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
		}}
		uiBackground={{
			color: Color4.Green(),
		}}
	/>
)
```

## Togglable buttons

A common use case is to make a button toggle between two states, like a switch. The example below switches between two colors each time the button is pressed:

```tsx
import { Button } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		variant="primary"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			console.log('Clicked on My Button!')
			buttonEnabled = !buttonEnabled
			if (buttonEnabled) {
				// do something
			} else {
				// do something else
			}
		}}
		uiBackground={{
			color: buttonEnabled ? Color4.Green() : Color4.Red(),
		}}
	/>
)
```

Note that in the example above, the color depends on a `buttonEnabled` variable. Whenever this variable's value changes, it inmediately affects the background color.

## Hover Feedback

Another common use case is to display some kind of visual hint when hovering over a button, to clarify that this is interactible, or even to display a hover hint explaining what this button does. Use the `onMouseEnter` and `onMouseLeave` callbacks to detect when the player's cursor is on the button, and react accordingly.

```tsx
import { Button } from '@dcl/sdk/react-ecs'

let buttonEnabled = false

export const uiMenu = () => (
	<Button
		value="My Button"
		uiTransform={{ width: 100, height: 100 }}
		onMouseDown={() => {
			// button function
		}}
		onMouseEnter={() => {
			// show hint
		}}
		onMouseLeave={() => {
			// hide hint
		}}
	/>
)
```

## Making other elements clickable

Any element in the UI can be made clickable by adding an `onMouseDown` property to it, it works identically to a button. The following example adds `onMouseDown` properties to background images and text.

```tsx
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
	<UiEntity
		onMouseDown={() => {
			console.log('Background clicked!')
		}}
		uiTransform={{
			width: 400,
			height: 230,
		}}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	>
		<Label
			onMouseDown={() => {
				console.log('Label clicked!')
			}}
			value={`Player: ${getPlayerPosition()}`}
			fontSize={18}
			uiTransform={{ width: '100%', height: 30 }}
		/>
	</UiEntity>
)

function getPlayerPosition() {
	const playerPosition = Transform.getOrNull(engine.PlayerEntity)
	if (!playerPosition) return 'unknown'
	const { x, y, z } = playerPosition.position
	return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

## Pointer blocking

All UI entities are non-pointer blocking by default, meaning that players's clicks will go through them and interact with objects in the 3D world space behind them. If an entity has an `onMouseDown` callback, then it becomes pointer blocking, so player's clicks don't affect what's behind that UI entity.

You can change this default behavior by changing the value of the `pointerFilter` property on the `uiTransform` component on any UI entity. For example to set an entity that has no `onMouseDown` to be pointer blocking.

The supported values for `pointerFilter` are:

* `block`: The UI element is pointer blocking, players can't click on anything behind this UI element.
* `none`: The UI element is non-pointer blocking. The element is not clickable and anything behind it can be clicked.

Below is a simple UI that doesn't have an `onMouseDown`, but that overrides the default behavior of not being pointer-blocking by setting `pointerFilter` to `block`.

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// draw UI
export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			width: '100%',
			height: '100px',
			pointerFilter: `block`,
		}}
		uiText={{ value: `This element is pointer blocking`, fontSize: 40 }}
		uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
	/>
)
```

### Blocking follows the layout box, not the visible pixels

A blocking element captures clicks across its **whole rectangle**, whether or not anything is drawn there. A fully transparent background makes no difference.


> [!CAUTION]
> **Warning:** Never put a pointer handler or `pointerFilter: 'block'` on a full-screen wrapper sized `100%` by `100%`.
> 
> Its rectangle is the entire screen, so a single stray `onMouseDown` on your layout root makes every other UI element and everything in the 3D world unclickable. The UI still looks perfectly correct, because the panel you can see only covers a small part of the screen, which makes this very hard to spot.


Attach handlers to the smallest element that needs them: the panel, the button, the row. Layout wrappers stay handler-free.

There are two cases where a full-screen blocking element is the right thing, and both are deliberate:

* A **modal backdrop**, which is meant to swallow clicks, and which you only render while the modal is open.
* A **drag-release catcher**, which only exists while a drag is in progress. See [Drag interactions](#drag-interactions) below.

If clicking stops working anywhere in your scene, this is the first thing to check.

## Drag interactions

UI pointer handlers (`onMouseDown`, `onMouseUp`, `onMouseEnter`, `onMouseLeave`) take no parameters. They fire as simple `() => void` callbacks with no position or coordinate data. There is no `onMouseDrag` or `onMouseMove` handler in the UI system.

To build drag-based UI (sliders, scrub bars, drag handles), use `PrimaryPointerInfo.screenDelta` from `@dcl/sdk/ecs`. This gives the mouse movement in pixels since the last frame, updated every frame regardless of what the cursor is over.

The pattern works as follows:

1. `onMouseDown` on the drag target starts the drag and records the initial value.
2. A system reads `screenDelta` each frame and accumulates it into the value while the drag is active.
3. A full-screen invisible overlay with `pointerFilter: 'block'` catches the mouse release, so letting go outside the narrow target still ends the drag.

```ts
import { engine, PrimaryPointerInfo, UiCanvasInformation } from '@dcl/sdk/ecs'

let dragging = false
let sliderValue = 0.5

// Call this system every frame to accumulate drag movement
engine.addSystem(() => {
	if (!dragging) return

	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta || delta.x === 0) return

	// Convert screen pixels to a 0–1 range
	// Divide by UI scale factor so drag speed matches the cursor
	const canvas = UiCanvasInformation.getOrNull(engine.RootEntity)
	const scale = canvas ? Math.min(canvas.width / 1920, canvas.height / 1080) : 1
	const trackWidth = 200 // virtual px width of the slider track

	sliderValue = Math.max(0, Math.min(1, sliderValue + delta.x / scale / trackWidth))
})
```


> [!WARNING]
> **Note:** On mobile, `screenDelta` always reports 0 (there is no free-moving cursor). For mobile-compatible sliders, add stepper buttons (`-` / `+`) alongside the drag track. Use [`isMobile()`](/docs/creator/build-for-mobile/develop/detect-platform/) from `@dcl/sdk/platform` to branch your UI.



> [!NOTE]
> **Tip:** Always divide `screenDelta` by the UI scale factor, or the drag will over- or under-shoot on screens whose resolution differs from your virtual size.

