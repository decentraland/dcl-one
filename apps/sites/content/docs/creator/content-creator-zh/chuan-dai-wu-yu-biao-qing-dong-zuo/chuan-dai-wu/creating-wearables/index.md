> 创建 Decentraland 可穿戴物的技巧和指南

# 创建可穿戴物

![](/docs/_remote/a3778599667aedc8c1fc29ed.webp)

## 简介

本指南介绍了为 Decentraland 可穿戴物创建自定义 3D 模型的基础知识。它解释了 Decentraland 头像系统的工作方式，并演示了如何正确建模你自己的可穿戴物。

*注意：本指南假设你已经具备一些从基础到中级的 3D 建模知识。如果你是 3D 建模新手，* [*从这里开始*](/docs/creator/3d-modeling/3d-models/)*.*


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物和动作。


开始之前，请下载示例文件，以便参考网格和纹理： [**可穿戴物参考模型**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)

## Decentraland 头像系统

Decentraland“头像系统”是可用自定义可穿戴物装饰的不同身体部件和子部件的广泛集合。这些部件包括：

* 身体形状
* 头部
  * 头型
  * 眉毛
  * 眼睛
  * 嘴巴
* 上半身
* 下半身
* 手部
* 脚部
* 配饰

### **基础身体形状**

下载基础头像示例文件后，将模型加载到你的 3D 编辑器中，例如 Blender。

你会注意到，每个模型都包含与骨架相关联的 8 个不同网格。这些网格分别代表头部、眉毛、眼睛、嘴巴、上半身、下半身、手和脚。你可以把这些示例模型作为你自定义可穿戴物的参考和起点。

目前有两种身体形状：A 或 B。形状 A（`AvatarShape_A.blend`）是 **女性** 身体，用于 `BaseFemale` 表示。形状 B（`AvatarShape_B.blend`）是 **男性** 身体，用于 `BaseMale`。文件名并不明显，所以在开始制作前务必再次确认：如果基于错误文件制作贴身可穿戴物，最终会导致其两种表示被颠倒。

![](/docs/_remote/15019e8948fa233ffdf5f7bb.webp)

### **头部**

![](/docs/_remote/1108d14609f042d64adb1c53.webp)

*基础头部包含可作为可穿戴物自定义的不同附加网格：眉毛、眼睛和嘴巴作为透明遮罩使用，渲染在脸部前方。*

### **上半身**

![](/docs/_remote/9ce8e60a3286071a888c6bd9.webp)

*头像的上半身，或躯干。不包含手。*

### **下半身**

![](/docs/_remote/f12c5382a0abf31077a0dfda.webp)

*下半身包括头像的骨盆和腿部。*

### **手部**

![](/docs/_remote/c61c01c445051846535b37fd.webp)

*A 和 B 形状的手相同，从头像的手腕处开始。*

### **脚部**

![](/docs/_remote/4e09b05d153e3455e46bc344.webp)

*脚部包括脚踝和脚掌。*


> [!WARNING]
> **重要：不要修改头部、上半身和下半身之间顶点的“切口/接缝”。**


身体的每个部分都有封口，使其“完全封闭”。这些封口是为了防止由于错误蒙皮权重导致动画穿模时出现难看的故障。编辑网格时最好不要移除这些封口。

![](/docs/_remote/6d31c3917522878384fcbbf7.webp)

## 为可穿戴物构建 3D 模型

#### **三角面、材质和纹理限制**

为了确保 Decentraland 能为所有玩家流畅运行，制作可穿戴模型时重要的是不要使用过多的三角面、材质和纹理。目标是在不过度牺牲细节的前提下，让 3D 模型尽可能简单，以便更容易渲染。

每个可穿戴物或配饰可使用的三角面和纹理数量都有上限：

* 每个可穿戴槽位的三角面不超过 1.5K：帽子、头盔、上半身、下半身、脚和头发。
* 每个类别的三角面不超过 500：面具、眼镜、耳环、皇冠、头顶和面部毛发。
* 对于手部配饰，预算是 1K 三角面。如果手部可穿戴物会遮住头像的基础手部，预算则为 1.5K 三角面。
* 每个可穿戴物的纹理不超过 2 张（分辨率为 1024x1024px 或更低）。所有纹理必须为正方形，分辨率为 72 像素/英寸。
* 不超过 2 个材质（不计算 AvatarSkin\_MAT）
* 对于皮肤类可穿戴物，允许的三角面数量为 5K，纹理数量为 5 张。


> [!WARNING]
> **可穿戴物三角面合并器：**
> 
> *如果可穿戴物会遮住其他可穿戴物，创作者可以将各个槽位的三角面合并。例如：如果你想做连体服，可以使用上半身类别并隐藏下半身来创建；在这种情况下你可以有 1.5K*2= 3K 三角面。\*
> 
> 如果是头盔，并且你隐藏了所有头部可穿戴物（头部、耳环、眼镜、皇冠、帽子、面部毛发、头发和头顶），你可以达到 4K 三角面、2 个材质和 2 张纹理）


#### 可穿戴物的最大宽度、高度和深度尺寸

可穿戴物有距离限制，以确保它们不会过度遮挡其他玩家的屏幕可见性，也不会以过分的方式侵占场景空间。

可穿戴物的尺寸不能超过：

> `高度：2.42 米`, `宽度：2.42 米`, `深度：1.4 米`

![](/docs/_remote/0c4ec0aad4c553469f608e3f.webp) ![](/docs/_remote/74d98bf9b871410fa2fac083.webp)

#### **贴图**

Decentraland 可穿戴物目前支持 3 种贴图，分别是：

1. **基础颜色**：这是包含模型颜色和细节的主纹理。
2. **发光**：该贴图用于表示模型中发光的部分。发光贴图应放在单独的材质中，专门用于发光。
3. **Alpha**：该贴图用于处理透明度。它使用纹理的不透明度通道。（*最好始终使用 Alpha Clip 而不是 Alpha Blend，并使用黑白纹理进行裁切*)

![](/docs/_remote/d9bad6330bf2adc9dd0f39cd.webp)

因为 Decentraland 参考客户端对头像材质使用的是 Toon Shader，所以某些贴图 **并非必需** 例如：

* **法线贴图：** 通过将表面法线编码为 RGB 值，用于在低多边形模型上模拟高分辨率细节的纹理。
* **粗糙度贴图：** 用于定义 3D 对象表面粗糙度的纹理。

要在 Decentraland 中使用这些贴图，变通方法是将它们烘焙到一张纹理中。关于纹理烘焙的更多信息请见： <https://docs.blender.org/manual/en/latest/render/cycles/baking.html>

#### **法线**

**Decentraland 引擎只渲染单面法线。** （这意味着一个平面只能从一侧可见，另一侧不会被渲染）因此，为了确保你的 3D 模型法线完全正确，我们可以通过两种不同方式进行检查。

第一种方法是在材质属性设置中切换“背面剔除”，这对于发现法线翻转很有帮助，例如下图所示：

![](/docs/_remote/0c50bdc39625df77edb130ed.webp) ![](/docs/_remote/19bd475725e7d546031c764d.webp)

检查法线是否正确的第二种方法，是在视口覆盖选项中切换“面朝向”。这会让你的模型变成蓝色，但不用担心。蓝色面是正确的，红色面则需要修正，你可以在这里找到该选项：

![](/docs/_remote/05e89b0f38d24052c2543b41.webp)

#### **骨架**

记住不要更改给定骨架的任何规格、命名约定、层级结构或变换。更改这些内容中的任何一项都会导致导出后可穿戴物在客户端中停止工作。

![](/docs/_remote/8d4e69ae0e0d37a5a75dd0ec.webp)

请确保导入的骨架没有任何 *末端* 或 *中性* 骨骼，否则该可穿戴物在导出到构建器后将无法工作。如果你导入的 .fbx 存在这个问题，可以在导入骨架时勾选 ***忽略叶骨骼*** 。

![](/docs/_remote/9bb431a3abead38036195f21.webp)


> [!NOTE]
> **弹簧骨骼：** 可穿戴物可以在基础骨架之外包含额外骨骼，用于动态物理效果（例如摆动的头发、垂坠的耳环）。这些骨骼的名称中必须包含 `springbone` 。完整详情请参见 [弹簧骨骼](/docs/creator/wearables-and-emotes/wearables/spring-bones/) 。


#### **眉毛、眼睛和嘴巴**

这些网格使用透明着色器，所以你只需为想要的新眉毛、眼睛或嘴巴样式创建自己的 png 纹理，并将其正确放入 UV 映射中即可。纹理应为 256x256px，并且需要带有用于透明度的 alpha 通道。

这里有一些示例 png 纹理：

![](/docs/_remote/5574fe5585ace1ba7b8fc724.webp) ![](/docs/_remote/65a2cf2f85f2d9c2eceaf72b.webp) ![](/docs/_remote/a7245593b361bbcdb12ba303.webp)

*眼睛和眉毛使用相同的网格和 UV 映射。*

![](/docs/_remote/769020e9eaad71f8b782e641.webp) ![](/docs/_remote/2b2a15a2fadd5b4fa790102b.webp)

*嘴巴网格和 UV 映射。*

要可视化最终结果，你需要在 Blender 中使用这些节点：

![](/docs/_remote/477f9ad97ab6c5847c84ba3a.webp) ![](/docs/_remote/7205c2f5e0ce6b631e182dbf.webp)

**遮罩：** 头像编辑器有不同的颜色选项，玩家可以选择这些选项来自定义他们的头像。

![](/docs/_remote/689357b1313432170c554166.webp)

这些颜色选择会应用到可穿戴物中的特定遮罩上。

![](/docs/_remote/859d860ec5e8ae3859344474.webp) ![](/docs/_remote/bc69b3aeb4af2645c8f66ddd.webp)

左侧图像中的黑色区域（眼睛遮罩）表示右侧纹理（眼睛基础）中将被着色的区域。请务必记住，虹膜始终需要是灰度的（如果虹膜是纯黑色，染色将无法生效。相反，如果虹膜是纯白色，那么它会被编辑器中所选颜色完全染色）。

#### **手部**

有两种 **手部** 你可以在同一类别下制作。

1. **替换手部：**

![](/docs/_remote/9affc36f5fddb68dcafbd2b3.webp)

如果你想创建能替换双手的手部可穿戴物，你可能需要覆盖 ***手部*** 基础网格。这样做后，该手部可穿戴物的限制将变为 1500 三角面。

2. **手部配饰：**

![](/docs/_remote/f9dc813cad31b498851c90f6.webp)

你也可以创建手表、手镯、戒指等手部配饰。在这种情况下，手部不需要被覆盖，每件物品的限制是 1000 三角面。


> [!WARNING]
> 注意： **手部可穿戴物** 类别用于手部配饰、手持物品（如工具或道具）以及替换双手的物品。所有这些都必须遵循头像骨架并正确蒙皮。提交不符合这些规范的物品可能会被审核委员会拒绝。


#### **头发和面部毛发**


> [!TIP]
> **提示：** 头发和其他悬挂类配饰现在可以使用 **弹簧骨骼** 来随着头像移动和重力动态运动。你可以不用静态头发，而是添加弹簧骨骼链，让马尾辫、辫子和长发自然摆动。请参见 [弹簧骨骼](/docs/creator/wearables-and-emotes/wearables/spring-bones/) 以获取完整指南。


创建自定义头发可穿戴物时，有两件重要的事需要记住。

首先，尽量遵循头部的形状。如果你需要一个起点，可以随时参考示例文件中提供的头部网格。

![](/docs/_remote/370336fb19510d9325816b5a.webp)

其次，如果你希望用户能够通过头像编辑器更改头发或面部毛发的颜色，那么你需要用灰度绘制头发，并在材质命名中使用“Hair”（例如“M\_Hair\_Short”）。如果你想包含其他不受染色影响的对象，就不要添加这种命名约定。

![](/docs/_remote/6c65a6133c0e05d828652d8e.webp) ![](/docs/_remote/cb8f9f3d94259277831a5b96.webp) ![](/docs/_remote/4b874f015973cb5a36ecc963.webp)

*较低的灰度值看起来会更暗，较高的灰度值看起来会更亮，并会与用户在头像编辑器中选择的颜色相乘。*

### **基础材质和纹理**

头像模型有三种基础材质。一种是可穿戴物本身使用的材质，另一种用于皮肤，第三种用于眉毛、眼睛和嘴巴。

![](/docs/_remote/d83fc01227d8770cbf81986b.webp)

每个基础网格都带有自己的皮肤纹理。

![](/docs/_remote/930d22b040b6df838d2d26f2.webp) ![](/docs/_remote/8dfa78f550fa2b8b6f32dfd0.webp)

皮肤纹理使用灰度制作，因此渲染引擎可以根据用户偏好，使用编辑器对头像皮肤进行染色。为了能够使用编辑器染色皮肤颜色，材质名称必须是 *AvatarSkin\_MAT*.

![](/docs/_remote/51dce26a37033db0987f4cbe.webp)


> [!WARNING]
> 重要：始终保留任何由可穿戴物暴露出来的身体部位的 UV 映射，例如短裤或裙子露出的腿部。


\src="../images/wearables-and-emotes/creating-wearables/19\_skin\_uv.png"width="600"/>

你可以为你的可穿戴物创建自定义纹理！不过，最好始终为每个可穿戴物使用单个、非常小的纹理文件。使用示例文件中提供的默认 AvatarWearable\_MAT 纹理，可以确保你的可穿戴物具有良好性能！

![](/docs/_remote/d821fd219afb4cca8fe745d3.webp)


> [!WARNING]
> 为避免触发与面部特征相关的着色器，请不要在任何网格名称中包含“\_mouth”、“\_eyebrows”或“\_eyes”。这些术语保留给面部特征，不恰当地使用它们会让着色器错误地应用到网格上。


✨ 如果你想为模型制作自己的纹理，我们建议使用以下插件来获得更好的 UV 展开：

**UVPacker**

UvPacker 是一个免费插件，只需点击几下就能帮助你打包和整理 UV。由于 Decentraland 的可穿戴物使用最高 1024x 的纹理，这个工具非常适合帮助创建更好、更有条理的纹理。

你可以在此网站直接下载：

[**https://www.uv-packer.com/download/**](https://www.uv-packer.com/download/)

**UVToolKit**

UvToolKit 是一个插件，可帮助你扩展 UV 设置和选项，以便在 Blender 中创建优秀的 UV。

下载链接如下： [**https://alexbel.gumroad.com/l/NbMya**](https://alexbel.gumroad.com/l/NbMya)

### **蒙皮权重**

蒙皮权重是确定头像骨架中的哪些骨骼会在动画期间影响哪些可穿戴物的过程。

在为新的可穿戴物设置蒙皮权重时，我们需要记住若干注意事项。

每个资产都必须对完整骨架进行权重设置。例如，应用蒙皮权重后，一个上半身资产会像这样：

![](/docs/_remote/18e214351b78092a3436da29.webp)

在身体部位交界处相接的可穿戴物必须完全权重到同一根骨骼。例如，在这两个绿色区域中，颈部的顶点必须只完全权重到“Neck”骨骼。

![](/docs/_remote/e5d072fd17af31865e1e1c4a.webp)

#### **关键骨骼**

用于蒙皮权重的“关键”骨骼有：

**头部骨骼：** 用于头发、耳环、皇冠、眼睛、眉毛、嘴巴以及任何需要跟随头部移动的配饰。

**颈部骨骼：** 用于头部和上半身的交界顶点。

**髋部骨骼：** 用于上半身和下半身的交界顶点。

**右腿和左腿骨骼：** 用于下半身和脚部的交界顶点。

**右前臂和左前臂** 用于手部交界顶点。


> [!WARNING]
> ⚠️ **提示**
> 
> * 记住，你可以用任何骨骼来影响任何网格的顶点！例如，你可以为一双高筒靴创建新的脚部网格，并将靴筒顶部的蒙皮权重设置到“腿部骨骼”。或者，你可以创建长发并使用“肩部”或“脊柱”骨骼，让头像移动时头发随之摆动。
> * 通常建议保持骨架两侧的对称性，左右两边应有相似的骨骼影响。
> * 通常来说，一个顶点不能受超过 4 根骨骼或关节影响。
> * 请记住，导出骨架时要与文档中提供的骨架完全一致。如果它包含任何其他骨骼，例如 *“\_end\_bones”* 或类似内容，它在客户端中将无法工作。


#### **刚性部件**

对于不应弯曲的部件，例如背包、喷气背包或装甲板，请将该部件的每个顶点 100% 权重到单个骨骼。例如，使用 `Avatar_Spine2` 用于背部上的任何物体。然后把该部件合并到可穿戴物的网格中，这样可穿戴物仍然保持为单一蒙皮网格。

`Avatar_Spine2` 在两种身体形状中都位于相同位置，因此权重分配给它的部件在两者上都可以使用相同位置。

背部上的刚性部件看起来可能像是漂浮着，因为背部是弯曲的，而部件本身是平的。测量该部件整个占位下方的间隙，并将其前表面放置在距离背部最远点约 4 毫米的位置。或者，在部件和背部之间添加一个小的安装块，并将安装块前侧顶点的权重设置得像它们接触到的身体顶点一样，这样安装块就会随脊柱弯曲。

#### **导出可穿戴物**

导出可穿戴物时，确保除了给定的骨架之外没有其他骨骼， **弹簧骨骼除外** （名称中包含 `springbone` 的骨骼，用于动态物理效果）。在不同软件之间导入骨架时，一个常见问题是会出现 *“\_end”* 或 *“\_neutral”* 骨骼。导出前务必将这些骨骼移除。否则，可穿戴物之后很可能无法在客户端中工作。

![](/docs/_remote/c6477cfc83e060c6483735ac.webp)

要导出可穿戴物，请先选择对象，然后选择骨架。务必不要导出其他内容，例如摄像机、灯光或空对象。

一个由单个网格组成的有效可穿戴物在其导出文件中有 64 个节点： `骨架`、其 62 根骨骼以及蒙皮网格。每根弹簧骨骼会增加一个节点。

接下来，将可穿戴物导出为 *glTF2.0* 格式。确保你只导出带有蒙皮属性的可穿戴物，不要包含任何其他不必要的功能，例如动画或形态键。

![](/docs/_remote/f71be0c8670ef1406a96e0f4.webp)

## 建模最佳实践

### **将网格从 T 姿势改为 A 姿势**

当你制作可穿戴物时，最好的做法是将模型放在 A 姿势下工作，这样既能更好地预览最终效果，也便于处理可穿戴物的拓扑和位置。为此，你需要遵循以下简单步骤：

1. 首先选择上半身，然后需要切换“**编辑模式**”和“**笼架上**”在骨架修改器中。

![](/docs/_remote/9dd9e778254d10a239d518c8.webp)

2. 现在，在姿势模式下你可以将手臂旋转 60°。

![](/docs/_remote/41758037910791f5efcffe51.webp)

3. 你可以在 A 姿势而不是 T 姿势下编辑网格。

![](/docs/_remote/71276e7651ff3507da8ee9fe.webp)

4. 但也要记住，你只需在骨架修改器中重新切换回“编辑模式”和“笼架上”，就能轻松在 A 姿势和 T 姿势之间切换。

![](/docs/_remote/a731a1c3b9056b0a3f73203e.webp)

### **关节变形：**

为了在关节部位（例如手臂或腿部）获得可穿戴物拓扑的最佳效果，在创建环线时遵循良好实践非常重要。这里我们可以看到不同环线切割对网格变形的差异：

![](/docs/_remote/c18bb25e36a973c241938296.webp)

确保一切都正确变形的一个好方法，是像下面示例那样进行权重绘制：

![](/docs/_remote/0e4eb4a84b57c9c5a3f1d079.webp) ![](/docs/_remote/a6d3efa7b12801dd5b29ed8b.webp)

### **裙子**

在建模裙子/连衣裙时，一个有用的提示和良好做法是在褶皱交汇处添加额外的环线切割。当你需要绘制骨架权重时，这会非常有帮助。

![](/docs/_remote/20d43358a97c94b9db32014e.webp)

使用这些环线切割后，顶点受影响的过渡会平滑得多，并且在为裙子/连衣裙制作动画时会得到更好的结果。

以下是骨骼影响应有的示例：

![](/docs/_remote/3a762bb515429e86217673cd.webp)

### **帽子**

创建帽子时，一个好的做法是在帽子的基础网格上添加头发，然后在编辑器中隐藏该类别 *头发* 。这样可以防止帽子与其他头发穿插，并减少意外结果。

![](/docs/_remote/52f717e012928949c63883f0.webp)

### **添加多边形数量**

一个有价值的提示是始终跟踪模型的多边形数量。为此，在 Blender 中你需要在视口覆盖面板中打开统计信息。

![](/docs/_remote/444dbdc043e94bf60c83af44.webp)

### 资源

在这个共享文件夹中，你可以找到基础模型、纹理以及其他各种资源，包括完整制作好的可穿戴物示例。制作你自己的作品时，欢迎随意利用这些资源。

[**可穿戴物参考模型**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/chuan-dai-wu/creating-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
