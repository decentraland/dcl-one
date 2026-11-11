# 基础组件

World Explorer 支持一系列基础组件，这些组件与场景共享定义，并提供对定位、动画、媒体、世界状态查询的内置支持

共有五种组件类型：

1. [对象组件](#object) 为实体添加视觉和物理属性。
2. [游戏组件](#game) 可以请求动作并从游戏引擎接收信息。
3. [媒体组件](#media) 可以显示图像并播放声音。
4. [区域组件](#area-components) 会改变特定区域中实体的行为。
5. [UI 组件](#ui) 允许场景渲染悬浮界面。

除了 [`变换`](#transform)，其详细说明如下，所有组件的状态都使用协议缓冲区进行序列化。你可以点击每个标题中的链接查看完整定义。

### 对象组件 <a href="#object" id="object"></a>

最常见的组件是那些为实体附加视觉和物理属性的组件。它们可用于在游戏内对象上进行定位、调整大小、绘制以及添加碰撞。

***

#### `变换` [↗ 规范](https://adr.decentraland.org/adr/ADR-153) <a href="#transform" id="transform"></a>

为实体添加位置、旋转和缩放。

在场景生命周期中，这个组件无疑是使用最频繁、更新最频繁的。因此， `变换` 不使用协议缓冲区序列化，而是打包进自定义结构中。

`变换` 的大小为 `44` 字节，布局如下：

```
.-----------------------.-------------------------------.-----------------------.-----------------.
|   x   |   y   |   z   |   x   |   y   |   z   |   w   |   x   |   y   |   z   | parent (uint32) |
'-----------------------'-------------------------------'-----------------------'-----------------'
╵ 位置（3 个 float）╵      旋转（4 个 float）      ╵   缩放（3 个 float）    ╵

```

这种做法使运行时（尤其在底层代码环境中）能够避免中间的序列化和反序列化步骤。某个 `变换` 所在的内存可以被复制、共享并直接指针引用。

坐标可以是小数，因此使用 4 字节浮点数。

该 `parent` 字段表示该实体应相对于另一个实体进行定位，后者由其 ID 标识。

由于 `parent` 也是 [`RootEntity`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities/#rootentity) ID（`0`），因此任何 `变换` 默认都相对于它。

按照协议， `变换` 必须以大端字节序序列化。

***

#### `MeshRenderer` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_renderer.proto#L7) <a href="#meshrenderer" id="meshrenderer"></a>

为实体提供基本渲染行为。

它可以设置为渲染平面、球体、立方体或圆柱体。若要渲染任意 3D 形状，请使用 [`GltfContainer`](#gltfcontainer).

***

#### `MeshCollider` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_collider.proto#L15) <a href="#meshcollider" id="meshcollider"></a>

为实体提供基本碰撞行为和鼠标指针检测。

它可以设置为表现为平面、球体、立方体或圆柱体。若要使用自定义碰撞网格，请使用 [`GltfContainer`](#gltfcontainer).

***

#### `Material` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/material.proto#L19) <a href="#material" id="material"></a>

设置同时具有 [`MeshRenderer`](#meshrenderer) 组件的实体的纹理、光照、颜色和透明度属性。

它是一个复杂结构，旨在涵盖所有常见情况，但每个字段都有合理的默认值，可以不指定。

若要完全自定义实体的视觉属性，请使用 [`GltfContainer`](#gltfcontainer).

***

#### `GltfContainer` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/gltf_container.proto#L7) <a href="#gltfcontainer" id="gltfcontainer"></a>

在给定 [文件路径](/docs/contributor/contributor-zh/nei-rong/entities/#files) 的 `.gltf` 场景清单中的资源时，将 3D 模型附加到此实体。

由于该模型拥有自己的网格和材质，此组件会覆盖来自 [`MeshRenderer`](#meshrenderer), [`MeshCollider`](#meshcollider) 和 [`Material`](#material).

***

#### `Billboard` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/billboard.proto#L17) <a href="#billboard" id="billboard"></a>

使实体自动重新调整其 `变换` 以面向摄像机，或者可选地面向通过 `target_entity`引用的另一个实体。顾名思义，它用于显示游戏内广告牌，并常与 [`TextShape`](#textshape).

默认会影响所有方向，但可设置为只在一个轴上旋转。如果引用的目标实体不存在，则会暂停重新朝向，直到它出现为止。

***

#### `Animator` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/animator.proto#L7) <a href="#animator" id="animator"></a>

定义一个或多个可同时应用于实体的动画。

场景可以设置组件状态来定制、播放并在动画之间过渡，也可以读取它以检查哪些动画可用或当前正在播放。

***

#### `AvatarAttach` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_attach.proto#L16) <a href="#avatarattach" id="avatarattach"></a>

表示实体的位置必须跟随头像身体中的某个特定锚点。

通过设置用户 ID，它可以影响任何头像，而不只是玩家自己的。

***

#### `TextShape` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/text_shape.proto#L11) <a href="#textshape" id="textshape"></a>

在实体的位置上渲染文本，由其 `变换`.

指定。它高度可配置，支持大小、边距、内边距、颜色、阴影等参数。

***

#### `可见性` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/visibility_component.proto#L7) <a href="#visibility" id="visibility"></a>

设置实体是可见（默认）还是不可见。

不可见对象仍然存在，并且会表现出其任何其他附加组件的行为。

***

#### `AvatarShape` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_shape.proto#L8) <a href="#avatarshape" id="avatarshape"></a>

包含有关玩家头像的信息，包括其身体形状、颜色、穿戴物品和过渡状态。

此组件附加在 [头像场景](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/execution/#avatarscene).

### 游戏组件 <a href="#game" id="game"></a>

某些基础组件可供场景和运行时用于交换信息。它们利用 [ECS 同步机制](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/engine-api/#synchronization) 来保证任何状态变更或事件的一致性和顺序。

***

#### `CameraMode` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode.proto#L8) <a href="#cameramode" id="cameramode"></a>

可用于确定玩家是否使用第一人称或第三人称视角。

***

#### `PointerLock` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_lock.proto#L7) <a href="#pointerlock" id="pointerlock"></a>

可用于确定鼠标指针是否会自动跟随摄像机的焦点（锁定），或可在屏幕上自由移动（解锁）。

它附加在 [`CameraEntity`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-entities/#cameraentity)，并且其状态可从场景中读取（但不能写入）。

***

#### `PointerEvents` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events.proto#L17) <a href="#pointerevents" id="pointerevents"></a>

当指针点击实体或悬停在实体上时显示视觉反馈。

***

#### `PointerEventsResult` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events_result.proto#L13) <a href="#pointereventsresult" id="pointereventsresult"></a>

包含来自玩家的近期输入信息，包括按键和指针操作。

它附加在 `RootEntity`，并由运行时在每一帧用任何新事件进行更新。

***

#### `Raycast` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast.proto#L15) <a href="#raycast" id="raycast"></a>

可以附加到实体上，以向游戏引擎请求射线投射。 `RaycastResult` 组件稍后会附加到同一个实体上。

可以配置射线的起点、方向和最大长度。

***

#### `RaycastResult` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast_result.proto#L9) <a href="#raycastresult" id="raycastresult"></a>

由运行时附加到具有 [`Raycast`](#raycast) 组件待处理结果的实体上。

它包含原始射线的信息，并标识任何被击中的实体。

### 媒体组件 <a href="#media" id="media"></a>

场景可以附加特殊组件来显示图像、播放视频或播放声音。

***

#### `AudioSource` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_source.proto#L7) <a href="#audiosource" id="audiosource"></a>

根据 [清单中的文件路径，播放与场景捆绑的音频片段](/docs/contributor/contributor-zh/nei-rong/entities/#files).

声音来源于关联实体的位置。其音调、音量和循环行为都可设置，并且可以读取音频播放器的状态。


> [!NOTE]
> 为适应未来升级， `AudioSource` 中的字段名称为 `audio_clip_url`，但在当前协议版本中，它实际上是清单中定义的路径。


***

#### `AudioStream` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_stream.proto#L7) <a href="#audiostream" id="audiostream"></a>

与 [AudioSource](#audiosource)类似，但音频会从外部 URL 实时流式传输。

尽管附加在特定实体上，声音不受其位置影响。其音量可设置，并且可以读取音频播放器的状态。

***

#### `NftShape` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/nft_shape.proto#L36) <a href="#nftshape" id="nftshape"></a>

显示与图像或视频资源关联的 NFT。

它会渲染一个具有可配置装饰边框的 2D 画布。

### 区域组件

这些组件允许场景修改指定范围内实体的默认行为。

***

#### `AvatarModifierArea` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_modifier_area.proto#L15) <a href="#avatarmodifierarea" id="avatarmodifierarea"></a>

更改位于以实体为中心的空间内的头像行为。

它使用 3D 尺寸向量定义，并可影响头像是否可见或可点击。可根据用户 ID 排除特定头像不受此效果影响。

***

#### `CameraModeArea` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode_area.proto#L11) <a href="#cameramodearea" id="cameramodearea"></a>

更改以实体为中心的空间内的摄像机模式（第一人称或第三人称）。

它使用 3D 尺寸向量和所需的摄像机模式定义。

### UI 组件 <a href="#ui" id="ui"></a>

以下组件用于创建悬浮在游戏世界之上的图形界面。

它们通常附加到具有层级 [`UiTransform`](#uitransform) 组件的实体集合上，并通过 `parent` 属性相互关联。

例如，一个悬浮的选项窗口可以是一个带有 [`UiTransform`](#uitransform) 和一个 [`UiBackground`](#uibackground) 组件的实体，再加上一个带有子级 [`UiTransform`](#uitransform) 和一个 [`UiDropdown`](#uidropdown).

***

#### `UiTransform` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_transform.proto#L77) <a href="#uitransform" id="uitransform"></a>

描述 UI 组件的大小、位置、边距和内边距。

它基于 flexbox 模型，并且高度可定制。

***

#### `UiBackground` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_background.proto#L11) <a href="#uibackground" id="uibackground"></a>

描述在 UI 实体中用作背景的颜色或纹理。

***

#### `UiDropdown` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown.proto#L11) <a href="#uidropdown" id="uidropdown"></a>

定义将在下拉控件中显示的一组互斥选项。

***

#### `UiDropdownResult` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown_result.proto#L9) <a href="#uidropdownresult" id="uidropdownresult"></a>

包含来自 [`UiDropdown`](#uidropdown)的所选值，由运行时设置并由场景读取。

***

#### `UiInput` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input.proto#L11) <a href="#uiinput" id="uiinput"></a>

定义一个文本输入控件，并留有一定边距以便自定义。

***

#### `UiInputResult` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input_result.proto#L9) <a href="#uiinputresult" id="uiinputresult"></a>

包含 [`UiInput`](#uiinput)的所选值，由运行时设置并由场景读取。

***

#### `UiText` [↗ 源码](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_text.proto#L11) <a href="#uitext" id="uitext"></a>

定义一个简单的文本视图，并留有一定边距以便自定义。

***


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/chang-jing-yun-xing-shi/basic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
