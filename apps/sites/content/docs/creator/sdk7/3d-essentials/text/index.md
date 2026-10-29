> How to add text to your scene

# Text

Add text to a scene using the `TextShape` component. This text sits in a position

Text in Decentraland supports all _utf8_ characters, this includes oriental and special characters.


> [!WARNING]
> **📔 Note**: This component is useful for in-world labels and UIs that exist in the 3D space of the scene, not for the player's 2D HUD UI.


To add text as a label on an existing entity, a good practice is to create a second entity that has the `TextShape` component and set it as a child of the other entity.

## Use the Scene Editor in Creator Hub

The easiest way to place text in-world is add a **Text** [Smart item](/docs/creator/scene-editor/interactivity/smart-items/) visually on the Scene Editor. You can then set all the available fields on the Scene Editor's UI.

![](/docs/creator/images/editor/text-smart-item.webp)

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


> [!WARNING]
> **📔 Note**: Currently, all fonts are rendered as Sans Serif. This is a known issue to fix in the future.



> [!NOTE]
> **💡 Tip**: If using VS studio or some other IDE, type `Font.` and you should see a list of suggestions with all of the available fonts.


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
> **💡 Tip**: If a text is meant to float in space, it's a good idea to add a [`Billboard` component](/docs/creator/sdk7/3d-essentials/entity-positioning/#face-the-user) so that the text rotates to always face the player and be legible.


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
