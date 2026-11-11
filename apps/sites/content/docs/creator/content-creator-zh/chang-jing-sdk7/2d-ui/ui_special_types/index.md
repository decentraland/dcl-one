> UI 的特殊实体类型，包括下拉菜单和输入框。

# UI 特殊类型

有一些特殊的实体类型，可用于实现某些特殊的交互。

## 下拉菜单

创建一个 `下拉菜单` 实体，让用户展开并从列表中选择一项。

一个 `下拉菜单` 实体至少必须具有以下属性：

* `选项`：下拉菜单展开时显示哪些值。请为每个选项提供一个字符串值数组。数组中的第一个值将作为默认选项显示。
* `onChange`：每次在下拉菜单中选择某个值时都会执行的函数，使用

你还可以配置该 `下拉菜单` 实体的其他组件，例如 `uiTransform`，就像其他 UI 实体一样。下拉菜单中的文本接受与 `标签`一样的样式属性，例如 `color`, `fontSize`, `font` 和 `fontSrc` 用于 [自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#custom-fonts).

***ui.tsx 文件：***

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
        value="选择颜色"
        fontSize={18}
        color={textColor}
        uiTransform={{
          width: '140px',
          height: '40px',
        }}
      />
      <Dropdown
        options={[`红色`, `蓝色`, `绿色`]}

        onChange={selectOption}
        uiTransform={{
          width: '100px',
          height: '40px',
        }}
      />
    </UiEntity>
)
```

***index.ts 文件：***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 注意**：本页下方的所有示例都假设你有一个 `.ts` 类似于上面的，正在运行 `ReactEcsRenderer.setUiRenderer()` 函数。


## 输入文本

创建一个 `Input` 实体，让用户输入文本。玩家必须先点击此框，然后才能在其中输入。

该 `Input` 实体通过以下属性进行管理：

* `onSubmit`：当玩家按下 Return/Enter 键时执行的函数，并将提供的文本作为输入。文本字段 **不** 在此情况下不会自动清空，你需要自行清空它（见下面的示例）。
* `onChange`：每当输入文本中的值发生变化时都会执行的函数。玩家在框中输入时，每添加或删除一个字符，此函数都会执行一次。
* `disabled`：如果 *true*，玩家将无法与输入实体交互。

以下示例使用 `onSubmit` 将提供的文本记录到控制台。

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
        console.log('提交的值：' + value)
      }}
      fontSize={35}
      placeholder={'请输入内容'}
      placeholderColor={Color4.Black()}
      uiTransform={{
        width: '400px',
        height: '80px',
      }}
    ></Input>
  </UiEntity>
)
```

同时提供一个提交按钮并处理 `onSubmit` 玩家按下 Enter/Return 键时触发的事件是个好做法。以下示例展示了如何实现。


> [!WARNING]
> **📔 注意**：不要 **不** 将 `value` 属性 `Input` 直接绑定到玩家通过 `onChange`输入的内容。 `value` 保持为空字符串，仅在你想清空输入框时才更改它。


要清空输入框，请将 `value` 设置为 `' '` 设置为（一个空格）。Explorer 会将其视为空并重新激活占位文本。由于 UI 函数每帧都会运行，请使用一个标志只在一帧内执行清空。

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
          console.log('提交的值：' + inputText)
          inputText = ''
          clearInput = true
        }}
        fontSize={35}
        placeholder={'请输入内容'}
        placeholderColor={Color4.Black()}
        value={inputValue}
        onChange={(value) => { inputText = value }}
        uiTransform={{
          height: '80px',
          margin: '15px',
        }}
      />
      <Button
        value="提交文本"
        variant="primary"
        uiTransform={{ alignSelf: 'center', padding: '25px' }}
        onMouseDown={() => {
          console.log('提交的值：' + inputText)
          inputText = ''
          clearInput = true
        }}
      />
    </UiEntity>
  )
}
```

以下属性也可用于自定义文本字段的外观，其中大多数与 `标签` 实体中的属性类似：

* `placeholder`：在玩家开始输入任何内容之前显示的字符串。将此文本作为提示，说明他们应该输入什么，这很有用。
* `placeholderColor`：占位文本使用的颜色，作为 [Color4](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/).


> [!NOTE]
> **💡 提示**：使用比玩家输入文本更浅一些的颜色。


* `fontSize`：文本大小，以数字表示。

  > 注意： `fontSize` 不受其实体或父实体大小的影响。
* `color`：玩家输入文本的颜色，作为 [Color4](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types/).


> [!WARNING]
> **📔 注意**：请确保你使用的颜色不同于 `placeholderColor`.


* `font`：要使用的字体。支持的值有：- `'sans-serif'` *（默认）* - `'serif'` - `'monospace'`
* `fontSrc`：指向 `.ttf` 场景中的字体文件路径，以使用自定义字体渲染文本。 `font` 将用作回退。参见 [自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#custom-fonts).
* `textAlign`：文本与其父级如何对齐。其值取自 \`TextAlignType\`。TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';

你还可以配置该 `Input` 实体的其他组件，例如 `uiTransform`, `OnMouseDown` 的其他组件，就像其他 UI 实体一样。

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
          currentTextString = '你输入了：' + e
        }}
        fontSize={35}
        placeholder={'请输入内容'}
        placeholderColor={Color4.Gray()}
      />
      <Label value={currentTextString} fontSize={40} />
    </UiEntity>
)

```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_special_types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
