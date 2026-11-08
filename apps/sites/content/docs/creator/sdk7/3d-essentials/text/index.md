> How to add text to your scene

# Text

Add text to a scene using the `TextShape` component. This text sits in a position

Text in Decentraland supports all _utf8_ characters, this includes oriental and special characters.


> [!WARNING]
> **📔 Note**: This component is useful for in-world labels and UIs that exist in the 3D space of the scene, not for the player's 2D HUD UI.


To add text as a label on an existing entity, a good practice is to create a second entity that has the `TextShape` component and set it as a child of the other entity.

## Use the Scene Editor in Creator Hub

The easiest way to place text in-world is add a **Text** [Smart item](/docs/creator/scene-editor/interactivity/smart-items/) visually on the Scene Editor. You can then set all the available fields on the Scene Editor's UI.

![](/docs/creator/images/editor/text-smart-item.webp)

## Create a text component

The following example shows how to create a `TextShape` component and add it to an entity via code.

```ts
const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Hello World',
})
```


> [!WARNING]
> **📔 Note**: If the entity with the text component is a child of another entity, then it will be affected by the parent's scale. If the parent is scaled unevenly along its axis, this will result in the text also being stretched or compressed.



> [!WARNING]
> **📔 Note**: `TextShape` components aren't clickable. `PointerEvents` comopnents aren't activated when used on entites that have a `TextShape` component.



> [!WARNING]
> **📔 Note**: `TextShape` must be imported via
>
> > `import { TextShape } from "@dcl/sdk/ecs"`
>
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


## Change the text value

When creating a new text component, you assign it a string to display. This string is stored in the `text` field.

If you want to change the string displayed by the component, you can do so at any time by changing the `text` field on a [mutable version](/docs/creator/sdk7/programming-patterns/mutable-data/) of the component.

```ts
const mutableText = TextShape.getMutable(myEntity)

mutableText.text = 'new string'
```

## Basic text properties

The `TextShape` component has several properties that can be set to style the text. Below are some of the most common:

* `font`: Value from the enum `Font`.
* `fontSrc`: _string_. Path to a `.ttf` font file in your scene, to render the text with a custom font. See [Custom fonts](#custom-fonts).
* `fontSize`: _number_. An entiy with font 10 is 1 meter tall.
* `textColor`: _Color4_ object. _Color4_ objects store an _RBG_ color as three numbers from 0 to 1, plus _alpha_ for transparency. See [color types](/docs/creator/sdk7/3d-essentials/color-types/) for more details.

```ts
TextShape.create(sign, {
	text: 'Hello World',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```

## Fonts

Text shapes can use fonts from the enum `Font`. This enum currently includes the following fonts:

* `Font.F_SANS_SERIF`
* `Font.F_SERIF`
* `Font.F_MONOSPACE`

By default uses it uses `Font.F_SANS_SERIF`.

```ts
TextShape.create(sign, {
	text: 'Hello World',
	textColor: { r: 1, g: 0, b: 0, a: 1 },
	fontSize: 5,
	font: Font.F_SANS_SERIF,
})
```


> [!NOTE]
> **💡 Tip**: If using VS studio or some other IDE, type `Font.` and you should see a list of suggestions with all of the available fonts.


### Custom fonts

To render text with your own font, add a TrueType (`.ttf`) font file to your scene folder and set the `fontSrc` property to its path, relative to the scene's root folder (just like the `src` of a `GltfContainer`). Keep `font` set too: it's the fallback used whenever the custom font can't be loaded.


> [!WARNING]
> **📔 Note**: `fontSrc` needs `@dcl/sdk` 7.31.0 or later. On earlier versions the property doesn't exist.


```ts
TextShape.create(sign, {
	text: 'Hello World',
	fontSize: 5,
	fontSrc: 'assets/fonts/Lobster-Regular.ttf',
	font: Font.F_SANS_SERIF,
})
```

The same `fontSrc` property exists on the UI `Label`, `Button`, `Input` and `Dropdown` entities, and on the `uiText` of a `UiEntity`. See [Custom fonts in the UI](/docs/creator/sdk7/2d-ui/ui_text/#custom-fonts).

Keep the following in mind when using custom fonts:

* **Only `.ttf` files are supported**. The file must be a TrueType font. OpenType fonts with CFF outlines (usually `.otf`), font collections (`.ttc`) and web fonts (`.woff`, `.woff2`) are rejected, and the text renders with the fallback `font`.
* **The font must be a file in your scene**. `fontSrc` can't point to an external URL, and font family names (for example from Google Fonts) are not supported either. Download the `.ttf` file, include it in your scene folder and reference its path. Make sure the font's license allows redistributing it; most fonts on Google Fonts use the SIL Open Font License, which does.
* **On the Desktop Client, custom fonts only show up once the scene's assets are optimized**. When you publish a scene, its fonts are converted into optimized asset bundles together with its 3D models, and the custom font appears once that conversion is done. Until then, text uses the fallback `font`. Scenes published before custom fonts were supported only get their fonts converted when they're published again. The same applies to local previews: the custom font only shows when previewing with optimized assets (the **Compress Assets** option in the Creator Hub, or `npm run start -- --asset-bundles` from the CLI). See [Preview with optimized assets](/docs/creator/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). The Bevy web client loads the `.ttf` file directly, so there the custom font shows right away, in local previews too.
* **Not supported on mobile**. The Decentraland mobile app ignores `fontSrc` and renders the fallback `font`. Pick a fallback that still looks right, and don't rely on the custom font to make text fit in a fixed space. See [Missing Features on mobile](/docs/creator/build-for-mobile/mobile-client/missing-features/).
* **The font file can be at most 16 MB**. Larger files are rejected and the fallback `font` is used. Fonts with extremely complex glyph outlines may also be rejected.
* **Characters**: Latin characters (ASCII and Latin-1) are prerendered. Any other characters that the font includes, like Cyrillic or Chinese, are rendered the first time they appear in the scene, which can cause a brief hitch. Characters that the font doesn't include are drawn with the built-in sans-serif font.
* **One file is one face**. A `.ttf` file holds a single weight and style. For a bold title and regular body text, add both files and point each text at its own file. Use static font files: if a font family only ships as a variable font, use its static instances.
* **Fonts are shared**: all text entities that use the same `fontSrc` share a single copy of the font, so reusing a font across the scene costs no more than using it once. Prefer a handful of fonts over many different ones, since each font has its own texture. Each font file adds an optimized download of about 700 KB.

When the desktop client can't use a custom font, it logs a message in the `SDK_FONTS` category of the console that names the file and says why. For example, `font_src "https://example.com/font.ttf" cannot be an external URL`, or that the file isn't part of the scene's content, or that the scene has no converted font for it yet. These messages are hidden by default. To show them, type `/log-matrix enable SDK_FONTS Warning` and `/log-matrix enable SDK_FONTS Log` in the chat, reload the scene with `/reload`, then open the log folder with `/logs`. If the font wasn't converted, check that the file is a TrueType `.ttf` under 16 MB, then publish again.

## Text alignment and padding properties

The `TextShape` component creates a text box that has a size, padding, etc.

* `textAlign`: Select a value from the `TextAlignMode` enum. Possible values include all combinations between vertical (_top_, _middle_, _bottom_) and horizontal (_left_, _center_, _right_) alignment.
* `width`: _number_. The width of the text box.
* `height`: _number_. The height of the text box.
* `paddingTop`: _number_. Space between the text and the outline of the text box.
* `paddingRight`: _number_. Space between the text and the outline of the text box.
* `paddingBottom`: _number_. Space between the text and the outline of the text box.
* `paddingLeft`: _number_. Space between the text and the outline of the text box.


> [!NOTE]
> **💡 Tip**: If a text is meant to float in space, it's a good idea to add a [`Billboard` component](/docs/creator/sdk7/3d-essentials/entity-positioning/#face-the-player) so that the text rotates to always face the player and be legible.


## Text shadow and outline properties

The text has no shadow by default, but you can set the following values to give it a shadow-like effect.

* `shadowBlur`: _number_
* `shadowOffsetX`: _number_
* `shadowOffsetY`: _number_
* `shadowColor`: _Color3_ object. _Color3_ objects store an _RBG_ color as three numbers from 0 to 1.

```ts
TextShape.create(sign, {
	text: 'Text with shadow',
	shadowColor: { r: 1, g: 0, b: 0 },
	shadowOffsetY: 1,
	shadowOffsetX: -1,
})
```

The letters in the text can also have an outline in a different color surrounding its perimeter.

* `outlineWidth`: _number_. How wide the text outline will be, in all directions, as a number from 0 to 1. By default _0_, which makes it invisible.
* `outlineColor`: _Color3_ object. _Color3_ objects store an _RBG_ color as three numbers from 0 to 1.

## Multiple lines

If you want your text to span multiple lines, use `\n` as part of the string. The following example has two separate lines of text:

```ts
TextShape.create(sign, {
	text: 'This is one line. \nThis is another line',
})
```

You can also set up the following properties related to texts with multiple lines:

* `lineCount`: _number_. How many lines of text to fit into the textbox as a maximum. If not set, there is no limit to the number of lines. The `textWrapping` property must be _true_ to use more than one line.
* `lineSpacing`: _number_. How much space between each line.

## Which way the text faces

A `TextShape` reads correctly from its local **-Z** side. If you create one and don't rotate it, players standing on the entity's -Z side can read it.

So to label the **+Z** face of a model, parent the text to the model, offset it slightly on **+Z** so it doesn't sink into the mesh, and rotate it 180 degrees around Y:

```ts
// A label stuck to the +Z face of a model
import { engine, TextShape, Transform } from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const label = engine.addEntity()

Transform.create(label, {
	parent: myModel,
	// A small offset on +Z keeps the text clear of the model's surface
	position: Vector3.create(0, 1, 0.01),
	rotation: Quaternion.fromEulerDegrees(0, 180, 0),
})

TextShape.create(label, { text: 'Front' })
```


> [!NOTE]
> **💡 Tip**: If the text should stay readable from wherever the player stands, add a [Billboard](/docs/creator/sdk7/3d-essentials/entity-positioning/#face-the-player) instead of rotating it by hand.


## Lay out text in columns

Spaces and tabs don't line up columns in the default fonts, because their characters have different widths. Padding a scoreboard with spaces gives you ragged columns that shift as soon as a name or a number changes length.

To build a table, use one `TextShape` per column. Give each column its own entity, position it, and put the rows in its `text` field separated by `\n`. Every row then sits at the same height across all columns.

```ts
// A two column scoreboard: one TextShape for names, one for scores
const board = engine.addEntity()
Transform.create(board, { position: Vector3.create(8, 2, 8) })

const names = engine.addEntity()
Transform.create(names, { parent: board, position: Vector3.create(-1, 0, 0) })
TextShape.create(names, {
	text: 'Ana\nBruno\nChiara',
	textAlign: TextAlignMode.TAM_MIDDLE_LEFT,
	fontSize: 2,
})

const scores = engine.addEntity()
Transform.create(scores, { parent: board, position: Vector3.create(1, 0, 0) })
TextShape.create(scores, {
	text: '120\n85\n9',
	textAlign: TextAlignMode.TAM_MIDDLE_RIGHT,
	fontSize: 2,
})
```

Align each column towards the gap between the columns: `TAM_MIDDLE_LEFT` for the left column, `TAM_MIDDLE_RIGHT` for the right one. Numbers read best right-aligned, so their digits line up.

To update a row, rewrite the whole `text` string of each column. Keep the number of lines the same in every column, otherwise the rows stop matching up.
