> 了解纹理在 Decentraland 中的工作方式

# 纹理

纹理是 3D 美术流程中的关键部分，可帮助实现你希望用于场景的视觉效果和氛围。在本节中，你将找到关于创建纹理、限制、Blender 中的节点，以及如何对其进行优化以使其发挥最佳性能所需了解的一切！


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


## 局限性

### 纹理尺寸限制

目前 Decentraland Explorer 会在场景上传到内容服务器后，使用 Asset Bundle Converter 为了优化目的将纹理压缩到最大 1024px。创建资源时务必考虑这一限制！

另一个需要考虑的重要点是，纹理应始终为 2 的幂；任何不符合此规范的纹理在渲染场景时都可能带来问题。

纹理尺寸必须使用以下匹配的宽高数值（以像素为单位）：

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> 这一序列由 2 的幂组成： `f(x) = 2 ^ x` . **我们允许的纹理尺寸最大值是 1024。** 这是其他渲染引擎中相当常见的要求，这是由于图形处理器的内部优化所致。

宽度和高度不必相同，但它们都必须属于这个序列。

**推荐的纹理尺寸为 1024x1024**，我们发现这是在国内网络中传输并提供合理加载/质量体验的最佳尺寸。

其他有效尺寸示例：

```
32x32px
64x32px
512x256px
512x512px
1024x1024px
```

另一方面，每个地块的纹理数量也有限制：

```
log2(n+1) × 10 每个地块的纹理数量。包括作为模型一部分导入的纹理。
```

## UV 映射

UV 映射是将 3D 模型的面展开为 2D 坐标的过程，之后会用这些坐标来为资源添加不同的贴图。它是创作流程中的关键部分。正确地展开模型是最大化模型分辨率以及让贴图便于修改的关键因素。

想了解更多关于 UV 展开，你可以观看 Blender Foundation 制作的这段超棒视频：

[![Video Preview](/docs/_remote/b25b76a5b0e4c1d0cec817e8.webp)](https://youtu.be/Y7M-B6xnaEM?si=qMnWTMsXxC-vxZAH)

这里还有另一个由 \[Blender Guru]\(<https://www.youtube.com/@blenderguru>) 制作的优秀中级教程，讲解如何展开更复杂的模型：

[![Video Preview](/docs/_remote/2b79a21fed4d0ba2abc23bfa.webp)](https://youtu.be/scPSP_U858k?si=Uw0xHbv9jtqVstDS)

## 贴图

在 [材质部分](/docs/creator/3d-modeling/materials/) 我们解释了 Decentraland 如何使用 PBR 着色器。在本节中，我们将向你展示每种纹理贴图如何影响着色器，以及之后将导出到世界中的 3D 对象的外观和感觉。

## 漫反射贴图

这是对象表面的基础颜色。让你的模型之间拥有协调的色彩搭配，是实现统一视觉效果和体验氛围的关键。

![](/docs/_remote/3a9f1ec299edbbb69e0531e0.webp)

如果你需要，这里有一个很棒的免费调色板生成器！

* **Coolors：** <https://coolors.co/>

或者来自动人影片的一些配色灵感：

* **Movies in Color：** <https://moviesincolor.com/>

## 金属度贴图

在金属度贴图中，灰度图表示对象具有的金属程度，白色表示完全金属，黑色表示非金属。在下面的示例中，我们可以看到光线如何影响模型，以及它如何与环境相互作用。

![](/docs/_remote/d7d680ed47a91de7cef0901a.webp)

*在图中我们可以看到纹理的深色部分如何影响模型。深色条纹是不透明的，而白色条纹则在金属着色器属性下表现，灰色则是两者之间的混合。*

## 粗糙度贴图

在粗糙度贴图中，较暗的区域对应更光滑的表面，而较亮的区域对应更粗糙的表面。这种灰度表示用于渲染引擎来判断光线应如何在表面的不同位置散射或反射。

![](/docs/_remote/ab1077956ce52917b01f28a4.webp) ![](/docs/_remote/cf4b607b56f34ffc9787473a.webp)

## 透明贴图

### Alpha Clip

在 Blender 中使用纹理时，Alpha Clip 涉及使用纹理的 alpha 通道来确定材质的哪些部分应可见。超过指定阈值的像素会显示，低于阈值的像素会被丢弃，从而根据纹理的透明度信息创建裁切效果。

在下面的示例中，我们使用了带有 alpha clip 的材质，使用两张纹理：漫反射颜色和黑白 alpha 纹理，将颜色连接到 alpha 通道。

![](/docs/_remote/a6eb0c3607b3ac754dd99f4a.webp) ![](/docs/_remote/7efa7539bc36fae5c645b2f7.webp)

![](/docs/_remote/85b8e0c6b3f5f217698536fc.webp)

*结果我们可以看到，被涂成黑色的区域会被丢弃，而白色区域会被渲染。*

### Alpha Blend

*Alpha Blend* 让你可以为每个区域选择中间值。

在 Blender 中使用纹理时，Alpha Blend 涉及根据 alpha 通道混合纹理的透明和不透明区域。这样纹理可以平滑地结合可见和透视部分，并与背景或场景中的其他对象自然交互。

![](/docs/_remote/46cdac0bd41b0f225bd498e4.webp)

*Alpha Clip 的渲染值为 0 或 1（在特定阈值下），而 Alpha Blend 会在 0 和 1 之间插值。在上面的示例中，alpha blend 材质显示完整的渐变过渡，而 alpha clip 会排除由裁切阈值设定的部分纹理。*


> [!WARNING]
> **🔥优化提示🔥**
> 
> * 请注意，透明纹理（RGB+A）在性能方面总是比使用灰度值来表示透明度更昂贵。
> * 透明效果在渲染场景时始终是一项昂贵的操作。尽量将透明度保持在最小，并且只在必要时使用 Alpha Blend；否则，比起 Alpha Blend，更推荐使用 Alpha Clip。


## 发光贴图

发光贴图是一种用于控制 3D 场景中表面自发光或发射光线的纹理贴图。它是着色器的一个组成部分，用来决定 3D 模型某一部分会发出多少光（以及何种颜色的光），而不依赖外部光源。发光贴图通常用于模拟看起来会自行发光的材质或物体。

![](/docs/_remote/e4c3fc1cd832dcc33edb3f1c.webp)

*在这个示例中，我们可以看到发光贴图与发光强度结合使用，在一个带有辉光后期处理的环境中，近似测试发光在世界中的表现。*

## 法线贴图

法线贴图是一种用于 3D 图形的纹理，用来模拟细微表面细节，并在不实际改变模型底层几何的情况下营造复杂几何体的错觉。它通常用于通过增加凹凸、裂隙和其他表面不规则的外观来提升低多边形模型的真实感。它也能让对象本身更轻量，因为许多细节可以放在法线贴图层中提供，而不需要复杂几何。

![](/docs/_remote/76188ce5daf39c2ac1f11691.webp) ![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

要通过 *Shader Editor*，你需要连接 ***Normal Map*** 节点在纹理和 *Principled BSDF* 着色器之间。

![](/docs/_remote/813d6e575903a5c3a19e8e2e.webp)


> [!WARNING]
> **⚠️重要⚠️：** 不要同时将同一张纹理用作 albedo 和 normal，否则在渲染场景时可能会造成问题。


## 优化纹理

优化纹理的过程在探索器中渲染场景时带来很多好处，同时也是让场景风格保持一致、并在设计过程中更灵活的好方法。这些好处包括：

* 尺寸和压缩都经过优化的纹理会让场景运行得更顺畅、更快，使其下载更快、渲染更容易（尤其是对于网速较慢的玩家）。
* 它减少了渲染你的体验所需的内存和处理能力，从而为玩家带来更好的用户体验。
* 它节省了社区内容服务器的存储空间。
* 使用 **纹理图集** 和/或 **修边贴图** 将让你在迭代场景创作以及保持对象之间风格一致性方面拥有更高的灵活性。使用这些技术，你可以轻松替换纹理、调整颜色或图案，而不必为每个模型单独处理。
* 在模型之间共享纹理可以减少每个场景所需的纹理数量，显著降低游戏中的绘制调用。若你使用 glb（带内嵌纹理），你会在下面的指南中找到一个提取器，可从中提取纹理，并让模型改用相同的纹理。

## glTF 模型之间共享纹理

在优化场景时，一个明智且常见的做法是在整个场景中的模型之间共享纹理和材质。这样做可以大幅减少绘制调用，让你的 Decentraland 场景运行得更流畅。

以下基于 [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)**,** 提供了一些优化，可让 3D 模型更轻量，并让场景中的玩家下载得更快。

**Mac：** [MAC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/texture_extractor.sh)

**PC：** [PC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/glb_texture_extract.bat)

它将 .gltf 格式转换为 .glb，后者是二进制格式，因此占用更少。它还会将纹理文件放在 3D 模型之外，从而允许你在多个模型上使用同一纹理。

> 📔 注意：.glb 格式默认总是将纹理嵌入文件中。引擎无法将两个嵌入式纹理识别为同一个，它们需要是共享相同哈希值的外部文件。

### 如何使用 GLB 纹理提取器

在这个示例场景中，我们在 Blender 中有一个简单的科幻场景。

![](/docs/_remote/09c63fb13acc8a4914118992.webp)

这个场景包含静态模型的基础环境，但还有另外两个资源：一个机器人和一艘飞船，我们希望分别导出，以便之后通过代码移动它们，让它们可以与玩家交互。在这种情况下，我们使用了 4 张纹理（地板一张，大部分资源使用一张图集颜色贴图，一张发光贴图，以及面板使用的一张 UI 科幻纹理）

![](/docs/_remote/1167ef79495fce28d3f04994.webp) ![](/docs/_remote/a8815581bf426ea4c90234ea.webp)

一旦我们将所有这些资源导出到 models 文件夹，就会得到 3 个模型：环境静态场景、飞船和机器人。

![](/docs/_remote/ba80d47093d413ede575a657.webp)

但我们遇到一个问题，.glb 文件中嵌入了纹理，因此如果你导出不同的资源并重复使用这些纹理，每当文件夹中出现一个新资源时，这些纹理都会被重复复制。为了避免纹理重复，我们可以使用这个实用工具。


> [!WARNING]
> ⚠️ 重要提示：在使用该工具之前，请先对你的模型进行备份，以防万一出现问题！！！


**在 Mac 上**

在将所有资源导出到 models 文件夹后，你可以将脚本文件拖到其中。

![](/docs/_remote/e18bbaa902da38ea04efdabb.webp)

1 - 将文件夹拖到终端中以在终端里打开该文件夹。

![](/docs/_remote/b110a995c9095984ab0e64a6.webp)

2 - 将 texture-extractor.sh 拖到终端中。

![](/docs/_remote/21793b724c96391277db0f59.webp)

3 - 运行命令。处理所有资源可能需要几秒钟。之后，你会看到类似这样的内容：

![](/docs/_remote/98233ac951185a1a694e7d05.webp)

4 - 进入你的 models 文件夹，会有一个名为“out”的新文件夹，你会在其中看到已提取纹理的新资源。

![](/docs/_remote/fb16a7c2d30d3624207c138e.webp)

5 - 将所有资源替换为新资源！另外，完成后请删除“texture-extractor.sh”和空的“out”文件夹。

![](/docs/_remote/98e2e796967f27259ce9c137.webp)

**在 Windows 上**

在将所有资源导出到 models 文件夹后，你可以将脚本文件拖入其中。

![](/docs/_remote/7a5b21bf0d0ceb24563c52c3.webp)

1 - 双击 glb\_texture\_extract.bat 以提取文件。如果 Windows 提示你这是无法识别的应用，请前往 **更多信息** 然后 **仍然运行**.

![](/docs/_remote/c0f9074745dc366687e508e5.webp) ![](/docs/_remote/e286200a0e0ba4a6327ffc69.webp)

2 - 脚本会生成一个名为 out 的文件夹，在那里你会找到所有带有提取纹理的新 .glb 文件。

![](/docs/_remote/c4d9063db0fa909ac05097d1.webp)

5 - 将所有资源替换为新资源。完成后，删除“texture-extractor.bat”脚本和空的“out”文件夹。

如果你遵循所有步骤，你的场景现在会快得多，而且资源将共享相同的纹理！当我们处理多个资源和大型场景时，这种提升非常明显！使用这个工具你可以节省相当多的兆字节信息！


> [!WARNING]
> ⚠️ 完成此步骤后，务必检查每张纹理是否符合指南。如果纹理不符合指南或过于庞大，请在部署前将其优化得更轻量。适合高性能场景的推荐纹理尺寸为 1024x1024。同时也要检查分辨率，超过 72 DPI 的图像会影响性能，而且不会让图像看起来更好。


## 纹理图集/ 修边贴图

纹理图集是一种单一图像文件，其中包含多个较小图像的数据并打包在一起。与其为每个网格使用一张纹理，不如让多个网格共享一张更大的纹理。

你可以在制作资源之前创建纹理图集，这意味着该资源会按照纹理图集进行 UV 展开。这在创建纹理时需要一些前期规划。

另外，你也可以在资源完成后，通过在绘画软件中合并纹理来创建纹理图集。不过这也意味着 UV 岛必须根据纹理重新排列。

下图展示了使用同一套纹理的多个 3D 对象：

![](/docs/_remote/ed4a55e913cda2f92bcb60db.webp) ![](/docs/_remote/d4e59128a0be5c8f7414ad23.webp)

另一种制作图集的方法是使用 Trim Sheets，这是一种 3D 建模和游戏开发中的常用技术，大型工作室和游戏行业会用它来确保视觉一致性和高效的艺术流程。Trim Sheets 是使用单张图像或纹理，其中包含可应用于模型不同部分的多个小细节或元素。这种技术在创建大型场景时非常有用，并且需要在美术流程的第一阶段就予以考虑。

![](/docs/_remote/6e219acc9852ed30d1a9b768.webp)

*一个使用漫反射纹理、法线和发光的 Trim Sheet 拼贴示例。*

有一个很棒的 [教程](https://www.artstation.com/blogs/jennifermcgarry/yd4Q/jenns-guide-to-trim-sheets) 的元素上， [Jennifer McGarry](https://www.artstation.com/jennifermcgarry/blog) 讲解如何使用 Blender 制作 Trim Sheets！

## 纹理命名

**正确命名纹理至关重要。** 正确的纹理命名将：

* 让你的美术流程更高效、更灵活、更有条理，并且在需要时更易于定位和修改。
* 避免使用纹理提取器或其他工具时出现同名纹理重叠。
* 避免在使用 Asset Bundles 时出现纹理重叠问题。
* 在分析场景时，更高效地定位问题点。

**如何正确命名你的纹理：**

* **资源名称** 应清楚地表示该纹理是什么。
* 纹理名称应以前缀 `T_`.
* 纹理名称应以下列定义纹理类型的后缀结尾：
  * `_D` - **漫反射/颜色贴图**
  * `_A` - **Alpha 纹理**
  * `_MT` - **金属度**
  * `_R` - **粗糙度**
  * `_N` **- 法线贴图**
  * `_E` - **发光**

示例：如果它是墙面砖纹理的漫反射贴图，名称 `T_BrickWall_D` 可能合适。如果是同一资源的法线贴图，名称可以是 `T_BrickWall_N`

**示例：**

* 🟢 **优先** 纹理名称以 - 开头 `T_Parquet_D`, `T_Floor_R`, `T_Pipes_MT`,
* 🔴 **避免** 纹理名称以 - 开头 `Image_`, `sprite_`,`未命名`

## 可选工具

有许多插件和外部工具可以在创建资源时帮助你，让流程更快更高效，其中有些是免费的，有些需要购买，举例如下：

### UVTools

* **UV Packer（免费）**:<https://www.uv-packer.com/blender/>
* **Zen UV：** <https://blendermarket.com/products/zen-uv>
* **Uvpackmaster 3：** <https://blendermarket.com/products/uvpackmaster>

### 图像压缩工具

有许多在线图像压缩工具可用于减小纹理体积。举例如下：

* **CompressPNG：** <https://compresspng.com/>
* **TinyPNG：** <https://tinypng.com/>
* **FreeConvert：** <https://www.freeconvert.com/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/textures.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
