> 向表情动作中添加道具和声音的指南。

# 道具和声音

为了将你的 Decentraland 表情动作提升到更高层次，你可以为它们添加道具（3D 几何体）和/或声音，让表情动作变得更加有趣、更具互动性！在本指南中，你会找到正确导出它们所需了解的一切！


> [!NOTE]
> **💡 提示**：安装 [Decentraland Tools Blender 插件](https://extensions.blender.org/add-ons/decentraland-tools/)。它包含一些实用功能，可帮助你编辑和导出 3D 模型、可穿戴物品和表情。


## **基础知识和限制**

要开始为你的表情动作添加道具，重要的是使用 [Decentraland 模板文件](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/BaseMale_Rig_1.0.blend) 它会包含 Avatar 的骨架，以及用于让你的作品保持在允许空间边界内的地面参考。

**目前，道具动画仅适用于骨架/骨骼动画，这意味着 \_transform 动画**\_\*\* 不允许。\*\*

带有道具的表情动作必须全部一起导出到同一个 *.glb* 文件中（Avatar\_Armature + Props\_Armature 及其动画）。

* 总大小不超过 3 MB。
* 道具总三角面数不超过 3k。
* 道具最多只能有 2 个材质和 2 张纹理。
* 道具骨架最多只能有 62 根骨骼。
* 该表情动作必须包含一段 Avatar 动画和一段道具动画。 *目前不允许多个动画。*
* 两个动画（Avatar 和道具）必须具有相同的关键帧长度。
* 动画不能超过 300 帧或 10 秒。
* 空间边界为 4 平方米。道具和粒子应保持在 Avatar 文件中提供的参考立方体内。对于 Avatar 的移动，请查看 [地面参考和动画区域](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/creating-emotes/#ground-reference-and-animation-area)。部分。

## **命名规范：**

必须严格遵循命名规范，表情动作才能正常工作！否则它们在构建器中和游戏世界里都不会正确播放。

#### 骨架命名规范：

**用于 Avatar：**

`骨架`

**用于道具：**

`Armature_Prop`

#### 动画命名规范：

**用于 Avatar：**

`AnimationName_Avatar`

* 示例： `TennisServe_Avatar`, `GunShoot_Avatar`

**用于道具：**

`AnimationName_Prop`

* 示例： `TennisServe_Prop`, `GunShoot_Prop`

## **开始使用**

在开始你的动画之前，你需要为道具创建一个骨架。如果你对这个过程不熟悉，请查看 [创建骨架](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/create-a-rig/) 以了解更多操作方法。

请确保道具对象和骨架在 Blender 中的原点都位于 0,0 位置。此外，请对道具对象和骨架应用变换，确保它们的缩放冻结为 1,1,1。这一点至关重要，可避免道具在世界中使用或在动画中播放时出现潜在问题。

#### 让道具跟随 Avatar 骨架

某些道具可能需要绑定到特定身体部位，例如网球拍绑定到手上。你可以通过简单地添加一个约束来实现。在 ***姿态模式***&#x4E2D;，选择道具骨骼（例如网球拍那根），按下 ***CTRL + Shift + C*** ，然后选择 ***子级约束*** ，或者直接点击 ***骨骼约束属性*** 选项卡，并在下拉菜单中选择 ***子级约束***.

*按下 `Ctrl + Shift + C` 即可添加约束。*

然后，在 ***目标***&#x4E2D;选择 Avatar 骨架，并在 ***骨骼*** 中选择你希望道具跟随的骨骼。要保持道具的原始位置，在添加约束后点击 ***Set Inverse*** 。如果 Influence 为 1，道具会完全跟随所选骨骼；如果为 0，则约束会被禁用。你可以在 Influence 上设置关键帧，在整个动画过程中将其打开或关闭。要做到这一点，只需将光标悬停在 ***Influence***.

***子级约束** 约束菜单上时按下 I，为 Influence 设置关键帧，以将其打开或关闭。*


> [!NOTE]
> **💡动画提示！** 如果你使用滑块来关闭 Influence，道具将不会保持其之前的位置，这会使动画难以保持流畅。为了避免手动修正位置，不要使用滑块，而是点击 Influence 旁边的 X，为它设置一个关键帧，再为所有变换属性也设置一个关键帧。这样道具就会保持与 Influence 开启时相同的位置！



> [!NOTE]
> **💡动画提示**
> 
> 不要让道具从一开始就可见！为了避免提前暴露即将发生的内容，以及避免突兀的过渡，请将动画开始时道具缩放到 0.001，并只在你希望它出现时将其设为 1。记得在动作结束时再将其缩放回 0。这样过渡会更加流畅，也会更酷！


### 动画槽位

Blender 4.4 引入了一项新功能：动画槽位。根据 Blender 文档，“槽位的目的是允许一个动作为多个数据块存储不同的动画数据”。简而言之，槽位使得在同一个 Action 中存储多个对象的动画成为可能。这会如何影响 Emotes 2.0？

Blender 4.4 新功能：动画槽位。

尽管可以让 Avatar 和道具共享同一个动作片段，但由于 Emotes 2.0 中的命名规范和涉及的动画片段数量，这样做不会生效。因此，这里的流程是：

1. 为 Avatar 创建一个动画片段，或者重命名提供的那个（***Starting\_Pose***）。它已经有一个动画槽位，但你也可以继续使用它（***Avatar\_Animation***）或者新建一个。
2. 重命名动画片段 ***AnimationName\_Avatar***
3. 为道具创建一个动画片段并将其重命名为 ***AnimationName\_Prop***
4. 点击 ***New*** 按钮为其创建一个动画槽位（它会获得一个自动名称： ***Armature\_Prop***)
5. 像在之前版本的 Blender 中那样进行动画制作。

为道具动画创建一个动作片段和一个槽位。

## **NLA 轨道**

为了导出所有动画，必须将这些片段添加到 NLA 轨道中。请确保 Avatar 只有一个动画片段，道具也只有一个动画片段， **它们必须具有完全相同的帧数。**

在 ***对象模式***，选择 Avatar 骨架，前往 ***姿态模式***，在 Browse Action 菜单中选择相应的动画片段，点击 ***动作*** ，然后点击 ***下推*** 选项。

然后，切换回 ***对象模式***，选择道具骨架，前往 ***姿势模式***，在 Browse Action 菜单中选择相应的动画片段，点击 ***动作*** ，然后点击 ***下推*** 选项。

将动作下推到 NLA 轨道。


> [!WARNING]
> ⚠️ 在下推动作时要小心。请确保选择了所需的骨架以及对应的动画。不要只是切换动画后就直接下推，而没有先选择另一个骨架，否则你会把两个动作都分配给同一个骨架，而另一个骨架则没有任何动作。


NLA 轨道应如下所示：每个骨架对应一个动画。


> [!NOTE]
> **🔥 优化提示**
> 
> **在这一步之前，请务必先备份你的项目。**
> 
> 如果你的道具有不同的对象，你可以将它们合并为一个网格。只需选中这些对象并按下快捷键 ctrl + J 即可。
> 
> 这有助于减少游戏中的绘制调用，从而让表情动作性能更好。
> 
> 不过请注意，这不适用于粒子。
> 
> *选择对象并按下 `Ctrl+J` 将它们合并在一起。*


## **导出**

Emotes 2.0 的导出方式与普通表情动作相同。请确保只有 Avatar 骨架、道具骨架和道具网格可见，并隐藏其他所有内容。

导出时仅保留 Avatar 骨架、道具骨架和道具网格可见。

要导出，请前往 File > Export > glTF2.0 (.glb, .gltf)

在导出设置中，展开 Include，并在 Limit to 中启用 Visible Objects。然后展开 Data 选项卡，再展开 Armature，并启用 Export Deformation Bones Only。

|   |   |
| - | - |

点击 Export，就完成了！

## **为表情动作添加音频**

### 音频片段的格式和限制

* 适合为表情动作导出声音的正确格式是 `.mp3` 和 `.ogg`.
* 音频片段的时长必须与表情动作相同。
* 虽然音频本身没有大小限制，但带有道具和声音的表情动作不能大于 3mb。


> [!NOTE]
> **📔 注意**：如果表情动作包含声音（mp3 或 ogg），则必须与 .glb 一起打包成 zip。之后，只需将 .zip 拖放到构建器中即可。更多细节请见： [上传带声音的表情动作](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-emotes/#uploading-emotes-using-a-zip-file)



> [!NOTE]
> **💡 注意！** 请注意，表情动作中使用的音频片段必须是原创 IP（知识产权），拥有复制权，并符合 [内容政策](https://decentraland.org/content/)标准。


### 编辑声音

你可以通过不同方式为表情动作添加声音：

1. **直接在 Blender 中编辑你的声音**

为表情动作添加声音的一种方法，是使用 Blender 提供的视频序列编辑器。

开始添加声音时，前往 *编辑器类型 > 视频序列编辑器。*

将你的声音拖放到通道界面中。

按下快捷键 `N` 即可查看更多处理声音的选项，例如显示波形、将声音设为单声道或更改音量。


> [!NOTE]
> 如果你想做淡入淡出，只需通过将音量属性的关键帧从 0 到 1 以及反向设置即可。


完成声音编辑后，你可以前往 *渲染 > 渲染音频*进行导出。在导出选项中，你需要在 `.mp3` 或 `.ogg` 中选择格式，在 *容器* 部分，然后选择 *混音导出*. **只有帧范围内的音频会被导出。**

2. **渲染动画并使用声音编辑软件添加声音**

虽然直接在 Blender 中编辑声音很方便，但它并不够灵活，因为该软件主要并不是用于音频编辑。可用工具也非常基础。如果你想让声音更专业一些，我们建议使用你选择的专门音频编辑软件。

你可以使用多种软件，例如 [Audacity](https://www.audacityteam.org/) （免费且开源）、Adobe Audition、Ableton Live 或 ProTools。使用专门的音频编辑软件将为你提供更广泛的工具、功能和音效，让你可以增强声音效果，使其更具专业感。

要渲染你的表情动作，你只需在 Blender 场景中添加一个摄像机，并将其摆放到能够尽可能清晰看到所有元素的位置，以便之后为添加声音提供良好的参考。

渲染表情动作时，重要的是只包含表情动作的帧范围，而不要更多。选择适合你需求的宽高比，并选择你想保存视频或图像序列的输出文件夹。


> [!NOTE]
> **提示！**
> 
> *在渲染前，请务必使用低采样率进行渲染，以节省渲染时间！*


完成这一步后，使用你的视频作为参考，并借助你偏好的音频编辑软件来创建相应的声音。 **确保视频序列与动画的帧率匹配，即每秒 30 帧（fps）**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/biao-qing-dong-zuo/props-and-sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
