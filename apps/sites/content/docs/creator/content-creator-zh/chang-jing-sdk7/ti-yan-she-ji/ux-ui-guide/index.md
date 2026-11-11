> 构建让玩家愿意停留并回访的场景的技巧与实践！

# UX 与 UI 指南

对我们来说，Decentraland 内容创作者能够自由、充分地发挥创造力至关重要。这对于让 Decentraland 成为一个值得探索并有所收获的地方至关重要。不过，也要记住，Decentraland 中的场景会被各种各样的用户访问，我们希望这些场景对所有人都可访问。正因如此，我们不能忽视建立统一设计标准的必要性；这份指南能够确保玩家无论进入哪个场景，都能获得同样直观且愉快的体验。

在本文档中，我们分享 Decentraland 的设计价值，旨在推广我们认为最适合用于设计玩家界面与体验的最佳实践。我们希望这些标准能够作为构建和/或改进组成元宇宙的各个场景的起点。

## 设计价值

Decentraland 中的所有场景都应以这些价值为设计依据：

* **热情欢迎**：玩家感到受到了热情欢迎。
* **易于使用**：UI 易用且有趣——你应该避免重复造轮子
* **易学**：玩家会觉得这个 UI 熟悉且直观。整个场景中应持续一致地使用相同模式。
* **提供引导**：场景会提供帮助。文字、动效、声音和图形会引导并提示玩家朝正确方向前进。
* **响应迅速**：对玩家输入有清晰的动作-反馈反应。
* **极简**：少即是多。玩家可以把注意力集中在重要的事情上。
* **有趣**：场景充分利用了让 Decentraland 独特且值得访问的那些特点。
* **有目的**：玩家有充分理由再次回来。
* **令人愉悦**：玩家会享受事物的视觉、听觉和触感

## 用户体验

### 从哪里开始？

你的场景目标是什么？是为了提供一个可供欣赏和探索的美丽景观吗？是一个博物馆吗？是单人游戏吗？是竞技型游戏吗？还是遵循线性叙事流程？

在设计过程的每一步都清楚地意识到这一目标非常重要；它应该指导你的所有决策，而这些决策都会深刻影响访客的体验。

对于遵循线性流程的场景，如果每一步都依赖前一步，我们建议用墙体围住场景，并设置一个或多个固定入口，以避免玩家以错误顺序接触到内容。顺带一提，入口点是展示欢迎信息并说明玩家在场景中目标的绝佳位置。

<figure><img src="/docs/_remote/978e0e21e5bb4482320c0ae1.webp" alt="In-world banner" width="800"><figcaption><p>场景内说明</p></figcaption></figure>

如果玩家在你场景中的体验并不是线性的，但你仍需要提供一些基本说明来确保他们充分享受体验，你可以在弹窗中显示说明。无论玩家从哪个方向进入，都可以在他们进入时立即在屏幕上显示一个弹窗，从而确保玩家始终知道最重要的信息。

<figure><img src="/docs/_remote/bab29f1240d71035b8426a51.webp" alt="Pop-up banner" width="800"><figcaption><p>弹窗说明</p></figcaption></figure>


> [!WARNING]
> **📔 注意**：确保弹窗不会太过打扰，而且点击任意位置都能轻松关闭。


欢迎信息应向玩家传达以下内容：

* 场景标题
* 欢迎玩家
* 玩家的最终目标与动机
* 说明/下一步
* *（可选）* 控制说明。主要在你的场景使用全局按钮事件时有用。否则，物品本身在悬停时会提示如何使用。

### 动机

你很可能希望访客不断回到你的场景中，想想有哪些方法可以激励他们这么做！问问自己：“什么会让玩家想回到我的场景，或者想把它推荐给朋友？”

例如，你可以给他们设置挑战，提供一个需要多次尝试才能完成的目标。你也可以组织多人竞技活动，这也是一个很好的社交机会。让你的场景成为一个充满活力的地方，让玩家觉得在这里度过时间很愉快！

### 反馈与声音

声音扮演着双重关键角色，别错过使用它！一方面，声音对于营造沉浸感至关重要，很多没有声音配合就不会显得真实的事物，都会因此失色。另一方面，声音的另一个关键作用是为玩家的操作提供反馈。如果你只依赖视觉提示，就无法确定玩家是否正朝着正确方向看，从而看到其操作的效果。通过使用声音，你可以确保玩家意识到某件事发生了。如果将视觉提示和声音提示结合使用，听到声音会促使玩家四处寻找对应的视觉提示，以防他们刚才没看到。

请记住，有些玩家可能会关闭声音进行游玩。如果你把声音用作提供场景说明或游戏机制中的关键内容，尽量也提供某种视觉提示来指向同样的方向。例如，你可以在 UI 底部显示文字来配合这些声音，就像电影字幕一样。

## 用户界面

本节旨在帮助你通过场景设计构建叙事，引导玩家把注意力集中到最重要的元素上。善用这里描述的工具，可以让他们的体验更具沉浸感，也更成功。

我们再怎么强调色彩、图标、动效和比例在传达正确信息方面的重要性都不为过：请记住，场景中的一切都在传达含义。

### 布局

在将 UI 放置到与默认 Decentraland UI 元素重叠的屏幕区域时要小心。在桌面客户端中，默认 Decentraland UI（侧边栏、小地图、聊天等）被设计为只占据屏幕左侧 25% 的区域（在图中以红色高亮显示）。剩余的屏幕空间是内容创作者可以自由使用的安全区。

<figure><img src="/docs/_remote/1f94cb72ac68f183e0aaa73e.webp" alt="Screen regions used by UI" width="800"><figcaption></figcaption></figure>

在屏幕左侧 25% 的区域放置 UI 元素没有限制，但请记住，Explorer 一直在持续改进和调整其设计。今天在该区域内未被遮挡的空间，未来版本中可能会被覆盖。


> [!NOTE]
> **📱 移动端安全区域**： [移动客户端](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/) 会在画布的一部分上绘制自己的控件： **左侧** （聊天、搜索、个人资料、摇杆、表情动作）、 **右上角** （个人资料和摄像机控制），以及 **右下角** （动作和交互按钮）。不要手动测量这些区域：请在客户端报告的区域内渲染，使用 `screenInset: 'interactable'`。请将关键 UI 保持在屏幕中央或上中部，并遵循 [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/)时无需使用 [`isMobile()`](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) ，以便在桌面端和移动端约束不一致时切换布局的



> [!WARNING]
> **📔 注意**：来自场景和智能穿戴设备的 UI 元素始终显示在默认 Decentraland UI 后方的图层上。


#### 永远不要将 UI 锚定到左上角

最常见的布局错误，是把一个面板固定在左上角，或者把它放在覆盖整个屏幕的根节点原点上。那个角落以及整个左边缘，正是 Explorer 绘制自己的小地图和聊天窗口的位置，而在移动端则是虚拟摇杆的位置。放在那里的场景 UI 仍然会渲染，但客户端自己的 UI 会盖在它上面，导致它被遮住，按钮也常常无法点击。布局引擎不会对此发出任何警告。

请改用以下方法之一：

* **将元素锚定到右侧，或将其居中。** 对于分数、计时器或小面板，设置 `positionType: 'absolute'` 和 `position: { top: 40, right: 40 }`，或使用 `justifyContent: 'center'` 在根节点上将其居中。
* **让 Explorer 告诉你哪些位置是安全的。** 将 [`screenInset: 'interactable'`](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#screen-inset-area) 在渲染器上。你的 UI 随后会被放置在 Explorer 为场景 UI 预留的矩形区域内，该区域已经排除了小地图、聊天和左侧控件。

```ts
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```

在移动端，右下角的动作按钮会刻意覆盖在那片区域上，那里发生的点击会发送给客户端按钮，而不是你的场景。请把交互元素避开右下角；或者，如果 UI 本身就是要占满屏幕的，就在它打开时隐藏触摸控制：

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// 当全屏面板打开时，玩家不需要跳跃或按 E
TouchScreenControls.hideAll()
TouchScreenControls.hideJoystick()

// 面板关闭时恢复它们
TouchScreenControls.showDefault()
TouchScreenControls.showJoystick()
```

`showDefault()` 会恢复场景开始时的一组默认按钮：跳跃、指针和主操作。请仅在 `showAll()` 只有在你的场景确实希望屏幕上显示全部八个按钮时才使用。二者都需要 SDK 7.32.0 或更高版本；在旧版 SDK 中，默认会显示全部八个按钮，而 `showAll()` 是唯一的恢复方式。这些辅助函数在桌面端上不起作用，因此你可以无条件调用它们。参见 [屏幕控件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/).

始终把网格放在心中，并把它作为空间组织 UI 时的主要标准。如果你正在制作一个需要 HUD 的场景，可以先把所有消耗品放在一起（例如货币、食物、生命值），另一侧则放工具或武器等可携带物品。尽量保持网格尺寸和内边距的一致性。让每个玩家操作的反馈都清晰明确。

### 颜色

选择一套配色方案非常重要，因为它会赋予你的场景身份，同时也会提示元素之间的关系和层级。

在组合配色方案时，先选择一个主色，必要时再选择一个辅助色。然后你需要决定，配色方案相对于这个主色是类似色、互补色，还是三色等。

<figure><img src="/docs/_remote/06d8337c29d507083f4727ab.webp" alt="Chromatic color palette" width="300"><figcaption><p>彩色调色板</p></figcaption></figure>

<figure><img src="/docs/_remote/cf0b4424895884aa68f549b1.webp" alt="Strategies for combining colors properly" width="300"><figcaption><p>正确搭配颜色的策略</p></figcaption></figure>


> [!NOTE]
> **💡 提示**：我们推荐的免费配色生成器： [Coolors](https://coolors.co), [Adobe Color CC](https://color.adobe.com/), [Colour Lovers](http://www.colourlovers.com), [Color Hunt](http://www.colorhunt.co), [Color by Hailpixel](http://color.hailpixel.com), [Colour Code](http://colourco.de), [Sip](https://sipapp.io/), [Paletton 的 Color Scheme Designer](http://paletton.com), [Cohesive Colors](http://javierbyte.github.io/cohesive-colors), [Colr](http://www.colr.org)


主色应该是你 UI 组件中使用最频繁的颜色。如果你的配色方案没有辅助色，可以通过将主色与黑色或白色组合来突出某些元素。拥有辅助色不是必须的，但它有助于强调并区分你的场景 UI 元素。辅助色最适合用于高亮、选择控件（滑块和开关）、链接和标题。

请记住，每种颜色都有其独特的表达特性，要善加利用，通过颜色传达信息。例如，红色常常与负面含义相关，而绿色则常常与正面含义相关。

<figure><img src="/docs/_remote/f0a0dc7306efbc4e1bb5c257.webp" alt="Colored switches" width="300"><figcaption><p>错误地使用颜色会造成混淆</p></figcaption></figure>

你也可以策略性地使用颜色来指示状态变化，例如通过改变某个元素的颜色来表示它是激活还是未激活。


> [!WARNING]
> **📔 注意**：状态用于传达 UI 元素的状态。元素的不同状态应保持一定连续性，但同时必须具备清晰的可操作性，并且容易与其他状态及周围布局区分开来。


<figure><img src="/docs/_remote/006fc8ebd49f1ffb83ca7363.webp" alt="Active inactive button" width="300"><figcaption><p>用颜色表示按钮状态</p></figcaption></figure>

警告！在选择将以叠加方式显示的颜色时，请特别注意确保它们一起使用时仍然清晰可读。这里有一条基本规则：同时显示的元素颜色之间应始终具有足够的对比度。请注意，有些玩家可能在光线不理想的环境下看屏幕，这会使阅读更加困难。

<figure><img src="/docs/_remote/78f863c16ac2c51e8736e68a.webp" alt="Icon contrast" width="300"><figcaption><p>这两个图标都能区分出来，但第一个对比度更好。它更容易阅读，也更省理解力。</p></figcaption></figure>

### 字体层级

尽量为标题、副标题和正文定义至少 3 个字号层级。注意颜色和字号的使用。所有字号都应为 12 px 或更大。

如果你要放置显示在图片上方（或世界上方）的文字，你肯定需要测试其可读性。在这些情况下，我们建议你在文字和图像之间添加一层有颜色的实心区域，这样可以确保文字保持清晰可读。

<figure><img src="/docs/_remote/26f47a30aa943bf272e86fd8.webp" alt="Text over images" width="300"><figcaption><p>优先保证可读性</p></figcaption></figure>

### 图标

图标能综合信息，通过图像帮助你识别操作。与用文字标注事物相比，它们是一个极其强大的工具，能够快速传达可被解读的信息，因为文字会要求玩家投入更多注意力和时间。图标与文字结合显示也很有用，因为这有助于消除其含义上的歧义。

<figure><img src="/docs/_remote/d3655e8d247e6f40487d2d6a.webp" alt="Icon examples" width="300"><figcaption><p>将图标作为通用语言</p></figcaption></figure>

### 动效

在需要时使用动效提供反馈并引导方向。动效能帮助玩家集中注意力，也能在 UI 变化时维持连续性。

<figure><img src="/docs/_remote/db15a9b89e2052e22e2de076.webp" alt="Adding glow" width="300"><figcaption><p>使用粒子、缩放或渐隐边缘来突出内容</p></figcaption></figure>

#### 避免闪烁效果

闪光灯、频闪灯以及快速的亮度或颜色变化，可能会诱发光敏性癫痫患者的癫痫发作。这包括脉冲式自发光材质、闪烁灯光、全屏 UI 闪烁和视频。

* 将任何闪烁限制在 **每秒 3 次闪烁或更少**，遵循 [WCAG “每秒三次闪烁或以下阈值”](https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html) 指南。
* 保持闪烁区域较小，并避免使用饱和红色和大面积、高对比度、充满整个屏幕的闪烁效果。
* 如果你的场景依赖强烈效果，例如俱乐部频闪、闪电或受伤闪烁，请在场景中加入一个选项，让玩家可以关闭这些效果或降低其强度。

### 写作与内容结构

UI 文本可以让界面更易用，也能让玩家对自己的操作更有信心。始终让 UI 文本尽可能简洁。玩家是来玩的，不是来读文章的。任何看起来太长的文本，大多数玩家都不会读。

用欢迎信息迎接你的玩家，并告诉他们场景目标。先说明他们在场景中的目标，再说明实现目标所需的操作。然后你可以在需要时逐步披露信息，这样玩家在体验一开始就不会感到不知所措。和图形资源一样，尽量在 UI 功能和叙事中保持用词一致。

#### 展示，不要直说

有两种模式会在玩家第一次交互之前就把他们劝退：

* **规则弹窗**：一个包含多段密集文字的介绍面板，涵盖每个机制、计分规则和控制方式。玩家会直接关闭而不阅读。
* **场景内规则手册**：把同样的段落写在场景中的标牌或板子上。看起来像是设计的一部分，但场景内文字太小、太长，根本读不下去，而且玩家也不会站着不动去眯着眼看。

把介绍面板控制在一屏之内，不要滚动：3 到 5 行简短文字，理想情况下是一行写目标，一行写控制方式，再配一张图。一个标牌只说一件事，10 个词以内，字体要大，一个标牌对应一个想法。

然后用展示代替书写：

* 让第一个目标一眼就能看出来。把一个发光的、会动的，或者尺寸很大的物体放在玩家开场视线范围内。脉冲式高亮轮廓比一句话更能教会玩家“点击我”。
* 在真正的游戏开始之前，先让玩家在练习区域安全地完成一次机制操作。
* 使用 `悬停文本` 作为一种即时说明，用在可交互实体上。它会在最相关的时候精确出现，而且不会增加前期阅读负担。参见 [悬停反馈](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/#hover-feedback).
* 对于空间规则或顺序规则，请使用示意图。“红宝石 = 1，蓝宝石 = 5”的图片就能替代一大段文字。
* 任何更长的内容，都应通过 NPC 或玩家可以跳过的对话，一次只传达一条信息。

如果某条规则需要比这更多的文字，那就重新设计机制，或者把它可视化。

### 对象交互

确保玩家能够分辨哪些对象可以交互，哪些不可以。在实体上注册按钮事件时，SDK 会默认显示悬停反馈：实体边缘高亮，并在光标旁显示提示，告知要使用哪种输入，同时还能用自定义文字描述交互效果。我们强烈建议使用它，这会让你的场景感觉更自然，也更容易使用。参见 [悬停反馈](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/#hover-feedback).

### 第三人称摄像机

玩家可以随时按下 ‘V’ 键在第一人称和第三人称摄像机之间切换。设计场景时，应确保交互在两种模式下都能舒适使用：例如，小的点击目标在第三人称下更难点中。你的场景可以读取玩家当前的摄像机模式，或者在某个区域强制使用特定模式，参见 [摄像机](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/camera/).

第三人称摄像机也会改变房间需要多大。按现实尺寸建造的室内空间在屏幕上会显得很局促，参见 [把室内空间建得比现实更大](/docs/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/design-games/#build-interiors-larger-than-real-life).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/ux-ui-guide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
