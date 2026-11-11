# 材质

了解导入到 Decentraland 的 3D 模型支持哪些材质属性和纹理。

材质会嵌入到一个 *.gltf* 或 *.glb* 文件。


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


本文档指的是导入到 3D 模型中的材质。对于通过代码定义并应用到原始几何体上的材质，请参阅 [**材质**](https://docs.decentraland.org/creator/development-guide/materials/) .


> [!WARNING]
> **📔 注意**：目前你无法通过场景代码动态更改 3D 模型的材质，除非它是原始几何体。


## 着色器支持

并非所有着色器都能用于导入到 Decentraland 的模型。请确保使用以下其中一种：

* **标准材质：** 支持任意着色器，例如漫反射、高光、透明度等。
* **PBR（基于物理的渲染）材质**：这种着色器更灵活，因为它包含漫反射、粗糙度、金属度和发光等属性，可让你配置材质与光的交互方式。

下图显示了两个使用相同颜色和纹理创建的相同模型。左侧模型全部使用 *PBR* 材质，其中一些包含 *金属度*, *透明度*，和 *发光度*。右侧模型全部使用 *标准* 材质，其中一些包含 *透明度* 和 *发光度*.

![](/docs/_remote/129845688fefc65d479ded1d.webp)

### 当前可与 Decentraland 引擎配合使用的 PBR 属性

> * 基础颜色
> * 金属度
> * 粗糙度
> * 高光
> * 自发光
> * 发光强度
> * Alpha
> * 法线

![](/docs/_remote/9edb0c21919b4803373b3885.webp)

为了直观地了解这些属性在世界中的表现，你可以前往这个 [测试世界](decentraland://?realm=TestEnvironment.dcl.eth) ，查看不同物体和材质，以及它们如何与灯光和世界环境交互。

![](/docs/_remote/1abdb0c1779701d95d2637dc.webp)

### 基础/漫反射颜色

定义对象表面的基础颜色。单独使用时，它不会受光照影响，因此通常会与粗糙度、金属度、高光等其他节点结合使用。

### 金属度

金属着色器是一种用于模拟金属表面外观的渲染技术。金属着色器会考虑金属的物理属性以及它们如何与光线相互作用，从而呈现金属特有的光泽和反射效果。

![](/docs/_remote/b601bec72f14836b99d6f451.webp)

### 粗糙度

粗糙度材质与光线如何与材质表面交互的真实模拟有关。通常会使用粗糙度贴图，为模型表面提供不同程度的“光滑”或“粗糙”效果。此类信息通常使用灰度纹理贴图来提供。

![](/docs/_remote/c4e81d0781e9fcd132dbed8f.webp)

此属性在非金属和金属材质模型之间进行混合。值为 1.0 时，会得到完全的高光反射，并带有基础颜色的染色，不包含漫反射或透射。值为 0.0 时，材质由漫反射或透射的基础层组成，上方叠加一层高光反射。

### 高光

在基于物理的渲染（PBR）着色器中，高光属性指的是光与表面的交互方式，具体表现为反射率和光泽度。高光反射是光从表面产生的镜面式反射。在 PBR 中，该属性用于控制表面以镜面方式反射多少光线。金属等材质通常具有较高的高光反射，产生清晰明亮的高光；而塑料等非金属材质的高光反射较低，因此高光更宽、更柔和。

![](/docs/_remote/92855ba60b2140bd9d3977ad.webp)

### Alpha

你可以将材质设置为 *透明*。透明材质可在不同程度上被看穿，这取决于其 *alpha*。为此，请启用材质的透明度属性，然后将其 *alpha* 设置为所需数值。Alpha 为 1 时材质将完全不透明，Alpha 为 0 时则会不可见。

下图显示了两个使用标准材质创建的相同模型。左侧模型只使用不透明材质，右侧模型的部分区域同时使用了透明和不透明材质。


> [!WARNING]
> 💡 请记住，与漫反射材质相比，使用透明材质的性能开销总是更高。请尽量将透明材质的使用降到最低。


![](/docs/_remote/69e874b1001f67ad59d3ac60.webp)

主要有两种不同的透明模式： *Alpha Clip* 和 *Alpha Blend*。主要区别如下：

* **Alpha Clip：** Alpha Clip 会根据灰度值的裁剪阈值，将渲染结果限制为 0 或 1 的绝对值。前一种颜色会被表面颜色覆盖，但前提是 alpha 值高于裁剪阈值。

![](/docs/_remote/6e885f9bbcbfca598d148a89.webp)

* **Alpha Blend：** Alpha Blend 会在 0 和 1 之间插值。你可以使用 alpha 混合将表面颜色叠加在前一种颜色之上。

![](/docs/_remote/293aba2fd75ef42086bd5c31.webp)


> [!WARNING]
> **🔥优化提示🔥** 除非你特别需要中间透明度级别， **否则在渲染上始终更高效的是使用 \_Alpha Clip***\*\* 而不是 \*\****Alpha Blend**\_**.**



> [!WARNING]
> **⚠️ 排序问题**
> 
> 在游戏中使用透明混合模式时，必须考虑颜色混合发生的顺序。因为最终输出颜色会受到混合顺序的显著影响。目前引擎仅支持按对象排序，这意味着它会根据对象原点自动对所有透明表面进行排序。不过，目前不支持按片段（像素）排序和按三角形排序。
> 
> 为避免与排序相关的问题，最好不要在同一个网格上同时使用 Alpha Clip 和 Alpha Blend。这有助于防止意外的混合伪影，并确保你的游戏呈现最佳效果。


### 自发光

你还可以让材质 *自发光*。自发光材质会发出自己的光。请注意，在渲染时，它们实际上并不会照亮场景中的附近物体，只是看起来周围有一层模糊的光晕。

下图显示了两个使用标准材质创建的相同模型。右侧模型的部分表面具有发光的自发光材质。

![](/docs/_remote/6a611f35617386705cb8bcce.webp) ![](/docs/_remote/4deead5b2aeaa81746397cb9.webp)

*要在 Blender 中让材质具有自发光效果，只需添加一个 `发光` 着色器到材质中。*

### 发光强度

发出的光强度。值为 1.0 时，可确保图像中的对象具有与发光颜色完全相同的颜色，即使其“无阴影”。

![](/docs/_remote/5392ea458ab888ee365b3678.webp)

*你可以在* [*测试世界*](decentraland://?realm=TestEnvironment.dcl.eth) *中查看发光强度在世界中的表现*

### 法线

PBR 着色器中的“normal”节点是用于控制材质表面法线的基础组件。法线是定义表面在特定点的垂直方向的向量，在决定光线如何与表面交互方面起着至关重要的作用。

![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

## 顶点绘制

Decentraland 引擎目前不支持 3D 模型的顶点绘制。

## 材质限制

请注意，每个地块的材质限制为：

> log2(n+1) × 20，其中 n 为场景中的材质数量。包括作为模型一部分导入的材质。

需要注意的是，每个材质都对应每个对象一次绘制调用，因此尽量将材质数量保持在最低并尽可能复用材质至关重要，例如使用纹理图集等技术；这也有助于让场景中的资源保持统一协调的风格。

## 材质命名

为了建立有序且健康的美术流程，我们建议你正确命名材质。一个方法是使用以下命名约定。

```
<对象>_<分类>_<子分类（可选）>_<_MAT>
```

例如，假设我们制作了两棵不同的树，一棵用于春天，具有自发光和发亮效果；另一棵用于冬天，偏冷色且带金属质感。我们可以将材质命名为： *“TreeSpring\_Emissive\_MAT”* 以及另一种 *“TreeWinter\_Metallic\_MAT”*

总之，

* 🟢 **优先** 使用以对象和分类开头的名称： *“Wood\_Oak\_MAT”*, *“SciFiFence\_Metallic\_MAT”*，等等。
* 🔴 **避免** 使用如下名称： *“Material009”*, *“material1”*，这会让场景和模型非常难以追踪和分析。

## 覆盖 glTF 材质

你可以覆盖一个 *glTF* 模型的材质，使用 [Gltf 节点修饰器](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 组件在你的场景代码中。请参阅 [修改 glTF 材质](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#modify-gltf-materials) 以了解更多详情。

## 材质最佳实践

* 如果你的场景包含多个使用相同纹理的模型，请将纹理作为外部文件引用，而不是将其嵌入 3D 模型中。

嵌入式纹理会为每个模型复制一份，从而增加场景体积。 *.glb* 文件默认会嵌入其纹理，但你可以使用 [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline) 将其提取到外部。

> 注意：在为不会嵌入的纹理引用文件后，请确保该文件不会被移动或重命名，否则对该文件的引用会失效。该文件还必须位于场景文件夹内，以便与场景一起上传。

* 在设置材质透明度时，尽量始终使用 *Alpha clip* 而不是 *Alpha blend*，除非你确实需要一个部分透明的材质（例如玻璃）。这样可以避免引擎将错误的模型渲染在另一个模型前方的问题。
* 经验法则是，请记得始终设置 *背面剔除* 在材质中启用它。这样会让你的场景性能更好，因为引擎只会渲染模型可见的一面。只有在以下情况下才取消勾选 *背面剔除* ：当你需要模型双面渲染时（例如由 3D 平面组成的一组树叶）。

![](/docs/_remote/83df362d7fc9ab62e2ae130d.webp)

* 使用 Decentraland [**默认纹理**](https://github.com/decentraland/builder-assets/tree/master/textures) ，这些纹理已被玩家预加载，可让你的资源渲染快得多。
* 阅读 [**这篇文章**](https://www.khronos.org/blog/art-pipeline-for-gltf) ，以详细了解在 glTF 模型中使用 PBR 纹理的完整美术流程。
* 你可以在以下位置找到关于如何使用 Blender 创建与 glTF 兼容的材质的详细参考： [**Blender 文档**](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html) .
* 在以下位置查找免费的高质量 PBR 纹理： [**cgbookcase**](https://cgbookcase.com/) .


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
