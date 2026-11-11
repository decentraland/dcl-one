> 了解有哪些特殊类型，包括 Vector、Quaternion 等。

# 特殊类型

## Vector3

Decentraland 使用 *Vector3* 数据来表示路径、空间中的点和方向。向量也可用于定义旋转朝向，作为比 *四元数*。Vector3 对象包含每个 *x*, *y*，和 *z* 轴。

```ts
const myVector: Vector3 = { x: 8, y: 1, z: 8 }
```

该 `Vector3` 命名空间包含一系列实用的方法，你可以调用它们，从而无需处理大多数向量运算。输入 `Vector3.`，VS Studio 就会显示一个包含所有可用函数的下拉菜单。

下面几行展示了向量一些基本操作的语法。

```ts
// 创建一个向量对象
let myVector = Vector3.create(3, 1, 5)

// 创建向量对象的另一种语法
let myOtherVector: Vector3 = { x: 8, y: 1, z: 8 }

// 编辑其中一个值
myVector.x = 5

// 从 Vector3 命名空间调用函数，
// 所有这些函数都需要在参数中传入 Vector3 对象

let normalizedVector = Vector3.normalize(myVector)

let distance = Vector3.distance(myVector, myOtherVector)

let midPoint = Vector3.lerp(myVector, myOtherVector, 0.5)
```

多个组件的字段中通常都需要使用 Vector3 对象。例如， `Transform` 组件包含 `Vector3` 实体的 *position* 和 *缩放* 值。

要创建一个 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 带有需要 Vector3 值的参数时，将这些参数的类型设为 `Schemas.Vector3`.


> [!WARNING]
> **📔 注意**: `Vector3` 必须通过
> 
> > `import { Vector3 } from "@dcl/sdk/math"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。


### 编写方向向量的快捷方式

用于定义通用向量的快捷方式有：

* `Vector3.Zero()` 返回 *(0, 0, 0)*
* `Vector3.Up()` 返回 *(0, 1, 0)*
* `Vector3.Down()` 返回 *(0, -1, 0)*
* `Vector3.Left()` 返回 *(-1, 0, 0)*
* `Vector3.Right()` 返回 *(1, 0, 0)*
* `Vector3.Forward()` 返回 *(0, 0, 1)*
* `Vector3.Backward()` 返回 *(0, 0, -1)*

## 四元数

四元数用于存储 Transform 组件的旋转信息。一个四元数由四个介于 -1 到 1 之间的数值组成： *x*, *y*, *z*, *w*.

```ts
const myQuaternion: Quaternion = { x: 0, y: 0, z: 0, w: 1 }
```

四元数不同于 [*欧拉* 角](https://en.wikipedia.org/wiki/Euler_angles)，这是更常见的 *x*, *y* 和 *z* 大多数人熟悉的从 0 到 360 的数字表示法。引擎将所有旋转都表示为四元数，因此尽可能避免在欧拉角和四元数之间进行转换计算是有意义的。

该 `Quaternion` 命名空间包含一系列实用的方法，你可以调用它们，从而无需处理许多数学运算。输入 `Quaternion.`，VS Studio 就会显示一个包含所有可用函数的下拉菜单。

下面几行展示了四元数一些基本操作的语法。

```ts
// 创建一个四元数对象
let myQuaternion = Quaternion.create(0, 0, 0, 1)

// 编辑其中一个值
myQuaternion.x = 1

// 从四元数命名空间调用函数
let midPoint = Quaternion.slerp(myQuaternion1, myQuaternion2, 0.5)

// 获取将一个朝向旋转到另一个朝向所需的旋转，
// 传入两个方向 Vector3 值
let rotation = Quaternion.fromToRotation(Vector3.Forward(), Vector3.Up())
```

由于用欧拉角度来思考要容易得多，SDK 包含了几个函数，可用于在四元数和欧拉角之间相互转换。


> [!NOTE]
> **💡 提示**：避免将这些转换作为系统内重复逻辑的一部分运行，因为它们会在每个 tick 执行，代价会很高。这些转换主要适用于一次性操作，例如设置新实体的旋转。


```ts
// 从欧拉角到四元数
let myQuaternion = Quaternion.fromEulerDegrees(90, 0, 0)

// 从四元数到欧拉角
let myEuler = Quaternion.toEulerAngles(myQuaternion)
```

Quaternion 对象通常是组件字段中所必需的。例如， `Transform` 组件包含 `Quaternion` 实体旋转的值。

要创建一个 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/) 带有需要 Quaternion 值的参数时，将这些参数的类型设为 `Schemas.Quaternion`.


> [!WARNING]
> **📔 注意**: `Quaternion` 必须通过
> 
> > `import { Quaternion } from "@dcl/sdk/math"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。


## 标量

标量不过就是一个数字。因此，实例化一个 `Scalar` 用于存储数据的对象并没有太大意义，因为用数字也能做到同样的事情。 `Scalar` 命名空间则提供了几个实用函数（类似于 *Vector3* 命名空间中的函数），可用于数字。

```ts
// 从 Scalar 类调用函数
let random = Scalar.randomRange(1, 100)

let midPoint = Scalar.lerp(number1, number2, 0.5)

let clampedValue = Scalar.clamp(myInput, 0, 100)
```


> [!WARNING]
> **📔 注意**: `Scalar` 必须通过
> 
> > `import { Scalar } from "@dcl/sdk/math"`
> 
> 参见 [导入](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/coding-scenes/#imports) 了解如何轻松处理这些内容。



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/special-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
