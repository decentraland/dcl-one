> 设置 UI 实体的背景和边框。

# UI 背景

以下属性用于为 UI 实体设置背景和边框。

## 背景

一个 `uiBackground` 组件会为实体的区域提供颜色或纹理。它使用实体的 `uiTransform`.

以下字段可以配置，全部都是可选的：

* `color`：要在实体上使用的颜色，作为一个 [Color4](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/) 值。


> [!NOTE]
> **💡 提示**：将 `Color4` 的第 4 个值设为小于 1，即可让实体半透明。


* `texture`：要显示在实体上的纹理；它接受一个包含纹理各项参数的对象。可用属性与 3D 实体上的材质中的纹理属性相同，见 [3D 实体上的材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#using-textures).

  * `src`：要用作纹理的图像文件路径。（字符串）

  * `filterMode`: *（可选）* 决定纹理在渲染时像素如何被拉伸或压缩。见 [纹理缩放](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#texture-scaling)。（FilterMode = 'point' | 'bi-linear' | 'tri-linear'）

  * `wrapMode`: *（可选）* 决定纹理如何平铺到实体上。它取值于 `TextureWrapMode` 枚举。见 \[纹理包裹]\(([查看文档](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)）。(WrapMode = 'repeat' | 'clamp' | 'mirror')

  > 提示：你可以将这两个 `texture` 和 `color` 属性组合在一个 `uiBackground` 组件上，以生成带色调的纹理。
* `textureMode`：选择纹理适应其所应用实体大小的方式。（TextureMode = 'nine-slices' | 'center' | 'stretch'）枚举，支持以下值：
  * `center`：纹理不会被拉伸，它会居中显示在实体上，部分内容可能会根据实体大小被裁剪。
  * `stretch`：纹理会被拉伸以匹配实体的整个表面。
  * `nine-slices`：纹理的部分区域会被拉伸以匹配实体的整个表面，而边距保持不拉伸。见 [九宫格纹理](#nine-slice-textures).
* `avatarTexture`：根据头像 ID 显示头像资料缩略图。见 \[头像肖像]\(([查看文档](https://github.com/decentraland/docs/tree/main/creator/sdk7/README.md)).
* `textureSlices`：在使用九宫格纹理模式时，确定要使用的边距，见 [九宫格纹理](#nine-slice-textures)。将数值设为小于 1 的数字，作为图像总宽度或总高度的一个分数。

简单颜色：

***ui.tsx 文件：***

```tsx
import { ReactEcs, UiEntity } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		color: Color4.create(0.5, 0.8, 0.1, 0.6)
	}}
  />
)
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


> [!WARNING]
> **📔 注意**：本页下方的所有示例都假设你有一个 `.ts` 类似于上面的，正在运行 `ReactEcsRenderer.setUiRenderer()` 函数。


重复纹理图案：

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400
    }}
    uiBackground={{
		textureMode: 'center',
		texture: {
			src: "images/brick-wall-texture.png",
			wrapMode: 'repeat'
		}
	}}
  />
)
```

## 边框

有几个属性用于为 UI 实体设置边框。这些属性位于 `uiTransform` 组件上。它们都允许你为边框的所有边设置单个值，或为每一侧设置不同的值。

* `borderColor`：要在实体上使用的颜色，作为一个 [Color4](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/) 值。
* `borderWidth`：边框的宽度，单位为像素。它也支持百分比值，例如 `borderWidth: '2%'` 会将边框宽度设置为实体宽度的 2%。
* `borderRadius`：使用此属性可让实体的角变成圆角边框。它以像素设置圆角半径。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: Color4.Red(),
      borderWidth: 4,
      borderRadius: 10
    }}
  />
)
```

`borderWidth`, `borderColor` 和 `borderRadius` 也可以为实体的每一侧设置不同的值。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      borderColor: { top: Color4.White(), left: Color4.Red(), right: Color4.Blue(), bottom: Color4.Gray() },
      borderRadius: { topLeft: 20, topRight: 20, bottomLeft: 20, bottomRight:0 },
      borderWidth: { top: 3, left: 2, right: 3, bottom: 4 }
    }}
  />
)
```

## 不透明度

使用 `opacity` 属性在 `uiTransform` 一个 `UiEntity` 中，用于为实体及其所有子项添加透明度。opacity 属性的取值范围是 0 到 1，其中 0 表示完全透明，1 表示完全不透明。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
        }}
        uiText={{
          value: "这段文字也同样透明",
          fontSize: 40
        }}
      />
   </UiEntity>
)
```

opacity 值会影响 UiEntity 的所有子项，并将透明度应用到背景颜色、文字颜色和背景图像上。当父级和子级都设置了 opacity 值时，子级的最终不透明度是自身值与父级值的乘积。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 700,
      height: 400,
      opacity: 0.7
    }}
    uiBackground={{ color: Color4.Green() }}
  >
    <UiEntity
      uiTransform={{
        width: 100,
        height: 30,
        opacity: 0.7
      }}
      uiText={{
        value: "这段文字会更加透明",
        fontSize: 40
      }}
    />
  </UiEntity>
)
```

## 九宫格纹理

你可以使用 [9 切片缩放](https://en.wikipedia.org/wiki/9-slice_scaling) 来处理纹理，以确保角落和边距不会被不均匀地拉伸。

通过这种常见技术，你可以将一张图像切成 9 个部分，并以不同方式拉伸，以保持边距和角落的比例。例如，可用它来定义可轻松适配任意大小的圆角背景。请考虑下图（借用自 [维基百科](https://en.wikipedia.org/wiki/9-slice_scaling#/media/File:Traditional_scaling_vs_9-slice_scaling.svg)):

![](/docs/_remote/e03c4d135bed8010ce6253a3.webp)

在这张图中，我们看到了原始纹理（左上）以及以传统方式缩放后的结果（右上）；注意角落是如何变形的。在此之下，我们看到了被分成 9 个切片的纹理（左下），以及按 9 切片方法拉伸后的结果（右下）。

以下是每个部分如何受到影响的说明，以上图为参考。

* 第 5 个部分是唯一在 x 和 y 轴上都被完全拉伸的区域。
* 第 1、3、7 和 9 部分（角落）根本不会被拉伸。
* 第 2 和 8 部分只在水平方向上拉伸
* 第 4 和 6 部分只在垂直方向上拉伸。

要在实体上使用九宫格拉伸，请将 `textureMode` 设置为 `'nine-slices'`。你也可以选择在 `textureSlices`.

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 700, height: 400 }}
    uiBackground={{
      textureMode: 'nine-slices',
      texture: {
        src: 'images/rounded_alpha_square.png'
      },
      textureSlices: {
        top: 0.2,
        bottom: 0.2,
        left: 0.2,
        right: 0.2
      }
	}}
  />
)
```

## 纹理 UV

使用 `uvs` 组件上的属性，用于显示纹理的特定区域。这对于从图集里挑选单独的精灵，或旋转图像都很有用。 `uiBackground` 字段接受一个包含 8 个数字的数组，表示纹理区域四个角的 4 对 UV 坐标。顺序如下：

底左 `uvs` 字段 **左上**, **右上**, **右下**, **。每个值的范围都是 0 到 1，其中**是图像的左下角，而 `(0, 0)` 是右上角。 `(1, 1)` ：当使用自定义


> [!NOTE]
> **💡 提示**时，设置 `uvs`， `textureMode` 设置为 `'stretch'` 以便所选区域填满实体的面积。


### 来自精灵图集的精灵

要显示较大图像的一部分（例如，包含多个卡片的精灵图集中的一张卡），请将 `uvs` 设置为该区域的坐标。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// 显示纹理的左半部分（例如，2 列图集中的第一张卡）
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 200, height: 300 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/card-atlas.png' },
      uvs: [
        // 左下、左上、右上、右下
        0, 0,
        0, 1,
        0.5, 1,
        0.5, 0
      ]
    }}
  />
)
```

对于带有帧网格的精灵图集（例如，4 列 2 行的图集），请根据你想要的帧的列和行来计算 UV：

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'

// 从网格精灵图集中选取单个帧
function getFrameUVs(col: number, row: number, totalCols: number, totalRows: number): number[] {
  const stepU = 1 / totalCols
  const stepV = 1 / totalRows
  const left = col * stepU
  const right = (col + 1) * stepU
  const top = 1 - row * stepV
  const bottom = 1 - (row + 1) * stepV
  return [
    left, bottom,
    left, top,
    right, top,
    right, bottom
  ]
}

// 显示 4x2 精灵图集中的第 2 列、第 0 行
export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spritesheet.png' },
      uvs: getFrameUVs(2, 0, 4, 2)
    }}
  />
)
```

### 使用 UV 旋转图像

你可以通过对 UV 坐标应用 2D 旋转来旋转纹理。这对于旋转器或加载指示器很有用，而无需基于变换的旋转。

```ts
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { engine } from '@dcl/sdk/ecs'

// 绕中心点旋转一个 2D 点
function rotate2D(angle: number, x: number, y: number, cx: number, cy: number): number[] {
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  return [
    cos * (x - cx) - sin * (y - cy) + cx,
    sin * (x - cx) + cos * (y - cy) + cy
  ]
}

// 构建旋转后的 UV 坐标
function rotateUVs(angle: number): number[] {
  const uv00 = rotate2D(angle, 0, 0, 0.5, 0.5)
  const uv01 = rotate2D(angle, 0, 1, 0.5, 0.5)
  const uv11 = rotate2D(angle, 1, 1, 0.5, 0.5)
  const uv10 = rotate2D(angle, 1, 0, 0.5, 0.5)
  return [uv00[0], uv00[1], uv01[0], uv01[1], uv11[0], uv11[1], uv10[0], uv10[1]]
}

let spinnerAngle = 0

// 每帧更新角度的系统
engine.addSystem((dt: number) => {
  spinnerAngle += dt * 5
})

export const uiMenu = () => (
  <UiEntity
    uiTransform={{ width: 128, height: 128 }}
    uiBackground={{
      textureMode: 'stretch',
      texture: { src: 'images/spinner.png' },
      uvs: rotateUVs(spinnerAngle)
    }}
  />
)
```

底左 `rotateUVs` 函数会将四个 UV 角点围绕中心点 `(0.5, 0.5)` 按给定的弧度角进行旋转。由于这会被类似 React 的渲染器在每一帧调用，因此旋转器会平滑更新。


> [!NOTE]
> **💡 提示**：在 [UI 动画](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/ui-animations) 示例场景中查看包含精灵、动画精灵图集、旋转器等内容的完整工作示例。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_background.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
