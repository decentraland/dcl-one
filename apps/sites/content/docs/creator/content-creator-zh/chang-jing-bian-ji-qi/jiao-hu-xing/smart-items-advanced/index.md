> 在场景中使用智能物品来增加交互性。

# 智能物品 - 高级

大多数智能物品都有一个基础模块，你可以用简单的方式配置最常见的设置，但你可以继续向下滚动，越过 **高级** 标记，以自定义该物品行为的几乎任何内容。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/m_xWCSDDxpQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=m_xWCSDDxpQ>">Watch on YouTube</a></p>



下面这个物品有一个 Transform 组件和一个基础模块，只显示用于配置按钮的基本字段。但如果你继续向下滚动，越过 **高级** 标记，你会找到所有可用的设置。

![](/docs/_remote/6f2502ce1e009c232a69e7cb.webp)


> [!NOTE]
> **📔 注意**：基础模块中的大多数设置也可在下面的组件中使用。基础模块中的更改会反映到下面的组件中，反之亦然，除了一些情况：基础设置是对下面多个设置的抽象。在这些情况下，将高级设置更改为基础模块不支持的值，会导致基础模块中的字段被标记为未定义。


## 高级配置

属性被分组到 [**组件**](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/)中。不同的智能物品可能有不同的组件，这取决于它们的功能。

大多数物品的行为由以下部分控制：

* [**动作**](#actions)：Actions 组件定义了物品可以执行的操作。例如播放声音、播放动画、向上移动或变为不可见。
* [**触发器**](#triggers)：Triggers 组件指定哪些事件会使这些操作发生。例如当玩家点击物品时、当玩家走入某个区域时，或当场景首次加载时。

例如，在一个门智能物品中， **动作** 组件包含“Open”和“Close”操作。 **触发器** 该物品中的 **On Click** 触发器会在玩家点击门时激活“Open”操作。

智能物品的触发器可以激活场景中任何智能物品上的操作，不仅限于同一个智能物品。例如，一个按钮智能物品可以有一个 **触发器** 组件，用于激活悬浮平台的 **动作** 组件中定义的“move up”操作。

触发器也可以有条件地发生。例如，门智能物品包含两个 **On Click** 触发器在其 Triggers 组件中：一个在门关闭时打开门，另一个在门打开时关闭门。更多详情请参见 [状态与条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/).

## 物品之间的交互

要让物品彼此交互：

* 一个物品需要至少在一个 [动作](#actions) 组件。
* 另一个物品需要在 [触发器](#triggers) 组件中有一个触发器，指向该操作。

例如，要让一个按钮打开一扇门：

1. 添加任意一个按钮智能物品，打开它的 **触发器** 组件。它有一个默认的触发事件，会为按钮本身播放声音和动画。
2. 点击 **+** “+”号旁边的 **已分配的操作**，以便为同一个触发事件添加第三个操作。
3. 在第一个下拉菜单中选择门的智能物品。
4. 在第二个下拉菜单中选择“Open”操作。

![](/docs/_remote/564f4fe0ae016962310472b6.webp)


> [!NOTE]
> **💡 提示**：你也可以改为创建一个只处理门操作的新触发事件。这两个触发事件都会在每次按钮被点击时调用。
> 
> <img src="/docs/_remote/4c4d3311baeb14d82a595558.webp" alt="" data-size="original">


任何物品都可以触发任何其他物品的任何操作，只要该操作已定义。参见 [触发器](#triggers) 了解操作可被触发的更多方式。

你可以使用 [状态和条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/) ，仅在条件满足时才触发操作。该条件甚至可以检查第三个智能物品的状态。例如，按钮只有在自定义的“power generator”智能物品的状态设为“On”时才打开门。

## 动作

该 **动作** 组件列出物品可以执行的操作。每个智能物品都包含一组预定义操作。你可以自定义现有操作或添加新操作。可用的操作类型如下：

* **播放动画**：在物品的 3D 模型中播放一个动画。参见 [关于播放动画](#about-playing-animations)
* **停止动画**：停止该物品的 3D 模型正在播放的所有动画。
* **播放声音**：在物品所在位置播放一个声音文件。参见 [关于播放声音](#about-playing-sounds)
* **停止声音**：停止该物品播放的所有声音。
* **开始补间**：在给定时间内逐渐改变位置、旋转或缩放。参见 [移动、旋转或缩放](#moving-rotating-or-scaling).
* **设置可见性**：使物品可见或不可见。
* **附加到玩家**：将该物品设为玩家头像的子对象。例如可以把它拿在手上或举过头顶。
* **从玩家处分离**：将该物品从玩家头像上分离。
* **打开链接**：在浏览器标签页中打开一个外部网站链接。打开前会询问玩家是否信任该域名。


> [!NOTE]
> **📔 注意**：此操作只能通过点击物品来触发，不能通过走入触发区域来触发。


* **移动玩家**：将玩家的位置更改为场景内的一组本地坐标。只能在同一场景内移动玩家。
* **传送玩家**：将玩家传送到 Decentraland 中的另一个位置。使用 **传送模式** 下拉菜单可以将其发送到 **坐标** ，即 Genesis City 中另一场景的 **到世界**，表示某个 World 的名称。玩家将出现在目标场景的出生点。
* **播放表情**：让玩家的头像执行默认头像动画之一（例如：挥手或鼓掌）。
* **播放自定义表情**：让玩家的头像播放一个自定义动画，来自上传到场景中的文件。
* **显示文本**：在屏幕的 UI 上显示文本，几秒后隐藏。适合作为提示、对话、通知等。
* **隐藏文本**：隐藏当前可能显示的任何 UI 文本。
* **开始延迟**：将同一物品的另一个操作延迟所需的秒数。
* **停止延迟**：取消该物品上的任何延迟操作。
* **开始循环**：以给定间隔反复重新播放同一物品的一个操作。
* **停止循环**：取消该物品上的任何循环操作。
* **播放视频**：将视频作为材质播放到一个基础几何体上。
* **停止视频**：停止当前播放的任何视频。
* **播放音频流**：播放音频流。
* **停止音频流**：停止当前正在播放的任何音频流。
* **克隆**：在指定位置复制一个物品。
* **生成实体**：在场景中创建该物品的新副本，位置相对于执行该操作的物品。要生成的物品不需要先放置在场景中。参见 [关于生成实体](#about-spawning-entities).
* **移除**：从场景中删除一个物品。
* **显示图像**：在 UI 上显示一张图片，可设置为仅显示有限时间。也可以包含说明文字。
* **隐藏图像**：隐藏通过“显示图像”操作当前显示的任何图片。
* **造成伤害**：减少附近任何生命条的生命值。 *层* 属性可以决定它是仅作用于玩家上的生命条，还是作用于其他物品。
* **将玩家移动到此处**：将玩家的位置改为此物品的位置。
* **放置到玩家身上**：将物品的位置改为玩家的位置。
* **按玩家旋转**：将物品的旋转改为玩家的旋转。
* **放置到摄像机上**：将物品的位置改为摄像机的位置。
* **按摄像机旋转**：将物品的旋转改为摄像机的旋转。
* **设置位置**：将物品的位置改为特定位置。可以是绝对位置，也可以相对于当前位置。
* **设置旋转**：将物品的旋转改为特定旋转。可以是绝对的，也可以相对于当前旋转。
* **设置缩放**：将物品的缩放改为特定值。可以是绝对值，也可以相对于当前缩放。
* **跟随玩家**：开始朝玩家位置移动并转向。它会忽略沿途的任何障碍。你可以设置速度，并让它只在某些轴向上移动。最小距离决定它会接近玩家到什么程度。
* **停止跟随玩家**：停止“跟随玩家”操作。
* **随机操作**：每次调用随机操作时，下面列出的某个操作都会以相同概率随机播放。你可以列出属于该物品的任何操作。
* **批量操作**：每次调用批量操作时，这里列出的所有操作都会同时播放。你可以列出属于该物品的任何操作。
* **治疗玩家**：恢复玩家生命条的生命值。
* **玩家面向物品**：让玩家的头像转向并面向该物品。
* **冻结玩家**：阻止玩家移动、跳跃或做表情。
* **解除冻结玩家**：在“冻结玩家”操作后恢复玩家移动能力。
* **开灯**：打开物品的 **光源** 组件。
* **关灯**：关闭物品的 **光源** 组件。
* **修改灯光**：更改物品 **光源** 组件的属性，例如颜色或强度。
* **选择摄像机**：将玩家视角切换到 **虚拟摄像机**，可以是此物品上的，也可以是另一个实体上的。
* **更改文本**：更改带有 **文本形状** 组件的物品文本。也可以更改字体大小和颜色。
* **停止补间**：停止当前正在进行的任何该物品补间移动。
* **滑动纹理**：让物品材质上的纹理持续朝给定方向滑动，可用于流水或传送带等效果。
* **更改碰撞**：启用或禁用该物品的碰撞器，既用于物理也用于指针事件。
* **更改天空盒**：将场景的天空盒更改为一天中的固定时间，以从午夜开始的秒数表示。
* **重置天空盒**：将场景的天空盒恢复为默认设置。
* **记录到控制台**：向控制台打印一条消息，便于在开发场景时调试。
* **删除**：从场景中移除该物品及其所有子对象。

参见 [状态和条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/) 以了解与逻辑条件相关的其他操作，例如 **设置状态** 和计数器操作。

该 **动作** 组件定义了可能的操作，但除非被触发，否则它们在场景中不会起作用。操作由一个 [触发器](#triggers)激活，可以来自同一个智能物品，也可以来自另一个物品。

要向物品添加新操作，请点击 **添加新操作** 按钮，位于 Action 组件底部。然后给操作命名，选择类型，并填写该类型操作所需的任何附加字段。

![](/docs/_remote/af4f04d249dfe05512b5fb47.webp)

### 触发器

该 **触发器** 组件定义触发事件，这些事件会在特定事件发生时激活操作。存在以下类型的触发事件：

* **On Click**：当玩家点击物品时。参见 [关于点击触发器](#about-click-triggers)
* **输入操作时触发**：当玩家在指向物品时按下输入按钮。
* **全局点击时**：当玩家在任何地方点击指针时，无需指向该物品。
* **全局主操作时**：当玩家在任何地方按下主键（E）时。
* **全局次操作时**：当玩家在任何地方按下次键（F）时。
* **玩家进入区域时**：当玩家进入某个区域时。参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area/)
* **玩家离开区域时**：当玩家离开某个区域时。参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area/)
* **出生时**：当场景开始时，或该物品在场景中生成时。参见 [出生触发器](#trigger-on-spawn)
* **延迟时**：当 **开始延迟** 该物品上的一个操作完成倒计时后。
* **循环时**：在该物品上的一个 **开始循环** 操作的每次迭代时。
* **克隆时**：当该物品通过一个 **克隆** 操作被克隆时，新副本会触发此触发器。
* **点击图像时**：当玩家点击由一个 **显示图像** 操作显示的图像时。
* **受伤时**：当该物品受到一个 **造成伤害** 操作显示的图像时。
* **当玩家被治疗时**：当玩家被一个 **治疗玩家** 操作显示的图像时。
* **每帧时**：在场景的每个 tick 中，每帧一次。请谨慎使用，因为它触发的操作会非常频繁地运行。

参见 [状态和条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/) 以了解与逻辑条件相关的其他触发器，例如 **状态改变时**, **计数器变化时** 和 **补间结束时**.

要添加新的触发器，请点击 **添加新触发事件** ，位于 Trigger 组件底部。然后选择触发器类型、你要激活的实体，以及该实体中的一个操作。

![](/docs/_remote/4c0c7ee0b8ac3b2401bebd31.webp)


> [!NOTE]
> **📔 注意**：在你触发它之前，需要先在 [动作](#actions) 实体的组件中定义一个操作。触发器只能影响具有 Actions 组件的实体。


## 关于播放动画

使用类型为 **播放动画** 的操作来在智能物品的 3D 模型上运行动画。该动画需要已经作为 3D 模型文件的一部分存在。 **选择动画** 下拉菜单会显示 3D 模型中所有可用动画的列表。

该 **播放模式** 字段可让你选择动画是只播放一次，还是持续循环。

![](/docs/_remote/b62f8f6fe8ab5cf4a5ed6eba.webp)

一旦操作创建完成，你就可以通过 [触发器](#triggers) 该同一物品或任何其他物品的组件来激活它。

使用 **停止动画** 操作可停止该物品的所有动画，包括循环和非循环动画。


> [!NOTE]
> **💡 提示**：要轻松检查 3D 模型的内容，查看它包含哪些动画以及它们的效果，一个很好的工具是 [Babylon Sandbox](https://sandbox.babylonjs.com/)。只需将 3D 模型文件拖入窗口即可。底部应该会出现一个带有其动画列表的下拉菜单。


要进一步了解动画以及如何在 3D 模型中创建自己的动画，请参见 [动画](/docs/creator/content-creator-zh/3d-jian-mo-yu-dong-hua/animations/).

## 关于播放声音

使用类型为 **播放声音** 来播放声音文件。只要声音文件已导入场景项目，你就可以播放任何声音文件。声音会按位置播放，来自物品所在的位置，这意味着玩家越靠近，声音越大。


> [!NOTE]
> **💡 提示**：你可以不用输入声音文件的路径，而是将它拖入 **路径** 字段，从 Scene Editor 底部的文件导航菜单中。


使用 **播放模式** 字段中选择声音是播放一次，还是持续循环。

![](/docs/_remote/eae4449cca4301e0122d9583.webp)

一旦操作创建完成，你就可以通过 [触发器](#triggers) 该同一物品或任何其他物品的组件来激活它。

使用 **停止声音** 操作可停止该物品播放的所有声音，包括循环和非循环声音。这也会停止来自 **AudioSource** 组件。

组件的声音。若你需要两个声音同时播放，例如环境音或音乐，更方便的做法是使用 **AudioSource** 组件，而不是使用 Actions 和 Triggers。此组件只需要你提供一个文件路径，并勾选 **开始播放** 和 **循环**.

![](/docs/_remote/efba578789bf705bd7ec2955.webp)


> [!NOTE]
> **📔 注意**：智能物品一次只能播放一个声音。调用第二个声音会中断当前正在播放的任何其他声音。这同样适用于 **AudioSource** 组件的声音。如果你需要两个声音同时播放，可以考虑在同一位置添加一个不可见实体来承载一个 **播放声音** 操作显示的图像时。


参见 [声音](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/sounds/) ，以了解在 Decentraland 中播放声音的更多信息。

## 移动、旋转或缩放

使用一个 **开始补间** 操作来更改物品的 **位置**, **缩放**，或 **旋转**，在一段时间内完成。所有 **开始补间** 操作都从物品的原始状态开始，并在一段时间内变为结束状态。

位置补间可以是相对的或绝对的。绝对位置补间会将物品移动到相对于场景的固定位置。物品会从当前所在位置移动到该位置。如果它已经在那里，就看起来不会移动。相对位置补间会将物品从当前位置移动一定距离，例如相对位置补间到 `1, 0, 0` 会使物品沿当前朝向向前移动 1 米。如果你第二次运行该补间操作，物品将再向前移动 1 米。

旋转补间也可以是相对的或绝对的。相对旋转会叠加到物品当前的旋转上。绝对旋转补间会让物品面向某个特定方向，相对于场景。

使用 **持续时间** 字段用于设置整个移动应该耗时多久，以秒为单位。请注意，滑块最多到 100 秒，但如果需要，你也可以手动输入更大的数字。

![](/docs/_remote/04196395f61a96c50fbd476c.webp)

一旦操作创建完成，你就可以通过 [触发器](#triggers) 该同一物品或任何其他物品的组件来激活它。

补间可以遵循不同的 **曲线类型** ，它们会影响随时间变化的速率。 **线性** 曲线（默认）表示变化速度从头到尾保持恒定。可选项很多，它们会根据起始和/或结束是慢启动以及程度不同而绘制出不同形状的曲线。 **easeinexpo** 曲线开始慢、结束快，速度呈指数增长；相反， **easeoutexpo** 曲线开始快、结束慢。

![](/docs/_remote/8451a60c562d729a69cf2138.webp)


> [!NOTE]
> **💡 提示**：尝试不同的运动曲线。差异通常很细微，但我们会潜意识地从事物的运动方式中解读信息，比如重量、摩擦力，甚至是个性。


使用 **补间结束时** 触发器事件在 **触发器** 组件中，用于在补间完成后激活一个操作。使用 [状态和条件逻辑](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/) 来描述悬浮平台的循环路径，使它不断在两个位置之间移动。

当物品执行补间时，这会影响物品的方方面面。例如，如果它改变缩放，它会同时改变其可见的 3D 模型、不可见的碰撞体几何体、文本大小等。如果该物品有任何子对象（在左侧实体树中嵌套），这些子实体也会受到补间的影响。


> [!NOTE]
> **📔 注意**：每个实体一次只能执行一个补间。例如，你不能让一个物品同时侧向移动和旋转。作为替代方案，你可以使用父子实体。例如，你可以有一个不可见的父实体向侧面移动，而一个可见的子实体进行旋转。


## 关于生成实体

使用一个 **生成实体** 操作，在运行时在场景中创建一个物品的新副本。不同于 **克隆** 操作，你生成的物品不需要先放置在场景中。

### 将物品设为可生成

在物品可以被生成之前，它必须先存在于项目文件中。要在不将物品放入场景的情况下添加它：

1. 在 **资源** 面板中，在目录或你的 **自定义项目**.
2. 右键单击该物品并选择 **添加到文件系统**.

这会将物品复制到你的项目中，以便以后可以生成。它不会向场景添加任何可见内容。然后该物品会出现在 **源** 下拉菜单中，适用于任何 **生成实体** 操作显示的图像时。

### 配置操作

一个 **生成实体** 操作有两个字段：

* **源**：要生成的项目。从已添加到你项目文件中的项目里选择。
* **位置**：新副本出现时的 X、Y 和 Z 坐标，相对于执行操作的项目。

如果你生成的项目本身也是智能项目，那么每个生成的副本都会保留自己的操作和触发器，并独立运行。


> [!NOTE]
> **💡 提示**：若要改为通过代码生成项目，请参见 [复合体](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/composites/)。添加到你项目文件中的项目会以复合体格式存储，这与 SDK 的 spawn 函数使用的格式相同。


## 关于点击触发器

若要通过点击某个项目来触发操作，请创建一个 **On Click** 触发器。每次玩家点击该实体时，都会激活该操作。

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

参见 [让任意项目变成智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/#interactivity) 。


> [!NOTE]
> **📔 注意**：使用自定义 3D 模型时，模型必须具有一个不可见的碰撞体几何体，才能被点击。参见 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/#pointer-blocking).
> 
> 或者，你可以配置该 **GLTF** 项目的组件，使其 **可见层** 的碰撞设置为 **指针**.
> 
> 另一种方法是添加一个 **点击区域** 智能项目，用来绘制一个与要点击的项目重叠的立方体。点击区域智能项目是一种 [不可见项目](#invisible-items).


## 出生触发器

类型为 **出生时** 的触发器会在场景加载时激活一个操作。它不会等待玩家与项目交互，而是会立即运行该操作。

例如，可用它让平台持续移动。使用一个 **出生时** 触发器来激活动画补间操作。然后使用 **状态改变时** 触发器让它在两个或更多位置之间持续移动。

![](/docs/_remote/b027f05ed8b8aee085690f4b.webp)

## 多人模式

所有智能项目默认都是多人模式。参见 [智能项目 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/) 。

你可以更改或微调这种多人行为，只同步项目的某些组件。

在该项目的 **多人模式** 组件中，勾选你想共享的组件。

例如，门会共享它的 `动画器` ，这样所有人都能看到开门动画；它的 `AudioSource` ，这样所有人都能听到它的声音；以及它的 `状态` ，这样所有人都能跟踪它当前是打开还是关闭。门不会共享它的 `可见性` 组件，因为门通常始终可见。如果你包含了切换其可见性的操作，你也许还想勾选这个组件，以便在所有玩家之间同步变化。

## 不可见项目

有些项目并不是要让玩家看到的，但在编辑场景时会显示出来，以便更容易管理它们。像 **氛围**, **触发区域**, **点击区域**等项目就是如此。

在高级模式下，这些项目都有一个 **可见性** 组件被设置为不可见。这个组件不会影响场景编辑器中项目的可见性，但任何被设为不可见的项目在预览运行时都不会被玩家看到。

## 另请参见

* [智能项目 - 基础](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/)
* [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)
* [让任意项目变成智能项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)
* [与代码结合](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
