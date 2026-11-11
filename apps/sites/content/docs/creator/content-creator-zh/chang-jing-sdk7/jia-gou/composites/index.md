> 在运行时从组合文件生成一棵实体和组件树

# 组合文件

一个 *组合体* 是一个描述树结构的文件， [实体和组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/)。与其编写代码逐个创建每个实体，不如将该结构存储在一个组合体文件中，并通过一次调用生成整个内容。

你可能已经在不知不觉中使用组合体了：

* 每个场景都有一个 `main.composite` 文件。它保存了你在 [场景编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/)。当你的场景启动时，此文件会自动加载。
* [自定义项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/custom-items/) 会保存为 `composite.json` 文件。每个自定义物品都是一个可在不同场景中复用的组合体。

本页介绍如何在运行时通过代码生成一个组合体，例如作为场景逻辑的一部分动态生成一个自定义物品。

## 生成组合体

生成一个组合体分两步： **加载** 组合体文件，然后 **生成** 它。加载是异步的，而生成是同步的，因此模式始终是先加载再生成：

```ts
import { engine, Composite, getCompositeProvider } from "@dcl/sdk/ecs";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  // 1. 从文件加载组合体
  const resource = await provider.loadComposite(src);

  // 2. 生成它：创建其所有实体和组件
  const barrel = Composite.instance(engine, resource, provider);
  return barrel;
}
```

`provider.loadComposite(src)` 会读取组合体文件并将其加载到内存中。在使用 `@dcl/sdk`构建的普通场景中，组合体提供者已经为你设置好了，你可以通过 `getCompositeProvider()` 中的函数 `@dcl/sdk/ecs`.

`Composite.instance()` 然后创建该组合体中描述的所有实体及其全部组件，并返回 **根实体** 。之后可使用此返回实体读取或修改组件，例如重新定位或移除生成的物品。


> [!NOTE]
> **💡 提示**：若不编写代码也要完成同样的事情，请使用 **生成实体** 动作。参见 [关于生成实体](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/#about-spawning-entities).



> [!WARNING]
> **📔 注意**：你必须先加载组合体，再生成它。 `Composite.instance()` 是同步的，只能生成已经在内存中的组合体。只有 `main.composite` 会与你的场景一起打包并从一开始就可用；任何其他组合体文件都必须先通过 `loadComposite()` 加载。若要同步检查某个组合体是否已可用，请使用 `provider.getCompositeOrNull(src)`.
> 
> `loadComposite()` 是幂等的：它会按每个组合体的 `src` 字符串作为键，因此再次使用相同路径调用时不会重新加载文件，而是返回已加载的组合体。你可以在每次生成前安全地调用它，而无需担心重复加载同一个文件。


### 生成到现有实体上

默认情况下 `Composite.instance()` 会创建一个新实体来承载生成的树结构。传入一个 `rootEntity` 作为选项，以便生成到你已有的实体上：

```ts
// 直接在场景根节点生成组合体，不使用包装实体
const barrel = Composite.instance(engine, resource, provider, {
  rootEntity: engine.RootEntity,
})
```

## 定位已生成的组合体

要将生成的组合体放置在特定位置、旋转或缩放，请在 `Transform` 由返回 `Composite.instance()`.

```ts
import { engine, Composite, getCompositeProvider, Transform } from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  const resource = await provider.loadComposite(src);

  // 在特定位置生成组合体
  const barrel = Composite.instance(engine, resource, provider);
  Transform.createOrReplace(barrel, {
    position: Vector3.create(8, 0, 8),
  });
}
```


> [!WARNING]
> **📔 注意**: `Transform.createOrReplace()` 会替换根实体现有的 Transform 组件。只有根实体会受影响，子实体会保持相对于根实体的位置。


## 当前限制：嵌套组合体

生成功能适用于自包含的组合体和自定义物品。一个组合体可以从其某个实体引用另一个组合体，但 `loadComposite()` 不会 **，** 递归处理这些嵌套引用：它只会加载你传入的文件。

在生成时， `Composite.instance()` 会递归处理内存中已有的嵌套引用。如果某个嵌套组合体尚未加载（除 `main.composite`之外，这是与你的场景一起打包的唯一组合体），该分支会记录警告并被跳过。要生成带有嵌套引用的组合体，请先对 `loadComposite()` 每个被引用的文件调用一次——或者更简单地，让你生成的组合体保持自包含。

## 相关页面

* [实体与组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/) —— 组合体所描述的构建块。
* [自定义项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/custom-items/) —— 作为组合体存储、可复用的物品。
* [智能物品 - 高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/#about-spawning-entities) —— 使用“生成实体”操作，无需代码即可生成组合体。
* [场景文件](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-files/) —— 在哪里 `main.composite` 位于你的场景项目中。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
