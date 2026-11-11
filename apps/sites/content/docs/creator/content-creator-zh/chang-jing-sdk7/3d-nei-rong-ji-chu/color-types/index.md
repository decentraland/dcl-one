> 如何定义颜色值

# 颜色类型

颜色值可以作为不同组件的属性传入，例如 [材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/), [文本](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text/) 或者作为以下对象的不同属性 [UI 实体](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/)。颜色值可以是以下类型之一： `Color3` 或 `Color4`.

`Color3` 包含三个属性， *红色*, *绿色*，和 *蓝色*. `Color4` 具有相同的三个属性，另外还有 `alpha`，用于透明度值。

## 设置颜色

你可以使用 `Color3.create()` 或 `Color4.create()` 函数来创建颜色。

```ts
// 红色
const red3 = Color3.create(1, 0, 0)
const red4 = Color4.create(1, 0, 0, 1)
```

你还可以创建某些预定义颜色，它们属于 `Color3` 和 `Color4` 命名空间。

```ts
const red = Color3.Red()
const blue = Color3.Blue()
const black = Color3.Black()
```


> [!NOTE]
> **💡 提示**：输入 `Color3.` 或 `Color4.` ，Visual Studio 应该会在智能下拉菜单中提示所有可能的值。


否则，你可以使用以下函数随机选择一种颜色：

```ts
const randomColor = Color3.Random()
```

如果你更喜欢用十六进制描述颜色，请使用 `Color3.fromHexString()`.

```ts
const red = Color3.fromHexString('FD350A')
const blue = Color3.fromHexString('0A0CFD')
```

任何对象值，只要其包含以下数值： `r`, `g`，和 `b` 都可以被解释为一个 `Color3` 值。同样，任何对象只要包含这些属性并额外包含一个 `a` 值，都可以被解释为一个 `Color4` 值。这也使你可以使用以下语法：

```ts
// 红色
const red3 = { r: 1, g: 0, b: 0 }
const red4 = { r: 1, g: 0, b: 0, a: 1 }
```

以下示例将颜色属性用作 `TextShape` 组件的一部分，用于设置文本颜色。

```ts
const myEntity = engine.addEntity()

Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

TextShape.create(myEntity, {
  text: '这段文本是红色的',
  textColor: Color4.create(1, 0, 0, 1),
})
```


> [!WARNING]
> **📔 注意**: `Color3` 和 `Color4` 必须通过
> 
> > `import { Color3, Color4 } from "@dcl/sdk/math"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。


## 透明度

使用 *alpha* 的 `Color4` 属性使颜色半透明。

如果 alpha 为 1，颜色将完全不透明；如果为 0，则将完全不可见。介于两者之间的值会产生部分透明的色调。

```ts
// 完全不透明
const opaque = Color4.create(1, 1, 1, 1)

// 半透明
const half = Color4.create(1, 1, 1, 0.5)

// 几乎不可见
const almostInvisible = Color4.create(1, 1, 1, 0.1)
```

## Lerp

使用 `Color3.lerp()` 或 `Color4.lerp()` 函数，用于定义介于另外两种颜色之间的颜色。这些函数的工作方式类似于 `Vector3.lerp()` 函数。

两者都 `Color3.lerp()` 或 `Color4.lerp()` 接受以下参数：

* `left`：第一个用作参考的颜色
* `right`：第二个用作参考的颜色
* `amount`：一个从 0 到 1 的数值，用于定义混合中使用多少 *right* 颜色。越接近 0，就越接近 *left* 颜色。

```ts
const orange = Color3.lerp(Color3.Red(), Color3.Yellow(), 0.3)
```

你可以使用系统逐渐改变 `amount` 参数，以创建平滑过渡。

```ts
// 系统
var pulseState: number = 0
const color1 = Color4.Red()
const color2 = Color4.Yellow()

export function PulseSystem(dt: number) {
  pulseState += dt
  // 在 0 和 1 之间振荡
  const amount = (Math.sin(pulseState) + 1) / 2
  const entitiesWithMaterial = engine.getEntitiesWith(Material)

  // 遍历该组中的实体
  for (const [entity] of entitiesWithMaterial) {
    const material = Material.getMutable(entity)
    if (material.material?.$case === 'pbr') {
      material.material.pbr.albedoColor = Color4.lerp(color1, color2, amount)
    }
  }
}

engine.addSystem(PulseSystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/color-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
