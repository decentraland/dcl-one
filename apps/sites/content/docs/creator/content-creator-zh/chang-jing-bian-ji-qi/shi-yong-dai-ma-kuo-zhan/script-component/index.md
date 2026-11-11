> 使用 Script 组件为代码赋予功能，而无需深入整个项目结构。

# 使用脚本组件

借助新的 Script Component，可以创建在实体内部执行自定义代码的实体。

Script Components 允许在无需直接在 `index.ts` 以及可能的其他文件上操作的情况下执行实体的自定义行为。

## 设置 Script Component

1. 通过点击 `+` 按钮并选择它，即可将 Script Component 添加到实体。通过点击 **+ 添加新的脚本模块** 并选择一个名称，或者使用文件路径（浏览或拖放一个现有文件）。

![](/docs/_remote/2c5b0bcd10d2297a8c766d9b.webp)

2. 点击组件中的 CODE 按钮以打开默认代码编辑器。我们来看看它的结构。有关如何选择和管理默认编辑器的更多详情，请前往 [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/).

## 理解脚本结构

当脚本首次打开时，它包含以下代码：

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class BuildingScript {
  /**
   * 属性
   * 定义你想在各个方法中复用的类字段。
   * 使用示例：this.myVariable
   */
   // private myVariable: boolean = true

  /**
   * 构造函数 / 输入
   * 在此声明的参数会显示在 Creator Hub 的 Script component UI 中。
   * 支持的类型：Entity、String、Number、Boolean、ActionCallback 和 Slider
   * 对于使用滑块编辑的数字，例如 public speed: Slider<0, 10, 0.5> = 1
   *
   * 注意：编辑此文件后，请点击 Script component UI 中的刷新图标
   * 以查看更新后的输入。
   *
   * 构造函数中的 `src` 和 `entity` 字段是内部引用所必需的。
   */
  constructor(
    public src: string,     // 请勿移除
    public entity: Entity,   // 请勿移除
    // 在下方添加你的自定义输入
  ) {}

  /**
   * start()
   * 在脚本初始化时调用一次。
   */
  start() {
    // 脚本初始化
    console.log("BuildingScript 已为实体初始化:", this.entity);
  }

  /**
   * update(dt)
   * 每一帧调用。
   * @param dt -（可选）自上一帧以来的时间增量（秒）
   */
  update(dt: number) {
    // 每一帧调用
  }
}
```

这个类由三大部分组成：

* 该 **构造函数**,
* 、 **start()** method
* 、 **update()** 方法。

## 构造函数

构造函数包含你希望在 Creator Hub 中从场景里动态暴露和修改的参数。

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public numericVariable: number, 
  ) {}
...
}
```

文件保存后， **刷新** 按钮在 Script Component 中会更新所有已完成的更改。

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="刷新按钮" width="360">

刷新后，Script Component 现在会显示在代码中添加的 `numericVariable` 。

![](/docs/_remote/7bf7d0505e8aa1c4870ff35a.webp)

## 参数

如果不同的实体在 Script component 中使用同一个文件，它们仍然拥有独立的参数：如果场景中有两栋建筑， `building1` 和 `building2`，两者的 Script Component 都指向 `BuildingScript.ts` 文件，那么每栋建筑都有自己可独立修改的 `numericVariable` 参数。


> [!WARNING]
> **重要说明**：不要修改/删除 `public src: string` 和 `public entity: Entity`。你可以在这些字段后面继续添加新的输入。


构造函数参数允许的类型有：

* `实体`
* `string`
* `number`
* `布尔值`
* `ActionCallback`
* `Slider<Min, Max, Step>`

### 带滑块的数字参数

将数字参数写为 `Slider<Min, Max, Step>` ，即可用滑块而不是普通数字输入框来编辑它。Creator Hub 会同时显示滑块和数字输入框。

```ts
constructor(
  public src: string,
  public entity: Entity,
  // 从 0 到 10、每次步进 0.5 的滑块
  public speed: Slider<0, 10, 0.5> = 1,
) {}
```

* `步进` 是可选的，默认值为 `1`.
* 允许使用负值边界，例如 `Slider<-90, 90>`.
* 运行时，该值是一个普通的 `number`，因此 `this.speed` 的行为与任何其他数字参数一样。


> [!NOTE]
> **📔 注意**：两者都是 `公开` 和 `private` 构造函数参数都会暴露给 Creator Hub。 `private` 关键字只会限制在 `BuildingScript` 类内部的访问。有关更多详情，请参阅 TypeScript 官方文档中的\
> [参数属性](https://www.typescriptlang.org/docs/handbook/2/classes.html#parameter-properties).


### 在脚本中访问参数

要在代码中访问某个参数的值，请使用 `this.definedParameter`这样的写法。例如， `this.numericVariable` 或 `this.entity`.

默认脚本模板在 start() 方法中包含这一行：

`console.log("BuildingScript 已为实体初始化:", this.entity);`.

可以像这样修改它，以记录你在构造函数中定义的值：

`console.log("BuildingScript 已使用 numericVariable 初始化：`, `this.numericVariable);`

请注意，当你在 Creator Hub UI 中更改该参数的值时，你也应该看到此处记录的值随之变化。

### 默认参数

构造函数默认包含一个 `src` 以及一个 `entity` 参数，这些参数对你的脚本代码非常有用：

* `this.entity` 始终指向持有该 `脚本` 组件的实体，可用它获取该实体的信息或向其添加组件。
* `this.src` 是脚本存储的路径。这在创建供他人使用的 Smart Items 时特别有用。即使 Smart Item 的路径发生变化或被重命名，也可以使用此字段来构建随你的 smart item 一起打包的文件路径。

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
  ) {}

  start() {
    Material.setPbrMaterial(this.entity, {
      texture: Material.Texture.Common({
        src: this.src + '/images/myImage.png',
      })
    });
  }
}
```

上面的脚本会获取拥有该脚本的实体，并为其应用纹理。它从一个 `.png` 文件中获取纹理，该文件被打包在 smart item 文件夹中，位于名为 `/images`的子文件夹内。通过使用 `this.src`，我们可以确保文件路径始终已知，无论 smart item 是被导入到场景中的 `/assets/custom/itemName` 或 `/assets/asset-packs/itemName`

### 参数上的工具提示

为你的输入参数添加工具提示，这样用户就能知道这些字段是用来做什么的，或者接受哪些值。用户会在 Script component UI 中每个字段旁边看到一个工具提示图标，并且在悬停在该图标上时可以阅读自定义文本。

要为构造函数添加工具提示，请在构造函数前面添加一个注释块，并为每个工具提示写一行 `@param` 加上字段名称，然后跟上描述。

```ts
  /**
   * @param startDate - 事件的开始日期，格式为 YYYY-MM-DD
   * @param yOffset - 物体显示在地面以上多少米
   */
  constructor(
    public src: string,
    public entity: Entity,
    public startDate?: string,
    public yOffset: number = 0.5,
  ) {
  }
```

你可能需要点击 Script component UI 上的刷新图标，才能看到工具提示的更改。

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="刷新按钮" width="360">

## start() & update() 方法

该 **start()** 方法包含的代码只会在实体创建时执行一次（在本例中，即场景首次加载时）。

预览场景并检查日志（**提示**：你可以使用 `` ` `` 快捷方式）：它会显示新消息，其中包括 `numericVariable` 参数，可以将图像文件设置为材质上的纹理。

![](/docs/_remote/3391665f06d28b07d1c783f1.webp)

该 **update()** 方法则会在游戏的每一帧执行其代码（就像 Systems 一样）。例如，检查 `PlayerEntity` 的值，以便在脚本中触发行为。

下面的代码会在游戏每一帧打印日志，只要 `PlayerEntity` 高于先前定义的 `numericVariable`，该值由创建者通过 Script Component UI 动态提供。

```ts
update(dt: number) {
    if (Transform.get(engine.PlayerEntity).position.y > this.numericVariable ) {
      console.log("玩家的高度高于", this.numericVariable);
    }}
```

<img src="/docs/_remote/cff6c35c55f5fdd39baaefae.webp" alt="Update Method" data-size="line">

第一条日志属于 start() 方法，表示我们设置了 numericVariable。第二条属于 update() 方法，在玩家高于该值时输出。

## 向 Creator Hub 暴露动作

可以在 Script Component 脚本中定义一个 `动作` ，并让它可以在 Creator Hub 的 UI 中访问。这样就可以触发这个 `动作` ，并与另一个实体一起使用。

```ts
  /**
   * 暴露此动作以供触发
   * @action
   */
  exposedAction(creatorHubParameter: number) {
    console.log("使用参数从另一个实体触发：", this.creatorHubParameter);
  }
```

`creatorHubParameter` 将会作为一个 `动作` 参数暴露出来，以便赋予它自定义值。刷新 Script Component 后，新动作将作为 Actions 下拉菜单中的一个选项可用。

![](/docs/_remote/eb92dee98d90a44d44de70b3.webp)

添加该动作后，Creator Hub 中的任何实体都可以通过 `触发器`

![](/docs/_remote/e8bcd93f2829f1275473cb37.webp)


> [!NOTE]
> **📔 注意**：你可以在 Script 中根据需要添加任意多个动作。它们都可以独立于 `动作` 下拉菜单进行访问。


## 从外部调用脚本方法

要从另一个脚本或从 `src/index.ts`调用脚本方法，应遵循以下步骤：

1. 创建一个 `公开` 方法，位于 Script 类内部。
2. 运行 `npm run build` ，位于场景的根目录中。
3. 在你想使用该公有方法的文件中，添加 `import { callScriptMethod } from '~sdk/script-utils'`.
4. 调用 `callScriptMethod` 以及所需参数（本例中为 `someParamter`).

下面是一个带有 `公开` 已暴露方法的示例

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    ...,
  ) {}

  public publicMethod(boolParameter: boolean, someNumberParameter: number) {
    if (boolParameter) {
      console.log("使用参数 true 调用公有方法！：", someNumberParameter);
    } else {
      console.log("使用参数 false 调用公有方法！", someNumberParameter);
    }
  }
...
}
```

要从 `src/index.ts`调用它，请使用：

```ts
import { callScriptMethod } from '~sdk/script-utils'


export function main() {
    const buildingEntity = engine.getEntityOrNullByName("building")
    if (buildingEntity) {
        const scriptMethod = callScriptMethod(
            buildingEntity,
            "assets/scene/Scripts/BuildingScript.tsx",
            "publicMethod",
            false,
            3,
        )

        scriptMethod
    }
}
```

首先， `main` 函数会查找 `实体` 拥有 Script 组件的对象。其次，如果 `实体` 存在， `callScriptMethod` 将使用以下参数被调用：

1. `entity`: `实体` 并带有 `公开` 方法。
2. `scriptPath`: `path` 其中 `脚本` 类所在的位置。
3. `methodName`：要被调用的方法名称。 `公开` ...args
4. `：方法的参数。在本例中有两个。它们应按顺序依次添加。`第三，调用定义好的

，在本例中， `callScriptMethod`给定参数值后，输出为： `scriptMethod`.

：你也可以遵循相同逻辑来调用

![](/docs/_remote/732f383d31c15cab43ea285e.webp)


> [!NOTE]
> **📔 注意**另一个脚本中的脚本方法，或者从另一个文件中调用。你可以用它来获取或更改 `公开` 中的值 `公开` Script 类中的变量。


## 从脚本触发其他实体的动作

可以在 Script 类构造函数中使用类型为 `ActionCallback` 的参数。这样就可以从脚本的方法中触发通过 Creator Hub UI 定义的另一个 `实体`的 `动作` 。

在此示例中， `anotherEntityAction` 被添加为一个 `公开` 参数，可以将图像文件设置为材质上的纹理。

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public anotherEntityAction: ActionCallback,
    ...,
  ) {}
  ...
}
```

一个可选择的 `实体` 和 `动作` 现在在 Creator Hub UI 中刷新 Script Component 时可用。 `球体` 是场景中一个已存在的实体，它有一个名为 `缩放`.

![](/docs/_remote/b12b866e818c2c2562ab037c.webp)

的动作。其他实体的动作现在可以在 Script 类中访问。它可以用于多种不同方式。在下面的示例中，按下 E 将触发 `this.anotherEntityAction` ，通过定义一个 `pointerEventsSystem` 在 `开始` 方法。

```ts
  start() {
    pointerEventsSystem.onPointerDown(
      {
        entity: this.entity,
        opts: {
          button: InputAction.IA_PRIMARY,
          hoverText: "按 E 触发来自另一个实体的动作。",
        },
      },
      () => {
        this.anotherEntityAction();
      }
    );
  }
```

### 可选动作回调

一个 `ActionCallback` 创建者未连接的参数 `undefined`是 `ActionCallback | undefined`。在调用前先检查，这样脚本在动作是否连接的情况下都能正常工作：

```ts
if (this.anotherEntityAction) {
  this.anotherEntityAction();
}
```


> [!NOTE]
> **📔 注意**：结合暴露与触发 `操作` 是一个非常强大的工具。你可以在一个实体上定义 Script Component，使用一个 `公开` 方法暴露一个动作，然后通过另一个实体的 Script Component 使用一个 `ActionCallback` 参数，可以将图像文件设置为材质上的纹理。


## 另请参见

* [智能物件 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)
* [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/)
* [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)
* [让任何物品变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)
* [SDK 快速入门](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/sdk-101/)：跟随这个简短教程快速入门。
* [开发工作流](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/dev-workflow/)：阅读此内容，了解从头到尾的场景创建流程。
* [示例](/docs/creator/content-creator-zh/jiao-cheng-yu-shi-li/examples/)：直接进入可运行的示例场景。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
