> 了解外部 3D 模型支持哪些资源和组件，以及在导入到 Decentraland 之前如何进行配置。

# 3D 模型

![](/docs/_remote/62bf81e2e6497cde955d408a.webp)

将 3D 模型导入 Decentraland 时，重要的是确保它们采用 glTF 格式。这些模型可以包含各种受支持的功能。本节主要讲解如何使它们与 Decentraland 兼容，并遵循最佳实践。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


请注意，所有模型、着色器和纹理都必须符合 [**场景限制**](/docs/creator/sdk7/optimizing/scene-limitations/).

## **受支持的 3D 模型格式**

Decentraland 中的所有 3D 模型都必须采用 glTF 格式。 [**glTF**](https://www.khronos.org/gltf) （GL Transmission Format）是 Khronos 推出的一项开放项目，为 3D 资源提供一种通用、可扩展的格式，既高效，又与现代 Web 技术具有高度互操作性。

glTF 模型可以有 *.gltf* 或 *.glb* 扩展名。glTF 文件是可读的，你可以用文本编辑器打开并像阅读 JSON 文件一样查看它。这很有用，例如可以验证动画是否已正确附加，并检查它们的名称。glb 文件是二进制的，因此不可读，但体积要小得多，这有利于场景性能。

我们建议在 *.gltf* 在制作场景时使用，不过在切换为 *.glb* 上传时。

3D 模型的以下方面可以嵌入到 *glTF* 文件中，或从外部引用：

* 纹理可以嵌入，也可以引用外部图像文件。
* 关于几何体、动画以及模型其他缓冲区相关方面的二进制数据，可以嵌入，也可以从外部 *.bin* 文件。


> [!WARNING]
> **📔 注意**：动画 *必须* 嵌入到 *glTF* 文件中，才能在 Decentraland 中使用。另外，每个模型只允许有 1 个 UV 贴图。


## **我们为什么使用 glTF**

我们使用 glTF，是因为它提供了一组强大的功能，包括：

* 层级对象
* 骨骼结构和动画
* 材料和着色器的强大管线
* 场景信息，例如光源和摄像机（可用于未来开发）。

与 *COLLADA*相比之下，受支持的功能非常相似。然而，由于 glTF 专注于提供一种“传输格式”而不是编辑器格式，因此它与 Web 技术的互操作性更高。

## **导出和转换 glTF**

### **从 Blender 导出为 glTF**

Blender 自带 glTF 导出器。

导出时，先选择要包含在 .glb 文件中的模型，然后进入 File - Export - glTF 2.0

![](/docs/_remote/1f95091f3ce5ddb4219f6eea.webp) ![](/docs/_remote/3edef4656ec7f2427b2d9dbe.webp)

*在侧边栏中勾选“Selected Objects”，如果你只想导出场景中选中的对象。然后，在 Mesh 下拉菜单中选择 Apply Modifiers（如果有的话），再导出。*

### **从 3D Studio Max 导出为 glTF**

3D Studio Max 默认不支持导出为 glTF，但你可以安装插件来启用它。

1. 从以下位置下载插件 [此链接](https://github.com/BabylonJS/Exporters/tree/master/3ds%20Max).
2. 按照以下步骤安装插件 [这些说明](http://doc.babylonjs.com/resources/3dsmax#how-to-install-the-3ds-max-plugin).
3. 按照以下步骤使用该插件导出 glTF 文件 [这些说明](http://doc.babylonjs.com/resources/3dsmax_to_gltf).

### **从 Maya 导出为 glTF**

Maya 默认不支持导出为 glTF，但你可以安装插件来启用它。

1. 按照以下步骤安装插件 [这些说明](http://doc.babylonjs.com/resources/maya).
2. 按照以下步骤使用该插件导出 glTF 文件 [这些说明](http://doc.babylonjs.com/resources/maya_to_gltf#pbr-materials).


> [!WARNING]
> **📔 注意**：另外，你也可以试试 [这个其他插件](https://github.com/WonderMediaProductions/Maya2glTF) 也可以。


### **从 Unity 导出为 glTF**

Unity 默认不支持导出为 glTF，但你可以安装插件来启用它。

从以下位置下载插件 [此链接](https://github.com/sketchfab/Unity-glTF-Exporter).


> [!WARNING]
> **📔 注意**：另外，你也可以试试 [这个其他插件](https://assetstore.unity.com/packages/tools/utilities/collada-exporter-for-unity2017-99793) 也可以。


### **从 SketchUp 导出为 glTF**

SketchUp 默认不支持导出为 glTF，但你可以安装插件来启用它。

从以下位置下载插件 [此链接](https://extensions.sketchup.com/en/content/gltf-exporter).

### **将 FBX 转换为 glTF**

*.fbx* 是一个非常流行的 3D 模型标准。我们的引擎不支持它，但你可以轻松地导出 *.fbx* 模型到 *.gltf* 格式。

我们建议使用以下工具：

* [**Blender**](https://www.blender.org/download/)：将 .fbx 转换为 glTF 的一种简单方法，是先用 Blender 将 .fbx 导入到场景中，然后再从那里导出。
* [**Facebook 的 CLI 工具**](https://github.com/facebookincubator/FBX2glTF)：这是最稳妥的替代方案，但需要使用命令行。
* [**Blackthread**](https://blackthread.io/gltf-converter)：这是最完整的基于网页的工具。虽然没有 CLI 那么稳妥，但使用起来容易得多。
* [**Modelconverter**](https://modelconverter.com/convert.html)：另一个易于使用的基于网页的工具。

## **预览 glTF 模型**

在将 glTF 模型导入场景之前，快速预览其内容的一个简单方法是使用 [**Babylon.js Sandbox**](https://sandbox.babylonjs.com/)。只需将 glTF 文件（以及其 *.bin* 文件，如果适用）拖放到画布中即可查看模型。

在沙盒中，你还可以预览模型中嵌入的动画，并通过下拉菜单选择要显示的动画。

![](/docs/_remote/34159024016bd56402873a74.webp)

## **分析 glTF**

分析你的 glTF 的一个很棒的工具是使用 [**glTF Sample Viewer**](https://github.khronos.org/glTF-Sample-Viewer-Release/)。该工具允许你检查通道、纹理、对不同光照环境的响应、预览动画等等！非常适合单个模型。

![](/docs/_remote/173cb3c41baf0f4c374aea20.webp)

另一个有用的工具是 [**gltf.report/**](https://gltf.report/)。只需将模型拖放到网站上，即可查看 glTF 包的内容，并评估模型的哪些部分可以优化。

该工具对于包含多个模型、纹理和动画的大型文件尤其有用。例如，通过按大小整理纹理，你可以轻松找出哪些纹理占用了最多资源。

![](/docs/_remote/40647d97e437f4808c189320.webp)

## **优化 glTF**

以下工具提供了一些优化，可使 3D 模型更轻、更快地下载到你场景中的玩家。

[glTF pipeline](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)

其中包括，它会将 *.gltf* 格式转换为 *.glb*，它是二进制格式，因此占用空间要小得多。它还会将纹理文件放到 3D 模型外部，这样你就可以在多个模型上使用相同的纹理。


> [!WARNING]
> **📔 注意**: *.glb* 格式默认总是将纹理嵌入文件中。引擎无法将两个嵌入式纹理识别为同一个，它们需要是具有相同哈希值的外部文件。


## 另请参见

以下页面还涵盖了与 Decentraland 3D 模型相关的主题：

* [网格](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/meshes/)
* [材质](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/materials/)
* [纹理](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/textures/)
* [碰撞体](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/colliders/)
* [动画](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/animations/)
* [有用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/)：Blender 插件、资源库和 AI 工具，可加快你的 3D 工作流程


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/3d-models.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
