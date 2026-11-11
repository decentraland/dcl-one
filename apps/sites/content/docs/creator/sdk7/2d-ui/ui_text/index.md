> Write text in UI entities.

# UI Text

Add text to your UI by creating a `Label` entity.

A `Label` entity has the following fields that can be configured:

* `value`: The string to display
*   `fontSize`: The size of the text, as a number.

    > NOTE: The `fontSize` is not affected by the size of its entity or parent entities.
* `color`: The color of the text, as a [Color4](/docs/creator/sdk7/3d-essentials/color-types/).
* `font`: The font to use. Supported values are:
  * `'serif'`
  * `'sans-serif'` _(default)_
  * `'monospace'`
* `fontSrc`: Path to a `.ttf` font file in your scene, to render the text with a custom font. `font` is used as the fallback. See [Custom fonts](#custom-fonts).
* `textAlign`: How the text will align with its parent. It takes a value from the `TextAlignType` type. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';
* `textWrap`: Sets if the text uses line-breaks to prevent exceeding the maximum width. It's on by default (`'wrap'`), to deactivate it pass the value `'nowrap'`.


> [!WARNING]
> **📔 Note**: The `fontSize` is not affected by the size of its entity or parent entities.


A `Label` entity can also have other common components found on other types of UI entities, like `uiTransform` and `uiBackground`.

_**ui.tsx file:**_

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 'auto', height: 'auto' }}>
		<Label
			value="This is a label"
			color={Color4.Red()}
			fontSize={29}
			font="sans-serif"
			textAlign="top-left"
		/>
	</UiEntity>
)
```

_**index.ts file:**_

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


> [!WARNING]
> **📔 Note**: All the following snippets in this page assume that you have a `.ts` similar to the above, running the `ReactEcsRenderer.setUiRenderer()` function.


If a line of text is too long to fit in the assigned width, or the maximum width of its container, the text will continue on the next line. You can disable this by changing the value of the `textWrap` property to `'nowrap'`.

You can also force a line break by explicitly adding `\n` to the string.

```ts
import { UiEntity, Label, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
	<UiEntity uiTransform={{ width: 700, height: 400 }}>
		<Label 
			value="Hello World!\nThis other bit is quite long. It probably won't fit in a single line, so it will include a line break somewhere.\nFourth line"
			textWrap= {`nowrap`} 
		/>
	</UiEntity>
)
```

If no explicit `height` or `width` is set on the `uiTransform` of the container, the container will use the value `auto`, which adjusts to fit all the text. You can set a `maxWidth` and a `maxHeight` to ensure it doesn't exceed certain limits. You can also use `minWidth` and `minHeight` to ensure the container does't grow too small, even if the text is shorter.


> [!WARNING]
> **📔 Note**: Don't rely on that auto-fit for text. How much space a `Label` takes up when you leave its size unset differs between explorers. The Bevy-based explorer measures the rendered text and lays it out accordingly. The Unity explorer gives it almost no height, but still draws the letters, so stacked labels land on top of each other and a parent sized from its text collapses to nothing.
>
> Give every `Label` an explicit `width` **and** `height` in its `uiTransform`, and give an explicit height to any container that stacks labels. For wrapped text, size the height for the number of lines: two lines at `fontSize: 20` needs about `height: 60`.
>
> Because this differs per explorer, a preview that looks right in one client doesn't prove the layout is correct in another.


## Custom fonts

To render a `Label` with your own font, add a TrueType (`.ttf`) font file to your scene folder and set the `fontSrc` property to its path, relative to the scene's root folder. Keep `font` set too: it's the fallback used whenever the custom font can't be loaded.


> [!WARNING]
> **📔 Note**: `fontSrc` needs `@dcl/sdk` 7.31.0 or later. On earlier versions the property doesn't exist.


```tsx
<Label
	value="Welcome!"
	fontSize={40}
	uiTransform={{ width: 300, height: 60 }}
	fontSrc="assets/fonts/Lobster-Regular.ttf"
	font="sans-serif"
/>
```

The `Button` entity, the `uiText` of a `UiEntity` and the [`Input` and `Dropdown`](/docs/creator/sdk7/2d-ui/ui_special_types/) entities accept `fontSrc` too, and so does the [`TextShape`](/docs/creator/sdk7/3d-essentials/text/#custom-fonts) component for text in the 3D space.

Keep the following in mind when using custom fonts:

* **Only `.ttf` files are supported**. OpenType fonts with CFF outlines (usually `.otf`), font collections and web fonts are rejected, and the text renders with the fallback `font`.
* **The font must be a file in your scene**. `fontSrc` can't point to an external URL, and font family names (for example from Google Fonts) are not supported either. Download the `.ttf` file and include it in your scene folder. Make sure its license allows redistributing it.
* **On the Desktop Client, custom fonts only show up once the scene's assets are optimized**. When you publish a scene, its fonts are converted into optimized asset bundles together with its 3D models, and the custom font appears once that conversion is done. Until then, text uses the fallback `font`. Local previews in the Desktop Explorer convert the font too, so it shows there, unless you turned on **Load Raw GLTFs**. See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). The Bevy web client loads the `.ttf` file directly, so there the custom font shows right away, in local previews too.
* **Not supported on mobile**. The Decentraland mobile app ignores `fontSrc` and renders the fallback `font`. Pick a fallback that still looks right, and don't rely on the custom font to make text fit in a fixed space. See [Missing Features on mobile](/docs/creator/build-for-mobile/mobile-client/missing-features/).
* **One file is one face**. For bold and regular text, add both `.ttf` files and point each text at its own file.
* **The Creator Hub UI Designer doesn't show or edit `fontSrc`**. Add it to the element in your code by hand. The UI Designer only changes the properties you edit in it, so a `fontSrc` you wrote stays in place. Its canvas doesn't show the custom font.
* **Characters that the font doesn't include** are drawn with the built-in sans-serif font. Non-Latin characters that the font does include are rendered the first time they appear, which can cause a brief hitch.

For the full list of limits, like the maximum file size, see [Custom fonts](/docs/creator/sdk7/3d-essentials/text/#custom-fonts) in the Text page.

## Don't use emoji in UI text

Leave emoji out of any `Label` or `Button` `value`, `uiText.value`, `Input` `placeholder`, and `Dropdown` option.

The SDK doesn't ship emoji glyphs. Whether an emoji shows up depends on the fonts each explorer bundles, and the Unity explorer has none, so the character comes out as an empty box or disappears entirely. The same caution applies to other decorative Unicode such as arrows and box-drawing characters.

For a pictorial label, ship the art instead: put an image on a small `UiEntity` next to the text, using a `uiBackground` with a `texture`. See [UI Backgrounds](/docs/creator/sdk7/2d-ui/ui_background/).

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
	<UiEntity
		uiTransform={{
			minWidth: 100,
			maxWidth: 300,
			height: 'auto',
			alignSelf: 'center',
			padding: 10,
		}}
		uiBackground={{
			color: Color4.Red(),
		}}
		uiText={{
			value: 'Hello world!',
			fontSize: 18,
		}}
	/>
)
```

## Responsive text size

Use the `scaleFontSize()` function to provide font values that adjust to the player's screen size. When setting the `fontSize` property of a text UI entity, pass this function instead of a single number.

```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Hello world!',
      fontSize: scaleFontSize(15)
    }}
  />
)
```

The `scaleFontSize()` function requires two parameters:

* `fontSize`: The base font size to use.
* `scaleUnit` _(optional)_: The scaling factor. This determines if the text should be adjusted based on the screen width or the height, and a multiplier for how much to adapt. Default: `0.39` (a number, so it's interpreted as relative to _width_). Values can be:
  * _Number_: A simple number, in this case it gets interpreted as relative to _width_
  * _String ending in **vw**_: This makes the number relative to the view width. For example `"0.8vw"`
  * _String ending in **vh**_: This makes the number relative to the view height. For example `"0.8vh"`


> [!NOTE]
> **💡 Tip**: This function works similar to the CSS `calc()` function.


The value of `scaleUnit` is a percentage of the window's width or height. So a `scaleUnit` of `"100vw"` is 100% of the width of the screen, a value of `"0.5vw"` is 0.5% of the width of the screen.

The formula that `scaleFontSize()` follows is it multiples the screen width or height by the `scaleUnit`, and adds to that the `fontSize` passed in the first parameter.

```ts
final font = fontSize + (screen width * scaleUnit / 100)
```

For example, in the snippet below uses a `scaleUnit` value of 0.8. If the screen width is _1280px_, that will result in text of size of **25.24**, having followed the equation `15 + (1280 * 0.8 / 100)`.


> [!WARNING]
> **📔 Note**: `scaleFontSize()` returns a **number**, so its result is a size in *virtual* pixels — it is still multiplied by the [UI scale factor](/docs/creator/sdk7/2d-ui/onscreen-ui/#screen-virtual-scale) before being drawn. In the example above, with the default `1920x1080` virtual screen and a 1280px-wide canvas, the scale factor is `1280 / 1920 = 0.667`, so the text renders at about 16.8px. If you want a font size measured directly against the canvas, that ignores the virtual screen, pass a `vw`/`vh` **string** straight to `fontSize` instead — for example `fontSize: '1.8vh'`.


```ts
import { scaleFontSize } from '@dcl/sdk/react-ecs'
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 'auto',
      height: 'auto',
      alignSelf: 'center',
      padding: 10,
    }}
    uiText={{
      value: 'Hello world!',
      fontSize: scaleFontSize(15, 0.8)
    }}
  />
)
```


> [!NOTE]
> **💡 Tip**: If you don't have different screen sizes to test, you can try using the Web Explorer and resizing the window where you run the preview. The text will adjust instantly every time you change the window.


As an alternative to `scaleFontSize()`, pass a `vw`/`vh` string directly as the `fontSize` — for example `fontSize: '1.8vh'` — which sizes the text against the canvas and ignores the UI scale factor entirely. Do **not** multiply font sizes by a factor you compute yourself from `UiCanvasInformation`: the SDK already applies one, see [Responsive UI size](/docs/creator/sdk7/2d-ui/ui-positioning/#responsive-ui-size).
