> 在场景中使用智能物品来增加交互性。

# 智能物品 - 基础

场景编辑器目录中的一些物品是 **智能物品**。玩家可以与这些物品交互，它们具有可配置的属性，并且可以触发其他智能物品上的动作。例如：可以打开和关闭的门、会上下移动的平台，或者可以激活其他物品的按钮和拉杆。



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/z7HF4GR01hE" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=z7HF4GR01hE>">Watch on YouTube</a></p>



你可以在资源包浏览器中认出这些物品，因为它们带有闪电图标，并且背景颜色不同。

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

你可以在实体树中通过它们旁边的闪电图标来识别场景中的哪些物品是智能物品。

![](/docs/_remote/d3ff99754b5e412ff6b047ee.webp)

## 使用物品

要使用智能物品，只需像其他物品一样将其拖入场景中。所有物品都包含默认行为，运行场景预览来试试。

以下是一些常见物品及其默认行为：

* **门**：门在被点击时会打开或关闭。你可以更改这种行为，让它们通过按钮、触发区域等方式打开。
* **按钮**：当被点击时，它们会播放声音并播放动画作为反馈。向其触发事件添加更多动作，可以激活其他智能物品。
* **拉杆**：当被点击时，它们会在两种状态之间切换。让拉杆的每个位置对其他智能物品执行不同的动作。
* **宝箱**：它们的行为类似于门，默认在点击时打开或关闭。你可以把较小的物品放在里面。
* **平台**：它们在两个位置之间移动。使用它们的补间动作来控制移动到哪里、速度等。
* **触发区域**：一种不可见的物品，当玩家走入其区域时可以触发其他智能物品。参见 [关于触发区域](#trigger-areas).
* **头像修改器区域**：一种不可见区域，会改变头像在其中的行为或外观，例如隐藏头像或禁用护照弹窗。参见 [修改器区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas/).
* **摄像机修改器区域**：一种不可见区域，会在玩家进入其中时强制其摄像机切换为第一人称或第三人称。参见 [修改器区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas/).
* **视频播放器**：用于显示视频或直播流的屏幕。参见 [播放视频](#playing-videos).
* **音频流**：从直播流播放音频。参见 [播放音频流](#playing-audio-streams)
* **NFT**：将 NFT 图像显示为肖像。参见 [显示 NFT](#displaying-nfts)
* **地板压力按钮**：一个圆形踏板，玩家站上去时会下沉，离开后会弹回原位。使用其 **按下时** 和 **松开时** 钩子来运行动作，例如在有人站在上面时保持门打开。
* **水面** 和 **水柱**：玩家走进或跳入时会溅起水花的水面。参见 [水](#water).

所有智能物品都可以配置为以自定义方式运行。例如平台移动多远，或者按钮会激活什么。

## 配置物品

在场景编辑器中选择一个物品，即可在右侧查看其所有属性。

大多数智能物品会将其行为保存在一个 [脚本组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/)中。它们的设置会显示在 **脚本** 面板里，每个参数对应一个字段，每个字段都有提示说明其作用。使用它们不需要编写任何代码。

你可以在许多物品中找到的一些典型字段有：

* **悬浮文本**：当玩家将光标移到物品上方时，UI 上显示的提示文本。例如门可能会显示“打开”。
* **交互**：该物品由哪个按钮触发？在普通键盘上：
  * **主键** 是 **E**
  * **副键** 是 **F**
  * **指针** 是 **鼠标左键**
  * **Action3** 是按键 **1**
  * **Action4** 是按键 **2**
  * **Action5** 是按键 **3**
  * **Action6** 是按键 **4**
* **响应物品的钩子**：你在其中选择特定时刻要运行的动作的参数。一个物品有哪些钩子取决于它的作用。按钮有 **onClick**，拉杆有 **onActivate** 和 **onDeactivate**，移动平台有 **onReachStart** 和 **onReachEnd**，铃铛有 **onRing**。将某个钩子留空，则在那个时刻不会发生任何事情。

每个物品都有自己特定的设置，这些设置可能因物品而异。

有少数物品仍保留旧的 **配置** 面板。比如灯光，仍然在那里设置颜色和强度。

所有物品都有一个 **高级模式** ，可让你几乎配置它们的任何内容。这包括诸如播放什么声音，或者平台朝哪个方向移动。你还可以添加自定义动作，涵盖各种功能，例如传送玩家、播放头像动画、将物品附着到玩家手中等。你也可以添加条件逻辑，只在某些情境下激活某些内容。参见 [智能物品 - 高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/).

![](/docs/_remote/ff0b8ab0126b0a18e07ec387.webp)

## 对另一个物品调用动作

智能物品可以触发其他智能物品上的动作，因此每次物品被激活时都会发生这些动作。只需从场景中所有物品的列表里选择你想调用的物品，然后选择一个动作。不同物品会暴露不同的动作。

例如，这里有一个按钮，它会打开或关闭一扇门。每次按下按钮时，门都会打开或关闭。

![](/docs/_remote/564f4fe0ae016962310472b6.webp)

这里有一个拉杆，它在激活时打开门，在停用时关闭那扇门。

![](/docs/_remote/dc29efe9b4e6df7d2b39a714.webp)

你可以添加多个来自不同物品的不同动作，让它们一起触发。只需点击 **+ 分配动作**.

通过点击动作旁边的三个点并选择来移除动作 *移除动作*.

物品暴露的动作保留其熟悉的名称，例如 **打开**, **关闭**, **坐在这里**, **开启**，或 **传送**，因此物品之间的连线方式仍与以往相同。

你也可以串联动作。例如，如果拉杆的 **onActivate** 会打开一扇门，而那扇门自己的钩子又触发了其他事情，那么拉杆就会间接触发整个链条。

如果你使用 [高级模式](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/) ，也可以为这些动作添加条件逻辑。

## 特殊智能物品

有些智能物品具有独特特性，使它们在常见场景中非常实用：

### 触发区域

使用触发区域智能物品，在玩家走入某个区域时触发动作。

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

把它拖进来，调整盒子或球体大小以覆盖你想要的区域，然后使用 **当玩家进入……** 和 **当玩家离开……** 部分中的按钮来描述应发生什么。 **反应** 。 [AI 助手](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/) 会为你编写脚本。参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area/) 获取完整演练。

![](/docs/_remote/83142e91c462787a36ca8b35.webp)

参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area/) 了解更多信息。

### 隐形墙

一组不可见的形状，可阻止玩家穿过某个区域或点击穿过该区域。

当导入没有碰撞网格的 3D 模型时，或者当你想创建玩家看不见的墙时，这些隐形墙会很有用。

参见 [碰撞体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/) 了解更多信息。

### 点击区域

一个不可见的立方体，玩家可以点击它来触发其他任意智能物品上的动作。此物品可以被其他智能物品启用或禁用；禁用时它无法被点击。你还可以设置玩家将光标指向它时看到的文本。

![](/docs/_remote/43e0c40165045ebe199d3264.webp)

### 座位

椅子、长凳和其他座位，允许玩家通过点击坐下。

* 座位只有在拿走它的玩家 **走开** 超过 1.5 米后才会空出来。
* 在带有多个座位的位置的物品上，例如长凳，点击它会让你坐到 **最近的空位**.
* 当所有座位都被占用时，悬浮文本会显示 **座位已被占用** ，点击不会产生任何效果。

### 水

以下类别中的两个物品可为你提供玩家可以在其中嬉水的水体： **材质** 类别

* **水面**：一个 16 x 16 米的平面。可缩放它以覆盖池塘、运河或海洋。
* **水柱**：一个圆形池，宽 8 米，深 2 米。

当玩家走进或跳入其中时，两者都会溅起水花。你可以设置：

* **由谁激活**: `我的玩家` ，这样只有你能看到自己的水花，或者 `所有玩家` ，这样每个头像都会溅水。
* **深度**：水花区域从水面向下延伸多远，单位为米。它永远不会超过池底。
* **水花大小**：水花效果的大小，单位为米。
* **声音**：物品文件夹中音频文件的名称，每次水花溅起时播放。留空则为静音水面。

它们还暴露 **进入** 和 **离开** 事件，以及一个 **溅起水花** 动作，可按需让水中央溅起水花。


> [!WARNING]
> **📔 注意**：每一层水都需要自己的法线贴图。没有法线贴图的层将不再动画，如果一个都不剩，溅水区域会缩小到 1 x 1 米。


### 传送门

该 **传送** 物品会在玩家与其交互时将玩家送到别处。

设置 **x** 和 **y** 参数可将他们发送到 Genesis City 中的一个地块。要改为发送到 [世界](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/#decentraland-worlds) ，请在 **world** 参数中填入世界名称，例如 `myname.dcl.eth`，并留空坐标。


> [!WARNING]
> **📔 注意**：以前有一个单独的 **世界传送** 物品。它已被移除。请改用带有其 **传送** 参数的 **world** 物品。


### 移动设置

该 **移动设置** 物品会改变玩家在场景中的移动速度和跳跃高度。可用它让场景感觉更飘浮、在博物馆里减慢玩家速度，或者在大型地图中加快他们的移动。

将它拖到场景中的任意位置。它会在场景开始时应用其数值，而其占位立方体只会在编辑器中可见，玩家看不到。

每个字段都是一个滑块，预设为 Decentraland 的默认值：

| 字段                    | 默认值     | 范围     |
| --------------------- | ------- | ------ |
| `walkSpeed`           | 1.5 米/秒 | 0 到 10 |
| `jogSpeed`            | 8 米/秒   | 0 到 30 |
| `runSpeed`            | 10 米/秒  | 0 到 50 |
| `jumpHeight`          | 1 米     | 0 到 10 |
| `runJumpHeight`       | 1.5 米   | 0 到 10 |
| `doubleJumpHeight`    | 2 米     | 0 到 15 |
| `glidingSpeed`        | 6 米/秒   | 0 到 30 |
| `glidingFallingSpeed` | 1 米/秒   | 0 到 10 |
| `hardLandingCooldown` | 0.75 秒  | 0 到 5  |

默认情况下，慢跑是玩家的移动速度。行走和奔跑则是通过按住 Control 或 Shift 键获得的。

该物品还暴露两个可供其他智能物品调用的动作：

* `apply`：将当前字段值再次写回给玩家，例如在触发区域或按钮之后。
* `restoreDefaults`：清除你的数值，让玩家恢复为 Decentraland 的默认值。

参见 [移动设置与限制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/locomotion/) 了解每个设置的详细作用，以及如何从代码中更改这些值。

### 播放视频

可从以下来源播放视频：

* **本地文件**
* **从 URL 直播**
* **从以下平台进行直播** [**Decentraland Cast**](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/#dcl-cast-easy)
* **从以下平台进行直播** [**RTMP 软件**](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-shi-yun-ying/live-streaming/#stream-advanced) **（OBS、XSplit、StreamYard 等）**


> [!WARNING]
> **📔 注意**：播放视频对性能要求很高，因此请保持同时播放的视频数量较少。引擎会根据每位玩家的画质设置限制可同时播放的视频数量（低画质为 1，中画质为 5，高画质为 10），并暂停超出该限制的任何视频。参见 [播放视频](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/#play-videos).


参见 [播放视频](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/video-screen/) 了解更多信息。

### 播放音频流

使用 **音频流** 智能物品从 URL 播放音频流。


> [!NOTE]
> **📔 注意**：并非所有流媒体服务都允许你在其网站之外播放音频。以下是一些在 Decentraland 中可用的示例：
> 
> ```ts
> GRAFFITI = 'https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5'
> SIGNS = 'https://edge.singsingmusic.net/MC2.mp3'
> DELTA = 'https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e'
> JAZZ = 'https://live.vegascity.fm/radio/8010/the_flamingos.mp3'
> ```
> 


你可以调整流的音量。请注意，流中的音频不是空间定位的，而是在整个场景中以相同音量被听到。

### 显示 NFT

要在相框上显示 NFT，请使用 **NFT** 智能物品。你必须提供以下字段：

* 网络


> [!NOTE]
> **📔 注意**：目前 **ethereum** 是唯一受支持的网络。


* NFT 合约：NFT 系列的智能合约。
* Token ID：此特定 NFT 藏品的代币 ID。

<img src="/docs/_remote/9bae5521b60927140f632d59.webp" alt="NFT 形状" width="400">

你可以从 [OpenSea](https://opensea.io)获取这些信息，方法是查看 NFT 图像下方的 **详情** 选项卡。

<img src="/docs/_remote/2ef80788a2ffef0f4c7b360b.webp" alt="OpenSea 详情" width="400">


> [!NOTE]
> **📔 注意**：你也可以从 opensea URL 获取这些信息。例如，如果 NFT 的 URL 如下：
> 
> > `https://opensea.io/assets/ethereum/0x32b7495895264ac9d0b12d32afd435453458b1c6/1956`
> 
> 你可以用以下内容填写字段：
> 
> * 网络：ethereum
> * 合约：0x32b7495895264ac9d0b12d32afd435453458b1c6
> * Token：1956


你还可以配置背景颜色，这对带透明背景的 NFT 尤其有用。

你还可以选择一个 **边框样式**，以多种不同风格、经典或现代的边框来装饰 NFT。

参见 [NFT](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/nfts/) 以了解更多详情。

### 生命值条

![](/docs/_remote/4b7b9bba4ad2a27559774acf.webp)

该 **生命值条** 智能物品是多个游戏机制的绝佳构建块。它可用于多种方式：

* 将其嵌套在 **玩家** 下方，以在头像上方显示玩家的生命值

  ![](/docs/_remote/7ec4155f634538c1243f6333.webp)
* 将其嵌套在 **摄像机** 下方，以将其固定显示在 UI 上

  ![](/docs/_remote/8f666b7f68658ee843a4304c.webp)
* 将其嵌套在场景中的任何物品下方，以跟踪该物品的生命值

  ![](/docs/_remote/1bc82b628c58c582bc684876.webp)

其他物品可以与生命值条交互，给它增加或减少生命值。

* 像 **尖刺** 或 **机器人敌人** 这样的物品可以降低生命值

  ![](/docs/_remote/8a3aa562c2ae8a2e3b67d7c6.webp)
* 像 **急救** 或 **治疗垫** 这样的物品可以恢复生命值。

  ![](/docs/_remote/4b71eac34757d0b4ca625eca.webp)

你必须配置生命值条，以定义当生命值等于 0 时会发生什么。你可以把玩家重生到一个 **重生垫** 智能物品的位置、重置其得分计数器、重生任何敌人、显示 UI 文本，或者做任何在你的游戏逻辑中合理的事情。

当生命值低于某个值时，你也可以触发动作，例如在生命值低于 3 时播放特殊音乐或显示 UI 提示。

生命值条可以被配置为影响任何东西！例如，在 **木门** 智能物品下添加一个生命值条。玩家可以使用 **剑** 智能物品、也可以通过 **桶** 的爆炸或者 **机器人敌人**的攻击来降低这个条的生命值。要使其生效，请配置生命值条，使其在数值为 0 时对其父物品执行一个动作。

![](/docs/_remote/03f177d0727bf4a6bb82243f.webp)

像 **剑** 这样的武器可以被玩家拾取，然后在执行动作时，用来对玩家附近任何具有生命值条的其他物品造成伤害。

## 多人模式

几乎所有智能物品都具有多人行为，因此当物品改变状态时，场景中的所有玩家都会共享同样的体验。如果玩家 A 打开了一扇门，玩家 B 也会看到那扇门打开。如果玩家 C 随后在其他玩家仍在场时进入场景，她也会看到那扇门已经打开。

不过，如果场景附近没有玩家，那么场景会恢复到默认状态。因此，如果所有玩家都离开了，然后玩家 A 回来，她会发现门是关着的（如果这本来就是门的默认状态）。

请确保你设计场景时，不会让一个玩家的动作破坏后来加入的其他玩家的体验。例如，如果场景是一个解谜游戏，你可以在 *延迟* 动作放到一个 *工具* 智能物品上，让谜题解开几秒后，场景中的所有物品都重置为初始状态。

你也可以禁用某个物品的多人行为，参见 [智能物品 - 高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/).

## 故障排除

* *我场景中的一个物品应该可以点击，但却无法点击*.

请确保它没有被其他东西遮挡。你无法穿过其他物品进行点击。有些物品有一个 *碰撞网格* 它具有简化的几何形状，即使其可见外形看起来并没有遮挡，也可能会挡住你的物品。试着移动该物品看看会发生什么。

## 另请参见

* [智能物品——高级](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items-advanced/)
* [状态和条件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/states-and-conditions/)
* [让任何物品变得智能](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/make-any-item-smart/)
* [与代码结合使用](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
