> 如何对你的 Decentraland 游戏进行试玩测试，并将玩家反馈转化为改进

# 试玩测试

游玩测试能帮助你做出更好的游戏，因为它能让你了解真实玩家是如何体验游戏的。

你不需要已经完成的游戏或大量测试者就能开始。你可以尽早测试、进行改进，并随着游戏发展再次测试。

本指南解释如何组织游玩测试、收集玩家反馈，以及如何利用你学到的内容来改进游戏。

## 为什么要为你的游戏做游玩测试？

当你制作游戏时，你已经知道它是如何运作的。新玩家并不知道。

游玩测试能帮助你发现一些否则可能会遗漏的东西：

* **清晰度：** 玩家明白该做什么吗？
* **易用性：** 玩家能完成操作而不会卡住吗？
* **乐趣：** 哪些部分有趣，哪些部分令人沮丧？
* **动力：** 玩家是否想继续玩，或者再回来？

**尽早开始游玩测试。** 只看几个人尝试你的第一个可游玩版本，你就能学到很多。

## 选择你想测试的内容

每次游玩测试前，先决定你想了解什么。

一次只聚焦一两个问题，而不是试图一次测试所有内容。

| 你的目标   | 要关注什么                            |
| ------ | -------------------------------- |
| 测试引导流程 | 新玩家在没有帮助的情况下能明白该做什么吗？            |
| 测试核心机制 | 玩家能使用主要游戏机制吗？它有趣吗？               |
| 测试游戏循环 | 玩家明白如何推进进度并获得奖励吗？                |
| 测试难度   | 玩家在哪里会卡住、感到无聊或沮丧？                |
| 测试情绪反应 | 玩家会感到兴奋、好奇，还是想继续玩？哪些时刻会引发最强烈的反应？ |

尽可能选择对你的游戏不熟悉的玩家。他们更有可能发现有经验的玩家已经不再注意的问题。

### 让游玩测试与你游戏所处阶段相匹配

先决定你想得到哪种反馈。起初，你主要需要知道玩家是否 **理解** 游戏，并且能形成 **策略**：他们知道该做什么吗？他们能想出怎样把它做好吗？一旦这部分稳定了，你就需要了解游戏让他们产生什么 **感受**：它是否在合适的时刻让人兴奋、满足或紧张，以及他们是否想再回来？

你邀请的玩家类型也会随着阶段变化。熟悉 Decentraland 的人可以越过客户端直接进入你的游戏，所以在测试机制时他们最合适。完全的新手会在各个地方都遇到磕绊，而当你测试完整的首次体验时，这正是你想要的。

| 阶段          | 你想了解什么                    | 邀请谁                                    |
| ----------- | ------------------------- | -------------------------------------- |
| V0（垂直切片）    | 核心机制是否易懂且可用？玩家能围绕它形成策略吗？  | 找几位已经了解 Decentraland 的玩家，这样客户端就不会干扰测试。 |
| V1（第一个完整循环） | 玩家能独立推进吗？他们在哪里会卡住或失去兴趣？   | Decentraland 玩家和从未玩过你游戏的人混合在一起。        |
| V2 及以后      | 游戏让玩家有什么感受？他们会回来吗？他们会推荐吗？ | 新玩家，包括来自 Decentraland 社区之外的人。          |

根据你所处的阶段来写问题。对 V0 切片问“你觉得好玩吗？”收获不大，因为游戏内容还不够，答案意义有限。对已经经历过几轮测试的 V2 问“你明白该做什么吗？”则是在把一次会话浪费在你本来就已经知道的事情上。

## 使用 Scene Feedback 自动收集反馈

Scene Feedback 是一个可复用模块，让你可以直接在 Decentraland 游戏内向玩家提问。

即使你不在场观看玩家游玩，它也能帮助你了解玩家如何体验你的游戏。

你可以：

* 在游戏进行中的特定时刻向玩家提问。
* 收集 1 到 5 分评分和书面评论。
* 从你 World 的 Scene Storage 中将回复下载为 CSV 文件。

### 你首先需要什么

Scene Feedback 会把答案存储在服务器上，所以你的场景需要一个 [多人服务器](/docs/creator/content-creator-zh/chang-jing-sdk7/lian-wang/authoritative-servers/)，可通过使用 `@dcl/sdk@auth-server` 并设置 `"authoritativeMultiplayer": true` 在 `scene.json`.

### 安装 Scene Feedback

1. 打开 [Scene Feedback 仓库](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback).
2. 按照 README 将模块安装到你的场景中。你也可以把安装提示提供给你的 AI 编码代理。
3. 选择一小组与你的游玩测试目标相匹配的问题。
4. 决定每个问题何时出现。
5. 将玩家回复下载为 CSV 文件。在 Creator Hub 中，打开 **管理**，然后进入你的 World 的 **⋮** 菜单，然后 **查看存储**。一段时间内，回复可能会分散在几个文件里（新的、尚未合并的保存，以及该模块早期版本中的一个旧的单文件）。下载所有文件名以相同 `scene-feedback-<number>`开头的文件，并删除带有重复 `id`的行。

该仓库包含可直接使用的问题、示例，以及有关如何选择问题的指导。

如果你使用 AI 编码助手，请安装或更新 [Decentraland SDK Skills](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/vibe-coding/)。其中包含一个 Scene Feedback 技能，可以帮助你决定要问什么、何时提问、安装模块，并在之后分析回复：

```bash
npx skills add decentraland/sdk-skills --all
```

## 组织你的游玩测试

你可以通过两种方式组织游玩测试。

**选项 1：举办一场实时游玩测试**

邀请所有人同时来玩。你可以观察玩家、讨论他们的体验，并在之后收集反馈。

你也可以在 [Decentraland 活动](https://decentraland.org/events/) 上创建一个活动，让玩家能找到并加入你的游玩测试。

**选项 2：持续数天进行游玩测试**

让玩家在设定的一段时间内，于他们有空时随时测试你的游戏。

安装 Scene Feedback，在他们游玩时收集回复。

当你想从更多玩家那里获得反馈，而不组织实时会话时，这很有用。

## 举办一场实时游玩测试

实时游玩测试让你可以观察玩家体验你的游戏，并在之后向他们提问。

### 第 1 步：准备你的游戏

确保玩家能访问你的游戏，并完成你想测试的操作。准备两到三个任务让玩家尝试。

例如：

* 找出如何开始游戏。
* 完成一轮。
* 购买一个升级。

### 第 2 步：邀请测试者

邀请一小群玩家。三到五名玩家是一个很有用的起点。尽量包含一些以前没玩过你游戏的人。

你可以在 [Decentraland Discord](https://discord.gg/decentraland)。参见 [寻找测试者](#find-playtesters) 频道里找到感兴趣的玩家。

### 第 3 步：观察玩家游玩

请玩家在不解释所有机制的情况下尝试游戏。观察他们做什么、在哪里卡住，以及他们喜欢什么。

**除非玩家无法继续，否则不要引导他们。** 如果你不得不解释某件事，那可能说明你的游戏需要更清晰的说明。

### 第 4 步：请求反馈

游玩后，提出几个开放式问题：

* 你最喜欢什么？
* 有没有什么地方让你感到困惑或沮丧？
* 有没有哪个时刻你不知道该做什么？
* 你会再玩吗？为什么会或不会？
* 你会把这个游戏推荐给朋友吗？为什么会或不会？

避免使用引导性问题，例如“你喜欢新的升级系统吗？”。这类问题会影响你得到的答案。

如果你不确定该问什么，上面提到的 Scene Feedback 技能可以引导你选择重点并措辞提问，无论你打算当面问还是在游戏内问都可以。

记录玩家遇到的问题，而不只是他们的建议。

## 使用 RITE 方法改进你的游戏

RITE 代表 **快速迭代测试与评估**.

这是一种简单的游玩测试方法：发现问题、修复它，并尽快再次测试。你不是进行一次大型游玩测试并收集一长串问题，而是进行多次小型测试，并在每次之间修复最明显的问题。

### 遵循 RITE 循环

1. **测试：** 观察一小组玩家尝试你的游戏。每轮三到五名玩家就足够了。
2. **识别：** 找出影响他们体验的问题。
3. **修复：** 在各轮之间，当问题和解决方案都清楚时进行更改。
4. **重复：** 用新玩家进行下一轮。继续下去，直到玩家能在没有重大问题的情况下完成体验。

例如，如果玩家找不到游戏的开始位置，那么在下一次游玩测试前让开始按钮更显眼。然后观察新玩家是否能在没有帮助的情况下找到它。

**在一轮进行时保持游戏稳定。** RITE 的意思是在轮次之间修复，而不是在轮次进行中修复。如果你在玩家会话进行到一半时重新部署，你可能会破坏他们的游戏，而且你也无法再判断某条反馈对应的是哪个版本。先收集一轮中的所有内容，然后再做修改，之后再测试。

### 让 RITE 在 Decentraland 中发挥作用

有几个习惯能让 RITE 在 Decentraland 场景中更实用：

* **小修，小快。** 在各轮之间，只修改上一轮明确显示为问题的内容。如果某个修复需要新机制或重新设计，就先记下来，保持每轮之间的改动足够小，以便一天内完成发布。
* **使用 World 进行测试。** 将你的测试版本发布到一个 [World](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing-options/) ，而不是发布到你正在使用的 LAND 或主要 World。玩家始终会进入一个已知版本，你也可以在轮次之间重新发布，而不会影响已上线的游戏。
* **在各轮之间保持你的 Scene Feedback 问题不变。** 如果你更改了问题的措辞，就无法比较前后两轮的评分。需要的话可以添加问题，但不要重写你正在追踪的问题。
* **决定什么算“清楚”。** 当你亲眼看到问题发生、能说出玩家原本期望什么，并且知道该改什么时，这个问题就是清楚的。如果只有一名玩家遇到，而且你也不确定原因，那就留到下一轮，看看是否还会再次发生。
* **记录每一轮。** 每轮都保留一条简短记录：哪个版本、谁玩了、你看到了什么、你改了什么。到第三轮时，这份记录就会告诉你某项改动是否真的有效。

RITE 在早期开发阶段尤其有效，因为那时你可以快速做出改动。

## 寻找测试者

普通的 Decentraland 玩家最容易接触到，但也要看看 [Decentraland 社区之外](#outside-the-decentraland-community)的人：他们能发现有经验玩家会忽略的问题。

### 在 Discord 上

Discord 上的 #playtesters 频道 [Decentraland Discord](https://discord.gg/decentraland) 是给喜欢测试游戏并帮助创作者改进游戏的人准备的。加入服务器并找到该频道。

### 发布你的游玩测试

准备好后，在 #playtesters 发一条消息，包含：

* **游戏：** 你的游戏名称和简短描述。
* **位置：** 你在 Decentraland 中游戏的链接。
* **时间：** 游玩测试的日期和时间，或者反馈开放的时间段。
* **测试内容：** 你希望获得反馈的具体功能或玩法。
* **如何反馈：** 通过现场讨论、Discord 或 Scene Feedback。
* **奖励：** 你为参与提供的任何激励。

### 鼓励玩家参与

可以考虑提供一点小奖励，以感谢玩家花费的时间。

例如：

* 赠送参与者一个特别的 Wearable 或 Emote。
* 提供一个独家的游戏内奖励。Scene Feedback 示例包含一个配方，会在玩家完成表单中的每个问题后，在其头像上显示一个 **游玩测试贡献者** 标签，带有 [`AvatarNametag`](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-nametags/) 。这个标签只有该玩家自己可见，并会在他们访问期间一直显示。参见 [一个用于参与的场景本地奖励](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0%2C-1-scene-feedback/EXAMPLES.md#6-a-scene-local-reward-for-participation).
* 向符合条件的玩家赠送 Marketplace Credits。

**赠送 Wearable 和 Emote。** 你可以直接从桌面客户端把你拥有的任何 Wearable 或 Emote 赠送给其他玩家：右键点击该玩家并选择 **赠送**。该物品会转移到他们的账户，并显示在他们的 Backpack 中。你也可以从 [交易市场](https://decentraland.org/marketplace/) 使用收件人的钱包地址转移你拥有的物品。如果你创建了 Wearable 或 Emote 系列，即使你在世界中没有碰到该玩家，也可以直接从 Builder 将物品铸造到任意钱包地址。参见 [铸造 Wearable](/docs/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/fa-bu-he-ji/publishing-collections/#minting-wearables).

**Marketplace Credits 赠送目前处于测试版，仅对受邀工作室开放。** 符合条件的工作室可以向玩家赠送 Credits，玩家可以在 Decentraland Shop 中使用这些 Credits。参见 [赠送 Credits（测试版）](/docs/creator/content-creator-zh/jiang-li/gifting-credits/).

奖励可以鼓励参与，但要明确表示你希望得到真实反馈，包括批评意见。

### 在 Decentraland 社区之外

你也可以邀请从未使用过 Decentraland 的人。Decentraland 玩家已经知道客户端如何运作，所以他们会直接进入你的游戏。来自社区之外的人会以全新的眼光看待一切，包括你已经停止注意的部分。和他们一起做游玩测试，也是向新玩家介绍 Decentraland 的好方法。

独立开发者通常会从以下地方招募：

* **朋友和家人。** 容易联系到，也乐意给你一个小时。请他们诚实反馈，并在困惑时告诉你。
* **平时不怎么玩游戏的人。** 如果他们能通过你的引导流程，几乎任何人都可以。
* **你已经加入的社区。** 游戏开发马拉松服务器、兴趣论坛、本地聚会，或与你游戏主题相符的群组。

如果你的测试者不熟悉 Decentraland，帮他们进入：分享你场景的网页链接（`https://decentraland.org/jump/?realm=<your-world-name>.dcl.eth` 用于 World，或 `https://decentraland.org/jump/?position=<x>,<y>` 对于 LAND），Creator Hub 在你发布后会显示该链接，再加上一段关于如何安装客户端和登录的简短说明。即使客户端尚未安装，链接也可以在浏览器中打开。记下他们在到达你的场景前卡在哪些地方，但要把这部分与关于你游戏的反馈分开看待。

## 利用反馈改进你的下一个版本

每次游玩测试后，在决定下一步要做什么之前，先回顾你学到了什么。

1. **将相似的问题归类。** 找出多个玩家都遇到的问题。
2. **优先处理重要问题。** 修复任何会阻止玩家理解、推进或享受游戏的问题。
3. **进行改进。** 先从容易实现、且很可能有帮助的改动开始。
4. **再次测试。** 检查新玩家是否有更好的体验。
5. **跟踪你的改动。** 简要记录你改了什么，以及下一次游玩测试中发生了什么。
6. **分享发生了什么变化。** 让你的测试者知道你学到了什么，以及你根据他们的反馈做了哪些改进。在你原来的 Discord 线程中发布更新，让他们看到自己贡献的影响。

记住，玩家 **做的** 可能不同于他们 **说的**。尽可能把现场观察、Scene Feedback 回复和游戏数据结合起来使用。

## 与测试者分享你的改进

当玩家看到自己的反馈带来了改变时，他们更有可能再次参与。完成改动后，回到你在 #playtesters 的测试帖，分享一个简短更新。

使用这个简单格式： **你说了，我们学到了，我们改了。**

### 示例：游玩测试结果

**游玩测试更新：\[游戏名称]**

感谢所有测试我们游戏的人！

**你说：** 第一轮如何开始并不清楚。

**我们学到：** 新玩家很难找到开始按钮。

**我们改了：** 我们把按钮做得更大了，并添加了更清晰的说明。

**再玩一次：** \[你更新后游戏的链接]

你不需要在发布更新前修复每个问题。你也可以说明你仍在处理什么，或者你接下来想测试什么。

## 持续进行游玩测试

游玩测试不是在发布前只做一次的事情。

当你引入新机制、修改引导流程、调整难度或添加新内容时，都要测试。即使是小规模的游玩测试，也能帮助你在问题影响更多玩家之前发现重要问题。

**目标不是证明你的游戏可行，而是发现如何让它变得更好。**

## 资源

* [Scene Feedback 模块和安装指南](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)
* [Decentraland Discord](https://discord.gg/decentraland)
* [Decentraland 活动](https://decentraland.org/events/)
* [RITE 方法：快速迭代测试与评估](https://handbook.gitlab.com/handbook/upstream-studios/experience-research/rite/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/ti-yan-she-ji/playtests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
