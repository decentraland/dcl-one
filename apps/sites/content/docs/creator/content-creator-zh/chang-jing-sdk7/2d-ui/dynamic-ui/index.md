> 了解如何让 UI 动态响应数据变化。

# 动态 UI

你可以定义一个包含动态元素的 UI，这些元素会在每个 tick 时更新。你只需要处理代表这些数据的变量更新，UI 就会相应地根据新值自动调整。

这对于包含计时器、玩家分数等元素非常有用。但你甚至可以更进一步，基于状态定义整个 UI 结构。

## 引用变量

你可以在 uiEntity 中任意组件的任何属性里直接引用一个变量。随着该变量值的变化，UI 也会相应地自动调整。

下面的示例定义了一个变量 `playerCurrentPosition` 并将其作为字符串的一部分引用在一个 `uiText` 组件中。随后系统会使用玩家当前坐标在每个 tick 中更新该变量的值。随着变量值的变化，UI 也会相应更新，而无需显式修改 UI。

***ui.tsx 文件：***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { playerCurrentPosition } from './index'
import { Color4 } from '@dcl/sdk/math'

// 绘制 UI
export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `玩家：`+  playerCurrentPosition, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { engine, Transform } from '@dcl/sdk/ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}

// 定义变量
export let playerCurrentPosition: string = ""

// 用于更新变量的系统
engine.addSystem(() => {
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return
  const { x, y, z } = playerPosition.position
  playerCurrentPosition =  `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
})
```

在上面的示例中，你也可以通过将变量包裹在其中来将其作为字符串的一部分包含进去 `${ }`.

```ts
uiText={{
	value: `玩家：${playerCurrentPosition}`,
	fontSize: 40
}}
```

## 从 UI 内部调用函数

你也可以在 JSX 定义内部调用一个函数，返回一个值用于 UI 的某个属性。此 JSX 定义内部调用的函数会在游戏循环的每个 tick 中反复执行。

在下面的示例中，一个 `uiText` 组件调用了 `getPlayerPosition()` 函数来定义要显示的字符串的一部分。

这个示例与上一节中的示例类似，但通过在 UI 定义内部调用函数，我们避免了声明一个单独的变量以及定义一个系统来修改该变量。请注意， `getPlayerPosition()` 会在游戏循环的每个 tick 中被调用，而无需显式声明系统。

***ui.tsx 文件：***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine, Transform } from "@dcl/sdk/ecs";

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '100px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiText={{ value: `玩家：`+  getPlayerPosition(), fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)

function getPlayerPosition(){
  const playerPosition = Transform.getOrNull(engine.PlayerEntity)
  if (!playerPosition) return '加载中...'
  const { x, y, z } = playerPosition.position
  return `{x: ${x.toFixed(2)}, y: ${y.toFixed(2)}, z: ${z.toFixed(2)} }`
}
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```

## 切换 UI 的显示与隐藏

切换 UI 显示与隐藏的最简单方法是使用一个变量来作为以下属性的值： `显示` 实体的 `uiTransform`相同。 `显示` 属性在设置为时会使 UI 实体及其所有子元素不可见 `none`.

下面的示例使用一个变量来设置 `显示` UI 某一部分的字段。单击另一个 UI 元素即可切换此变量的值。

***ui.tsx 文件：***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

// 用于反映菜单可见性当前状态的变量
var isMenuVisible: boolean = false

// 切换菜单状态的函数
function toggleMenuVisibility() {
  isMenuVisible = !isMenuVisible
}

export const uiMenu = () => (
   // 父级
   <UiEntity>
      {/* 菜单 */}
      <UiEntity
       uiTransform={{
          width: '80%',
          height: '100px',
          alignContent: 'center',
          justifyContent: 'center',
          display: isMenuVisible ? 'flex': 'none'
        }}
         uiText={{
          value: "菜单",
          fontSize: 30
        }}
        uiBackground={{ color: Color4.Green() }}
      />
      {/* 按钮 */}
      <UiEntity
        uiTransform={{
          width: 100,
          height: 30,
          margin: { top: '35px', left: '500px' }
        }}
        uiText={{
          value: "切换菜单",
          fontSize: 40
        }}
        uiBackground={{ color: Color4.Red() }}
        onMouseDown={toggleMenuVisibility}
      />
   </UiEntity>
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

## 动态 UI 实体

上面的示例展示了如何动态更改单个实体中的一个属性，但你也可以定义整个实体结构，使其能够根据动态变化的数据进行扩展。这种模式在使用 React 等库的 Web 开发中很常见，而且非常强大。借助它，你可以定义极其灵活且可扩展的 UI 应用程序。

下面的示例列出了场景中所有具有一个 `MeshRenderer` 和 `Transform`的实体的 ID。它会创建一个 `uiText` 并为每个实体创建一个。随着场景内容的变化，UI 实体列表也会在每个 tick 中自动适应。

***ui.tsx 文件：***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'
import { MeshRenderer } from '@dcl/sdk/ecs'
import { Transform } from '@dcl/sdk/ecs'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '300px',
      justifyContent: 'center',
      alignItems: 'center',
    }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  >
    <UiEntity>
      {generateText()}
    </UiEntity>
  </UiEntity>
)


function generateText(){
  return Array.from(engine.getEntitiesWith(
    MeshRenderer,
    Transform
  )).map(([entity]) => <TextComponent value={entity.toString()} key={entity} /> )
}


function TextComponent(props: { value: string; key: string | number }) {
  return <UiEntity
    key={props.key}
    uiTransform={{ width: 80, height: 20 }}
    uiText={{ value: props.value, textAlign: 'middle-center', fontSize: 12 }}
    uiBackground={{ color: { r: 1, g: 0.176, b: 0.333, a: 1 } }}
  />
}
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu, { virtualWidth: 1920, virtualHeight: 1080 })
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/dynamic-ui.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
