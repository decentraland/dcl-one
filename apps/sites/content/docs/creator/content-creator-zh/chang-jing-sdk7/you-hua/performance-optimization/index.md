> 优化你的场景，使其加载更快并为所有玩家流畅运行。

# 性能优化

你可以在场景中优化多个方面，以确保访问它们的玩家获得尽可能好的体验。本文档介绍了一些最佳实践，它们能够显著改善场景的加载速度，以及它在场景内或相邻场景中的玩家设备上运行的流畅度。

请记住，许多玩家可能会使用并非为游戏设计的硬件、通过浏览器，或者从 [移动应用](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 手机上访问 Decentraland——这些都会限制可供你的场景使用的处理能力。访问你的场景的体验应当对所有人都流畅。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tc1PwYKW1Kc" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tc1PwYKW1Kc>">Watch on YouTube</a></p>



查看 [有用资源](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/useful-resources/) 可使用一些工具，例如 Decentraland 场景优化器，它会从你场景的 3D 模型中提取、去重并压缩纹理。


> [!NOTE]
> **📱 移动端**：移动设备通常是资源最受限制的客户端。如果你的场景面向移动玩家，也请查看 [为移动端构建](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 有关移动端的专门指南。


Decentraland 浏览器在引擎层面强制执行了许多优化。这些优化效果显著，但在浏览器中同时渲染多个用户生成的体验仍然是一个很大的挑战。我们需要你的帮助来让一切顺畅运行。

## 时序

### 视频播放

播放视频是引擎最耗资源的任务之一。如果你的场景包含视频，请确保同一时间只有 *一个* VideoTexture 在使用。你可以让几十个平面共享同一个 VideoTexture，而不会对性能造成明显影响，但一旦添加第二个 VideoTexture，它对帧率的影响就会非常明显。

你还应避免让视频在无法被看到的区域中播放。例如，如果室内有一块屏幕，你可以根据玩家进出触发区域来切换视频。


> [!NOTE]
> **💡 提示**：有些场景采用的一种技巧是，流式传输单个视频，但为不同平面映射多个不同区域。每个视频屏幕都使用 [UV 映射](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/materials/#using-textures) 来只显示 VideoTexture 的一部分。借助这一点，看起来就像有多个独立视频在播放，而无需承担多个 VideoTexture 的开销。



> [!NOTE]
> **💡 提示**：当玩家站在你的场景外时，VideoTexture 不会在每一帧都更新。这有助于降低对周围场景的影响。不过，理想情况下，只有当玩家 [走进你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#player-enters-or-leaves-scene) .


### 延迟加载

如果你的场景很大，或者有并非始终可见的室内区域，你可以选择不从一开始就加载整套实体。相反，当玩家访问场景的不同部分时，再按区域加载内容。这可以显著缩短场景的加载时间，也能减少引擎在每一帧中需要处理的纹理和 3D 内容数量。

例如，博物馆的主建筑可以从一开始就加载，但每层的画作只在玩家访问该层时才加载。

参见 [这个示例场景](https://github.com/decentraland-scenes/lazy-loading) 了解这种做法可能如何运作。

为了尽量避免卡顿，最好通过切换实体形状的 `visible` 属性为 false 来隐藏实体。采用这种方式时，你在创建实体时就把它们加入引擎，但只是让它们的模型不可见。

另一种做法是在需要时才把实体加入引擎。这可能会在实体第一次出现时造成一些卡顿，而且它们也可能需要几秒钟才会可见。这种方法的优点是，它是一种绕过 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)的有效方式。请记住，场景限制计数的是任意时刻场景中正在渲染的内容，而不是所有可能被渲染的总内容。通过加载和卸载场景的部分内容，你应该能够绕开这些限制。


> [!WARNING]
> **📔 注意**：即使不可见，但已加入引擎的实体，仍会计入场景限制。


你也可以对距离较远或被遮挡的实体切换动画开关。例如，对于一个播放非常轻微待机动画的 NPC，你可以让它只在玩家距离小于 20 米时才播放该动画。可在 NPC 周围设置触发区域，并据此切换其动画开关。


> [!NOTE]
> **💡 提示**：当实体距离很远且尺寸足够小时，引擎会将其剔除。这种剔除有助于降低绘制调用层面的开销，但直接从引擎中移除实体总是更好。这种剔除也不会考虑被其他实体遮挡的情况，因此那些并不算小、但被墙挡住的实体仍然会被渲染。


### 异步块

一段 [异步代码](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/) 在等待时不会阻塞其他一切的进度：场景会继续运行，而异步块会在其等待的响应到达后恢复。请注意，场景是在单线程上运行的，因此这不是并行处理，它只是避免在等待外部响应时让场景停滞。

任何依赖异步服务响应的过程，例如 `getPlayerData()` 或 `getRealm()` 都应始终放在异步块中运行，否则它们在等待响应时会阻塞场景其余部分的加载。对任何第三方服务器的调用也同样适用。

请注意，当所有非异步内容都完成后，场景才会被视为完全加载。玩家进入场景时，异步过程可能仍在运行。应避免出现这样的情况：某个异步过程会导致某个实体加载，而该实体可能会让玩家卡在其几何体内部。

### 依赖事件

尽量让场景逻辑依赖于监听 [events](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/) ，而不是每一帧都运行检查。

该 `update()` 函数的数据存储在一个 [系统](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems/) 会在每一帧运行，理想情况下每秒 30 次。如果可以通过订阅事件来处理，就尽量避免重复检查。

例如，与其持续检查玩家的穿戴物，不如订阅 `onProfileChanged` 事件，并且只在它们发生变化时检查玩家的穿戴物。

如果必须使用系统，尽量不要在每一帧都进行检查或调整。你可以在 update 函数中加入计时器，只在每满一秒或其他合适的周期时运行一次检查。

## 优化 3D 模型

你的 3D 模型有多种方式可以优化得更轻量。

在使用该 [创作者中心](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/editor-installation/)，你可以查看场景中 3D 模型所使用资源的统计信息，以及它们是否超出了任何 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/).

![](/docs/_remote/ab87e37107598ee524831639.webp)

你可以展开此菜单查看详细信息。

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)

以下是一些改善这些指标的建议：

* 在可能的情况下，在多个 3D 模型之间共享纹理。一个好做法是使用单一纹理作为纹理集，在场景中的所有模型之间共享。与其使用多个小纹理，不如使用一张 1024x1024 像素的大共享纹理。

  > 注意：不要将同一张图片文件同时用于材质的反照率纹理、法线贴图或自发光贴图。即使内容相同，也要使用独立文件。将同一图片文件分配给不同类型的纹理属性，在压缩为资源包时可能会引入不希望出现的视觉瑕疵。
* *.glb* 是一种压缩格式，它总是会比 *.gltf*更轻。另一方面，使用 *.gltf* 时，你可以通过将纹理导出为单独文件来轻松共享纹理图像。你可以通过使用 [以下流程](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)来兼得两者的好处，它允许你拥有 *.glb* 带有外部纹理文件的模型。
* 避免使用混合透明。混合透明必须绕过相当多的渲染优化。如果可能，优先使用不透明或 alpha 测试几何体。
* 避免使用蒙皮网格。它们会显著拖慢性能。


> [!NOTE]
> **💡 提示**：有关 3D 模型最佳实践的更多内容，请阅读\[3D 建模部分]\(/creator/3d-modeling/3d-models


### 在 Blender 中挽救一个高负载模型

当统计信息指向某一个具体模型时，通常直接修复该模型比在代码中绕过它更快。在 Blender 或任何类似工具中，最有用的编辑包括：

* **减少三角形数量** 使用 Decimate 修改器。这是用于你已经拥有的模型的挽救工具。新模型最好从一开始就以低多边形方式建模。
* **调整纹理大小** 为 2 的幂次方、1024x1024 或更小，并将它们重新打包到 *.glb*.
* **删除玩家永远看不到的面**，例如道具的底面和背面。启用 [背面剔除](#backface-culling) ，而不是双倍叠加几何体。
* **将材质合并** 为一个，使用单一纹理集，而不是每个部分一个纹理。
* **从 `_collider` 网格中删除灯光、相机和材质。** 引擎会忽略它们，所以它们纯属负担。

在前后检查三角形数量和材质数量，然后覆盖导出回同一个文件。如果预览正在运行，它会使用你的更改重新加载。

### 多次重复使用同一个模型

场景中往往充满重复内容：街道上的路灯、房间里的椅子、公园里的树。构建这些内容的最佳方式也是最简单的—— **给每个副本各自一个实体，并让它们都指向同一个&#x20;*****.glb*****&#x20;文件。**

```ts
// 良好：一个文件，多个实体
for (const position of lampPostPositions) {
  const lampPost = engine.addEntity()
  Transform.create(lampPost, { position })
  GltfContainer.create(lampPost, { src: 'assets/scene/lampPost.glb' })
}
```

引擎会识别这些实体共享同一个来源。该文件只会下载一次，只会转换为资源包一次，并且它的网格和纹理只会在内存中保留一份——第 20 个路灯几乎只额外消耗它在世界中的位置成本。这在不同场景之间也同样适用：如果相邻场景使用相同文件，它已经在内存中了。导出二十个几乎相同的 *.glb* 文件则意味着二十次下载和二十份内存副本。

这 **不会** 节省的是绘制调用。二十个路灯就是二十个要绘制的对象，无论它们来自二十个实体，还是来自一个 *.glb* 其中建有二十个路灯的模型。减少这一数量的唯一方法，是在你的建模工具中将网格合并，这确实是一种权衡——请参见 [实例化 vs 复制 vs 合并](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/meshes/#instancing-objects-vs-duplicating-objects) ，了解何时值得这样做。

还有一些相关建议：

* **分组为多个簇，而不是一个巨大的模型。** 如果你有很多零散道具，一个不错的折中方案是每组一个 *.glb* ——一段街区、一间房里的家具——而不是每个道具一个模型，或者整个场景只用一个模型。这样既减少了对象数量，又能让每个簇足够小，使剔除仍然能发挥作用。
* **一次生成很多副本？** 先使用 [`AssetLoad` 组件](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/pre-load-resources/)预加载模型。这样副本就可以从已经在内存中的模型创建，而不是每个副本都等待同一个下载。创建每个副本本身仍然需要工作，所以一次大批量生成无论如何都可能吃掉一帧——预加载只是消除了等待文件的时间，而不是创建副本的成本。
* **一个非常大的模型比许多小模型更耗引擎资源。** 引擎会把构建场景的工作分散到多帧中，以避免卡顿，但它无法拆分单个超大模型——那会落在同一帧中。许多较小的部分会平滑加载；一个整体巨大的部分则更可能造成明显卡顿。
* **重复使用一个模型不会引入新的纹理或几何体。** 由一个模型构建的二十个路灯会共享该模型的网格和纹理，而不是为集合增加新内容——第 20 个不会增加纹理内存。材质是个例外：为了应用你场景的边界裁剪，引擎会为每个渲染对象创建一个材质实例，因此材质数量会随着你渲染的对象数量而变化，而不是随着你使用的模型数量而变化。这些实例仍然共享相同的纹理和着色器变体，所以它们只会占用少量内存，而不是帧时间。会把场景推向纹理 [限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 的是拥有很多 *不同的* 模型，而且每个模型都有自己定制的一套内容——因此围绕更小的复用模型库来整合场景，会有所帮助。

### 背面剔除

为了性能优化，背面剔除将被设置为 **开启** 在 **全部** 模型的材质在引擎中渲染后，无论其设置如何。

如果你希望看到模型的背面或内部，请复制面并反转法线。

#### 故障排除

要验证某个场景是否存在材质背面剔除问题，请按以下步骤操作：

1. 打开场景中的 `调试` 面板。

* 如果场景已发布，请输入 `/debug` 命令到聊天框中。
* 如果你处于场景预览模式，请点击位于屏幕右上角的故障虫图标（

  <img src="/docs/_remote/3a2d6a1437dc36383f2065db.webp" alt="调试图标" width="32">

  ）。

2. 调试面板会出现在屏幕右下角。
3. 在 **当前场景**下，点击 **背面调试器** 按钮。

![](/docs/_remote/1e0c76604bd561268516ee00.webp)

4. 切换 **强制背面剔除**：它显示的是开启背面剔除时渲染的材质。这就是优化在正式环境中的实际渲染效果。通过切换开/关，可以找出需要修复的材质。
5. 切换 **背面调试器** ，可以轻松找出关闭背面剔除的材质。它会高亮显示：

* **红色**：未将背面剔除设置为 **开启**.
* **绿色**：开启背面剔除的材质 **开启**.

<p align="center"><img src="/docs/_remote/0c934b5889f4f369d893365d.webp" alt=""> <img src="/docs/_remote/d2530a38414758e20fb9ebd0.webp" alt=""><br><em>关闭背面剔除的材质（左）与开启背面剔除的材质（右）</em></p>

### 资源包转换

每次发布场景时，Decentraland 内容服务器都会将其中的每一个 *.gltf* 和 *.glb* 模型压缩为资源包格式。这种格式 *显著* 更轻，因此场景加载更快、运行更流畅。转换会在每次部署后立即开始。通常只需几秒钟，但对于非常大的场景，或者服务器繁忙时，可能需要更长时间，因为转换会针对每个平台分别运行，并可能排在其他场景之后等待。你可以 [检查转换状态](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/#check-the-conversion-status) ，直接查看你场景的状态。

你也可以在开发场景时在本地运行同样的转换。这会让你的预览更流畅，并让你在发布前检查压缩模型是否存在任何问题。参见 [使用优化资源的预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 提示**：在 Decentraland 中筹备活动时，请至少提前一小时发布最终版本，以便在此之前所有模型都已转换为资源包，并留出时间修复任何意外问题。如果你不想在活动前泄露惊喜，你可以部署一个包含项目文件夹中所有最终 3D 模型的场景版本，但这些模型不可见，或者它们的大小被设置为 0。



> [!WARNING]
> **📔 注意**：如果你对一个 3D 模型文件做出 *任何* 改动，即使只是更改名称，它也会被视为一个新文件，并且必须再次转换为资源包格式。


## 连接性

如果你的场景会连接到任何第三方服务器，或者使用 [messagebus](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/#send-explicit-messagebus-messages) 在玩家之间发送消息，你也需要记住一些事项。

* 你的场景同一时间只能有一个活动的 WebSockets 连接。
* HTTP 调用由引擎统一处理，因此同一时间只会处理一个。任何额外请求都会在内部排队，并必须等待其他请求完成。这个排队过程会自动处理，你不需要做任何事情。
* 当使用 [messagebus](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/serverless-multiplayer/#send-explicit-messagebus-messages) 在玩家之间发送消息时，请注意所有消息都会发送给服务器岛中的所有其他玩家。避免出现这样的情况：一条传入消息直接触发发送另一条消息，因为当场景里人多时，消息数量可能会迅速呈指数增长。

## 场景 UI

当场景 UI 由许多独立元素组成时，渲染成本会变得很高。请记住，每个 UI 元素都需要引擎单独进行一次绘制调用。


> [!NOTE]
> **💡 提示**：尽量把多个元素合并成一张图片。例如，如果你有一个包含多个文本元素的菜单，最好把磁贴中的文本以及任何额外图片都烘焙到背景图中。这样可以避免引擎为每个文本元素在每一帧额外执行一次绘制调用。


避免在每一帧都调整 UI，这样做尤其耗性能，而且最终可能会被排队。例如，如果你的 UI 中有一个应该随时间缩小的生命值条，玩家可能不会注意到它以 10 FPS 更新而不是 30 FPS（每帧一次）之间的差别。更新这个条的系统可以使用一个计时 100 毫秒的短计时器，只在计时器归零时才影响 UI。

避免存在大量隐藏的 UI 元素，即使它们未被渲染，也会影响性能。只要可能，尽量按需创建 UI 组件。

## Worlds 中的地形景观

发布到 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 会被自动生成的草地、树木和海洋所环绕。渲染这些景观会消耗玩家渲染预算的一部分。如果你的场景不需要它，你可以 [禁用地形景观](/docs/creator/content-creator-zh/chang-jing-sdk7/xiang-mu-lei-xing/scene-metadata/#landscape-terrain) 在你的 `scene.json` 中，以便把这些资源释放给你场景自己的内容。

## 监控性能

了解场景运行状况的最佳指标是 FPS（每秒帧数）。在预览中，你可以在调试面板里看到当前场景的 FPS。你的目标应始终保持 30 FPS 或更高。

在已部署的场景中，你可以通过输入 `/showfps` 到聊天窗口来切换显示这些指标的面板。

### 场景统计面板

在桌面客户端的预览中运行场景时，你可以打开一个显示场景内容实时统计信息的面板。点击屏幕右上角、控制台和调试图标旁边的统计图标。

<img src="/docs/_remote/96e07557afec5b2ade6528d5.webp" alt="场景统计面板" width="400">

该面板会将场景的三角形、实体、刚体和纹理与各自的限制进行对比，并显示每项预算的使用百分比。某项数值一旦达到其限制的 80%，就会变为橙色。它还会显示材质、几何体、碰撞体以及外部视频或音频的纯计数。将鼠标悬停在每个指标旁的信息图标上，可查看它统计什么以及为何重要的说明。

这些数字会随着内容的加载和卸载实时更新，这使得该面板非常适合检查诸如 [延迟加载](#lazy-loading) 这类策略会如何影响引擎在任意时刻实际处理的内容。有关每个限制的详细信息，请参见 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/) 。请记住，这些都是软限制：超出其中一项不会阻止你的场景运行，但可能会降低玩家的性能。

### 场景与引擎之间的消息

场景性能的主要瓶颈之一通常是场景代码与引擎之间的消息传递。

当你在预览中运行场景时，请注意右上角会显示“Y = Toggle Panel”。按键盘上的 Y 可打开一个面板，其中包含一些会实时更新的有用信息。

当你与涉及 SDK 和引擎之间消息的内容交互时，你会注意到“Processed Messages”数字在增长。你应密切关注“Pending on Queue”数字，它应该始终为 0 或接近 0。这表示有多少此类消息尚未被处理，而是被推入了队列。如果“Pending on Queue”的数量开始增长，那么你已经进入危险区，应该考虑对场景进行更多优化。


> [!WARNING]
> **📔 注意**：不要在不使用它时一直打开该面板，因为它会对性能产生负面影响。


请记住，你在预览中体验到的性能可能与正式环境不同：

* 周围相邻场景可能会产生负面影响
* 场景的 3D 模型压缩为资源包可能会产生正面影响
* 访问你场景的部分玩家可能正在使用性能较弱的硬件

通常建议先将你的场景部署到一个 [Decentraland World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) 进行更彻底的测试。

一定要向玩家征求反馈。不要想当然地认为你体验场景的方式与其他人相同。

## 使用 AI 优化

AI 代理可以为你处理本页所述的大部分测量和修复工作。Decentraland 提供 [AI 技能](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#install-skills-for-any-ai-agent) ，用于教会编码代理（如 Claude Code 或 Cursor）如何优化场景，而桌面客户端还包含一个 [MCP 服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/#let-the-ai-see-your-scene-in-world) ，可让代理直接检查并测量你正在运行的场景。

借助 `optimize-scene` 和 `unity-explorer-mcp` 已安装的技能并连接 MCP 服务器后，代理可以：

* **检查场景是否符合内容预算**：它会读取与场景统计面板相同的实时数据，并将其与 [场景限制](/docs/creator/content-creator-zh/chang-jing-sdk7/you-hua/scene-limitations/)进行比较，因此它可以告诉你是哪项预算即将吃紧。
* **找出责任资产**：它会按每个 3D 模型贡献的三角形、材质和绘制调用数量对场景中的模型进行排序，包括从特定视角可见的部分，因此它能精确知道应优先优化哪些模型。
* **测量真实帧率**：它可以把玩家放置在场景中的任意位置，并采样玩家在那里实际体验到的 FPS，包括瞬时卡顿。
* **验证自己的工作**：每次更改后它都可以重新测量，因此它的建议和修复都有真实数据支撑，而不是猜测。

这会把优化变成你只需直接提出请求的事情：

> “检查我的场景是否在限制内，找出在出生点附近拖慢帧率的任何因素，修复它，并向我展示修复前后的数据。”

### 结合 Blender MCP 服务器

Decentraland 工具会告诉代理哪些模型最重，但它们不会直接编辑模型本身。要更进一步，你可以尝试将它们与 [Blender MCP 服务器](https://www.blender.org/lab/mcp-server/)结合使用，它让代理可以访问 Blender 的编辑工具。这样代理就可以在 Blender 中打开一个高负载模型，降低其三角形数量，合并其材质，或缩小其纹理，并将结果导出回场景中作为一个 `.glb` 文件，然后它就可以在世界中重新测量帧率。

请记住，自动化网格编辑可能会明显改变模型的外观，因此一定要检查结果，并保留原始文件的备份。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/you-hua/performance-optimization.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
