> 在游玩过程中或通过反馈按钮、自助终端或区域收集玩家评分和评论，结果以 CSV 形式存储在场景 Storage 中。

# 场景反馈

<figure><img src="/docs/_remote/a0baa00d823b7604d1c33550.webp" alt="Coin Hunt participation invitation with Skip and Give feedback buttons" width="400"><figcaption><p>该演示邀请玩家选择是否参与。</p></figcaption></figure>

即使你无法观察玩家游玩，也能了解他们觉得哪些地方令人困惑、沮丧或愉快。这个可重复使用的模块包含一个寻币场景，展示如何在游玩过程中以及玩家选择提供反馈时收集反馈。

* [游玩共享测试版本（Zone）](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1)
* [安装与使用指南](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md)

## 项目信息

**维护者：** Decentraland

[查看源代码](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)

**许可：** 本目录中未标明。复用前请查阅上游仓库和维护者，了解适用条款。

## 通过 AI 代理添加

1. 确保 [Decentraland SDK Skills](https://github.com/decentraland/sdk-skills#updating-skills) 已安装并保持最新。
2. 告诉你的 AI 代理：“我想收集这个场景中玩家的反馈。”

该代理可以使用 Scene Feedback 技能在两个阶段提供帮助：

* **在试玩前：** 将问题、评分量表和时机调整为适合你的游戏，然后在你审阅方案后集成该模块。
* **在试玩后：** 再次使用该技能并结合你的 CSV 文件，识别反复出现的问题和偏好，审查证据并决定下一步要调查什么。你还可以请求一个 [可读的 HTML 报告](#example-ai-generated-feedback-report) ，其中包含分组答案和评分摘要。

想自己集成？请按照 [安装指南](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#add-it-to-your-scene).

## 该模块如何融入你的场景

由你决定何时邀请玩家、要问哪些问题，以及这些问题在游戏中的出现时机。该 [Zone 中的寻币演示](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1) 展示一种可能的设置：

1. **收集第一枚硬币。** 邀请会询问玩家是否愿意提供反馈：“嗨，我们是 Coin Hunt 团队！”如果接受，随后会出现一个问题：“找到这些硬币有多容易或多困难？”玩家选择一个评分，并可以添加评论。
2. **完成第一轮（五枚硬币）。** 同意参与的玩家会连续收到两个问题：下一个目标是否清晰，以及他们是否想继续游玩。
3. **随时提供反馈。** 任何访问者都可以使用 **留下反馈** 按钮、可点击的信息亭或标记区域来打开欢迎消息，随后出现一组问题，即使在拒绝游戏相关问题之后也可以。

选择玩家如何参与：

* **选择参与的玩家：** 通过屏幕提示邀请玩家，或让他们通过按钮、信息亭或标记区域打开反馈表单。
* **选定测试者：** 使用你的游戏逻辑通过钱包列表或其他规则选择测试者，然后使用 `feedback.enroll`。请参见 [选定测试者示例](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md#4-selected-testers-without-an-intro).
* **或者向任何人提问：** 在游戏过程中向访问者展示问题，无需预先邀请或测试者列表。玩家仍可拒绝。请参见 [参与设置](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#asking-participants-during-play-dynamic-approach).

<figure><img src="/docs/_remote/c600bed5d83d04aa544fb1e0.webp" alt="Example feedback form with five rating choices and an optional comment field" width="400"><figcaption><p>一个带有标注评分和可选评论的问题。</p></figcaption></figure>

## 主要特性

* **灵活的问题。** 选择现成的问题和评分量表，将它们调整为适合你的游戏，或创建你自己的问题。可使用带标注的评分、可选评论或仅文本回答。
* **参与奖励。** 用适合你游戏的奖励来鼓励参与。在该演示中，完成一个表单中的每个问题即可获得仅在本次访问中有效、仅你可见的 Playtest Contributor 名牌、+5 硬币分数，以及接下来五次拾取的双倍分数，每次访问仅可获得一次。任何评分都算数；评论仍为可选。
* **场景存储中的 CSV 结果。** 多人服务器会将每个回答与对应的问题及游戏时刻一起保存。下载这些文件即可在电子表格中查看。

## 将其适配到你的场景

使用 [集成方案](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md) 以将问题连接到你自己的游戏事件，包括立即邀请参与者以及在后续游戏过程中再提问。有关试玩规划和寻找玩家，请参见 [试玩](/docs/creator/sdk7/design-experience/playtests/).

## AI 生成的反馈报告示例

**该报告由 AI 代理生成，而不是由 Scene Feedback 模块生成。** 将导出的 CSV 文件交给你的代理，并使用 Scene Feedback SDK 技能对其进行分析并生成 HTML 报告。

此示例使用的是来自虚构场景的合成响应。你的代理可以根据你自己的试玩问题和数据来定制分析和报告。

<figure><img src="/docs/_remote/ad88dfb78877894462538f58.webp" alt="Synthetic racing-scene report showing a main finding, response counts and a rating distribution" width="600"><figcaption><p>使用虚构场景的合成响应制作的示例报告。</p></figcaption></figure>

{% file src="/files/1ccf5083a791aec9ae70f9377da5b11b60bae4b2" %}
下载示例报告，解压 ZIP 文件并在浏览器中打开 HTML 文件。
{% endfile %}


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/jiao-cheng-yu-shi-li/scene-management-feedback/playtest-feedback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
