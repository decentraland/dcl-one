> 将表情动作上传到编辑器的指南

# 上传表情动作

一旦导出你的表情动作，你就需要将其上传到构建器。本文档将介绍上传表情动作的流程。

## **上传你的文件**

请记住，在上传文件之前，你需要先创建一个合集。如果你不知道怎么做，请查看 [创建合集](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/)。要上传你的表情动作，只需将文件拖放到 ***新建物品*** 窗口，或浏览你的电脑。它会自动检测文件是否包含任何动画，并将其识别为表情动作。

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

拖放你的动画文件以上传它。

系统会要求你为表情动作输入名称，定义其稀有度、类别和播放模式。缩略图下方会显示文件中的动画片段数量、动画时长（秒）、帧数长度以及帧率。

![](/docs/_remote/e376a1e332918af541d1a910.webp)

## **使用 .zip 文件上传表情动作**

如果表情动作带有声音（*mp3* 或 *ogg*）则必须与 `.glb`一起压缩。然后，只需拖放 `.zip` 到构建器中。另外，还可以添加一个 `.json` 文件，并与其他资源一起放在同一个 `.zip` 以添加名称、描述、稀有度、类别、播放模式和/或标签。以下是各项定义：

* `名称`：表情动作的名称
* `描述`：你的表情动作描述（总计不超过 64 个字符，包括空格）
* `类别`：表情动作的类别（"dance"、"stunt"、"greetings"、"fun"、"poses"、"reactions"、"horror"、"miscellaneous"）
* `稀有度`：物品的稀有度（"unique"、"mythic"、"legendary"、"epic"、"rare"、"exotic"、"uncommon"、"common"）
* `播放模式`：单次或循环动画（"simple"、"loop"）
* `标签`：用于在市场中轻松查找的标签。

要将这些定义添加到表情动作中，只需创建一个文本文件，并将其命名为 **emote.json** 并按示例添加以下几行：

```
{
  "name": "网球击球",
  "description": "让我看看你会打网球",
  "category": "fun",
  "rarity":"epic",
  "play_mode": "simple",
  "tags":["tennis", "emote", "shot"]

}

```

这样，构建器会读取所有 .json 信息并自动将其应用到表情动作中。

**稀有度**

请查看稀有度图表，位于 [此处](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/#rarity).

## **类别**

选择最能描述你的表情动作的类别。

* 舞蹈
* 特技
* 问候
* 趣味
* 姿势
* 反应
* 恐怖
* 其他

## **播放模式**

目前有两种播放模式：

* ***播放一次***：表示你的表情动作只会播放一次。之后，头像将返回到 *待机* 状态。
* ***循环***：表情动作将持续循环播放，直到用户输入其他动作。

## **自定义缩略图**

对于表情动作，你无需上传任何图片，因为编辑器内置了创建缩略图的工具。只需选择最能代表该动作的帧。

人们应该能够通过缩略图识别动画的内容。如果正面镜头不够好，试着旋转模型、放大或缩小、上下平移，或者从你的片段中挑选任意一帧。选择最佳镜头非常重要！

![](/docs/_remote/9efbcbc5b36b6a3edfbcf38b.webp)

旋转、放大或缩小、上下平移。使用这些工具为你的动画获取最佳镜头！

## 编辑器

一旦设置好缩略图，编辑器就会打开。你可以在这里检查动画是否播放正常，因为一个 3D 头像将会执行它。

![](/docs/_remote/58fef5ae57e3864b91e06697.webp)

点击左下角的图标即可编辑头像。点击右下角的圆柱体图标，以检查你的动画是否保持在高度和空间边界内。

![](/docs/_remote/c32c351247d541f21bafadb2.webp)

左下角的图标可用于编辑头像。

![](/docs/_remote/f4f828d6e5577c46357b79cf.webp)

圆柱体图标会显示动画的边界。

你还可以编辑表情动作的名称、缩略图、类别、稀有度和播放模式，并为动画添加描述和标签。点击 ***保存*** 完成后。

* **描述：** 这是一段简短说明，用于描述你的物品，并会显示在市场中。
* **实用性** 描述可穿戴物品或表情动作在世界中的用途。
* **标签：** 标签只是一些描述性词语，用户在搜索或筛选物品时可以使用。它们对于比赛或活动很有帮助！

![](/docs/_remote/ae584da404403aeb0e47bcee.webp)

## 在世界中测试

即使在编辑器中测试过动画，也很重要去查看它在 Decentraland 中实际会呈现和表现成什么样。要在世界中测试，请前往 Collections 选项卡，选择所需的合集，它会显示其中包含的所有物品。点击“在 Decentraland 中查看”。

\
\\

![](/docs/_remote/7fc9c8d40af1b921993b8bfb.webp)

\
\\


> [!NOTE]
> 系统会让你选择是在空地块还是在 Genesis Plaza 中测试它。选择空地块会将你传送到一个内容不多、加载更快的地方。选择 Genesis Plaza 会将你带到主广场。


\
\\

![](/docs/_remote/d1c728c7c0ffee7fe42730da.webp)

\
\\


> [!NOTE]
> 选择后，浏览器中会打开一个新标签页，提示你即将使用自定义 Catalyst。点击 TRUST PEER-TESTING.DECENTRALAND.ORG 继续。


\
\\

![](/docs/_remote/5e0eef48997d27eadb27149b.webp)

\
\\


> [!NOTE]
> 之后会弹出一个窗口，询问是否打开 Decentraland Explorer。探索器运行后，点击 Jump Into Decentraland。


\
\\

![](/docs/_remote/0518f3adcaa8974ae07124e4.webp)

\
\\


> [!NOTE]
> 进入世界后，按 I 打开背包，然后在表情动作选项卡中选择要测试的表情动作。


\
\\

![](/docs/_remote/80aeb1e924a01a378e86f206.webp)

\
\\

## **发布前**

请务必添加一个好的描述，并确认所有信息和设置都正确。也要再次检查缩略图。如果你已填写所有必要信息，你会看到物品状态显示为 Ready to Submit。现在你的表情动作就可以发布了！

![](/docs/_remote/e404c18c57fe014dd11e6091.webp)

Ready to Submit 状态表示你的文件已准备好发布！


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
