> Special entitiy types for the UI, including dropdowns and input boxes.

# UI Special Types

There are certain special entity types that allow for some special kinds of interactions.

## Dropdown

Create a `Dropdown` entity to allow users to expand and select an item from a list.

A `Dropdown` entity must have at least the following properties:

* `options`: What values to display when the dropdown is expanded. Provide an array with a string value for each option. The first value in the array is displayed as the default option.
* `onChange`: A function that is executed every time that a value is selected in the dropdown, using

You can also configure other comopnents of the `Dropdown` entity, like a `uiTransform`, as in other UI entities.

_**ui.tsx file:**_

```tsx
import { UiEntity, Label, Dropdown, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { Color4Type } from '@dcl/sdk/ecs'

function selectOption(index: number) {
  switch (index) {
    case 0:
      textColor = Color4.Red()
      break
    case 1:
      textColor = Color4.Blue()
      break
    case 2:
      textColor = Color4.Green()
      break
  }
}

let textColor: Color4Type = Color4.Red()

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '200px',
        height: '100px',
        alignContent: 'auto',
        flexDirection: 'column',
        alignSelf: 'center',
      }}
    >
      <Label
        value="Select a color"
        fontSize={18}
        color={textColor}
        uiTransform={{
          width: '140px',
          height: '40px',
        }}
      />
      <Dropdown
        options={[`Red`, `Blue`, `Green`]}
        onChange={selectOption}
        uiTransform={{
          width: '100px',
          height: '40px',
        }}
      />
    </UiEntity>
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


## Input text

Create an `Input` entity to allow users to to type in text. Players must first click on this box before they can write into it.

The behavior of the `Input` entity is managed via the following properties:

* `onSubmit`: A function that is executed when the player hits Return/Enter, using the provided text as input. The text field is **not** automatically cleared when this happens, you need to clear it yourself (see the example below).
* `onChange`: A function that is executed every time that a value in the input text is changed. As the player types into the box, this function is executed once for each character that is added or removed.
* `disabled`: If _true_, the player won't be able to interact with the input entity.

The following example uses `onSubmit` to log the provided text to the console.

```tsx
import { UiEntity, Input, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'


export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 400,
      height: 300,
      positionType: 'absolute',
      position: {
        left: '35%',
        top: '40%',
      },
    }}
    uiBackground={{
      color: Color4.Gray(),
    }}
  >
    <Input
      onSubmit={(value) => {
        console.log('submitted value: ' + value)
      }}
      fontSize={35}
      placeholder={'type something'}
      placeholderColor={Color4.Black()}
      uiTransform={{
        width: '400px',
        height: '80px',
      }}
    ></Input>
  </UiEntity>
)
```

It's a good practice to provide both a button for submitting and handling the `onSubmit` event when the player presses the Enter/Return key. The following example shows how you can do this.


> [!WARNING]
> **📔 Note**: Do **not** bind the `value` property of the `Input` directly to what the player types via `onChange`. This creates a round-trip between the scene and the Explorer that causes characters to be dropped when typing fast. Instead, keep `value` as an empty string and only change it when you want to clear the input field.


To clear the input field, set `value` to `' '` (a single space). The Explorer treats this as empty and re-activates the placeholder text. Since the UI function runs every frame, use a flag to apply the clear for just one frame.

```tsx
import { UiEntity, Input, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let inputText = ''
let clearInput = false

export const uiMenu = () => {
  const inputValue = clearInput ? ' ' : ''
  if (clearInput) clearInput = false

  return (
    <UiEntity
      uiTransform={{
        width: 400,
        height: 300,
        positionType: 'absolute',
        position: {
          left: '35%',
          top: '40%',
        },
        flexDirection: 'column',
      }}
      uiBackground={{
        color: Color4.Gray(),
      }}
    >
      <Input
        onSubmit={() => {
          console.log('submitted value: ' + inputText)
          inputText = ''
          clearInput = true
        }}
        fontSize={35}
        placeholder={'type something'}
        placeholderColor={Color4.Black()}
        value={inputValue}
        onChange={(value) => { inputText = value }}
        uiTransform={{
          height: '80px',
          margin: '15px',
        }}
      />
      <Button
        value="Submit text"
        variant="primary"
        uiTransform={{ alignSelf: 'center', padding: '25px' }}
        onMouseDown={() => {
          console.log('submitted value: ' + inputText)
          inputText = ''
          clearInput = true
        }}
      />
    </UiEntity>
  )
}
```

The following properties are also available to customize the look of the text field, most of them similar to those present in `Label` entities:

* `placeholder`: String to display before the player starts inputing anything. It's useful to make this text a hint about what they should write.
* `placeholderColor`: The color to use for the placeholder text, as a [Color4](/docs/creator/sdk7/3d-essentials/color-types/).


> [!NOTE]
> **💡 Tip**: Use a paler shade of the color of text that the player writes.


*   `fontSize`: The size of the text, as a number.

    > NOTE: The `fontSize` is not affected by the size of its entity or parent entities.
* `color`: The color of the text the player writes, as a [Color4](/docs/creator/sdk7/3d-essentials/color-types/).


> [!WARNING]
> **📔 Note**: Make sure you use a different color from the `placeholderColor`.


* `font`: The font to use. Supported values are: - `'sans-serif'` _(default)_ - `'serif'` - `'monospace'`

* `textAlign`: How the text will align with its parent. It takes a value from the \`TextAlignType\`. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';

You can also configure other components of the `Input` entity, like a `uiTransform`, `OnMouseDown` as in other UI entities.

```tsx
import { UiEntity, Input, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

var currentTextString = ''

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '50%',
        height: 150,
        flexDirection: 'column',
        alignContent: 'flex-start',
        margin: { left: 20, top: 20 },
        padding: { left: 10, top: 10, right: 10 },
        alignSelf: 'center',
      }}
      uiBackground={{ color: Color4.Gray() }}
    >
      <Input
        onChange={(e) => {
          currentTextString = 'you wrote: ' + e
        }}
        fontSize={35}
        placeholder={'type something'}
        placeholderColor={Color4.Gray()}
      />
      <Label value={currentTextString} fontSize={40} />
    </UiEntity>
)

```
