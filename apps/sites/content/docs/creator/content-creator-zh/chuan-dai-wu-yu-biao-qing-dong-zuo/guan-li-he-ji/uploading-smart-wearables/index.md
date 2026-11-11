> 将智能可穿戴物上传到编辑器的指南

# 上传智能可穿戴物

在使用以下工具生成你的智能穿戴设备后 [SDK7](https://github.com/decentraland/docs/blob/main/creator/sdk7/projects/smart-wearables.md)，下一步是将其上传到构建器。本文件将说明如何上传、发布并将你的智能穿戴设备上架出售。

## 上传你的文件

请记住，在上传文件之前，你需要先创建一个合集。如果你不知道如何操作，请查看 [创建合集](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/manage-collections/creating-a-collection.md)。要上传你的智能穿戴设备，请将文件拖放到……上 ***新建物品*** 窗口中，或浏览你的电脑。系统会自动检测该文件是否为智能穿戴设备。 **请记住，合集的最大文件大小为 3MB**.

![](/docs/_remote/48697c84cb446eeea05d89d3.webp)

上传文件时，系统会要求你上传一个视频展示。

## 上传你的视频展示

这个视频展示将显示在市场中，供买家了解智能穿戴设备的运行方式。

要上传你的视频展示，请拖放一个 **.mp4** 视频文件到……上 ***为你的智能穿戴设备上传一个视频*** 窗口中，或浏览你的电脑。 **请记住，视频文件最大大小为 4MB，最长时长为 15 秒**.

![](/docs/_remote/763eb735dd9262151b2d4576.webp)

上传视频展示后，系统会要求你输入名称并定义稀有度和类别。如果你愿意，也可以为智能穿戴设备添加缩略图。

### 稀有度

请选择你的物品稀有度。

![](/docs/_remote/5aa59b7826603f649de1aade.webp)

| 稀有度 | 物品数量    |
| --- | ------- |
| 独特  | 1       |
| 神话  | 10      |
| 异域  | 50      |
| 传奇  | 100     |
| 史诗  | 1,000   |
| 稀有  | 5,000   |
| 不常见 | 10,000  |
| 普通  | 100,000 |

### 类别

穿戴物会根据它们修改头像的哪个部位被归入不同类别。在这一步中，你必须选择合适的物品类别。

![](/docs/_remote/805bd4e946bcf6044e5f79e7.webp)

### 自定义缩略图

你可以点击相机图标，并从你的电脑中选择所需图片来添加缩略图。 **请确保缩略图为 256px 的方形 .png 文件格式，并且背景透明。** 请注意，策展委员会不会批准那些缩略图背景不透明的合集。

![](/docs/_remote/d45b65391e8cc81a9f4c7a8c.webp)


> [!WARNING]
> ⚠️ 拥有一张优质的穿戴物渲染图对于吸引潜在的市场用户至关重要。 **重要的是，避免添加除穿戴物本身之外的其他图形，因为这可能导致策展委员会拒绝它。**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### 更新视频展示

要更新你的视频展示，只需点击相机图标并在电脑中浏览查找该视频。 **请注意，允许的最大文件大小为 4MB，时长不应超过 15 秒**.

![](/docs/_remote/3375e0716ee39e30f5ccb5aa.webp)

### 属性

在缩略图旁边，你会看到穿戴物的属性，例如模型的三角形数量、材质数量以及纹理。

![](/docs/_remote/41d92d3af0ff7eebd17ea3cf.webp)

## 上传嘴巴、眼睛和眉毛

嘴巴、眼睛和眉毛类别在编辑器中的行为不同，因为这些只是 .png 文件。要上传它们，只需将 png 文件拖放进去，作为带透明背景的图片（256X256 像素）。嘴巴会自动根据肤色着色，眉毛也会根据发色着色。


> [!WARNING]
> 如果你希望资源带有遮罩，使嘴巴或眉毛的某一部分不受着色影响，请上传一个同时包含 png 和 mask 文件的 zip 文件。请记住，mask 文件必须带有“\\\_mask”后缀才能生效。


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

上传穿戴物之后，你最终会看到这样一个屏幕，显示你合集中的物品。

![](/docs/_remote/83043b56afafb5b81fe2be0f.webp)

## 设置你的智能穿戴设备价格

你可以通过点击……来设置你的穿戴物价格 ***设置价格***。所有这些内容都可以随时编辑，所以如果你以后想更改也不用担心。价格以 MANA 设置。请记住，当你铸造穿戴物时，它们会直接铸造在 Matic/Polygon 上。实际上，当用户购买你的物品时，交易将以 Matic/Polygon MANA 进行。

你也可以 ***设为免费***，这意味着价格将为 0 MANA，受益地址将为空。请注意，作为一级销售免费提供产品，并不妨碍它在二级销售中以任何价格出售。

不要忘记设置受益地址，也就是接收你销售所得 MANA 的地址。你可以使用任何你喜欢的 Ethereum 地址。若要自动填入你当前登录的地址，请点击 ***我是受益人。***

![](/docs/_remote/44c1580896b6f7d4ff6a5349.webp)

保存价格后，你将返回到合集中的穿戴物列表。点击该物品时，你会看到其基本信息。点击按钮 ***在编辑器中预览*** 即可在编辑器中查看。

![](/docs/_remote/8170f9434918babf3565876f.webp)

## 编辑器

当你点击 ***在编辑器中预览***，你将打开编辑器。你可以编辑智能穿戴设备信息，包括描述、标签和覆盖项。

![](/docs/_remote/cabb347fa07def0f5e919e48.webp)

## 说明

这是一段简短说明，用于描述你的物品，并会显示在市场中。

## 覆盖项

覆盖项决定你的物品会隐藏哪些可穿戴类别或头像身体部位。例如，一个带有附加头发的帽子可能需要隐藏 *头发* 类别。深海潜水员头盔可能需要隐藏耳环、眼镜、王冠等头部配饰，因为这些都不会可见。每个覆盖项可以选择多个选项。

* **基础身体**：这指的是头像的核心部位，例如 *头部* 和 *手部*。例如，如果你正在创建一个 **手部穿戴** 物品，比如机器人机械手，你很可能需要隐藏 *手部* 以防止重叠和穿模。
* **穿戴物**：这包括其他穿戴物类别。你可以隐藏多个类别。请查看 [**创建可穿戴物品**](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/wearables/creating-wearables.md) 以了解每个类别及物品交互的更多详情。


> [!WARNING]
> 注意：你选择的覆盖项将作为可穿戴物品的建议默认设置。不过，用户可以从背包中自定义要隐藏或显示哪些可穿戴物品。


## 标签

标签是用户可用于比赛或活动的描述性词语！

## 预览

要预览你的智能穿戴设备，请将鼠标悬停在左上角的穿戴物元素上，然后点击眼睛图标。

![](/docs/_remote/38a5124d67833aaa252218c1.webp)

点击左下角的图标，你可以编辑头像。如果你的穿戴物有男性和女性版本，这会很有用，因此请在编辑器中查看两个版本的外观，测试不同的表情动作以发现蒙皮问题，并混搭其他穿戴物以查看与其他衣服的搭配效果。完成穿戴物编辑后，点击 ***保存***.

![](/docs/_remote/2d3fb93f880674b537bb022c.webp)

## 在 Decentraland 中测试

即使在编辑器中测试过智能穿戴设备，仍然有必要检查它在 Decentraland 中的外观和行为。要在那里试用，请前往“合集”标签页。选择所需的合集并点击按钮 ***在 Decentraland 中查看***.

![](/docs/_remote/f98021fb76ec1a330e1a9f4a.webp)

点击……后 ***在 Decentraland 中查看*** 按钮后，将会出现以下弹窗。选择 ***空地块*** 会把你传送到内容不多的地方，这样加载会更快。选择 ***创世广场*** 会带你前往主广场。

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

一旦你选择了传送地点，浏览器中将打开一个新标签页，并显示此消息。

![](/docs/_remote/61eed76d8ed92cce5d514ea9.webp)

点击 ***切换到 Sepolia***，你的钱包会弹出一个提示，要求你切换网络。点击“切换网络”，新标签页将自动刷新。要测试你的智能穿戴设备，你可以前往背包并选择它。


> [!WARNING]
> 如果你尚未启用以太坊测试网网络，可以按照此 [MetaMask 文章](https://support.metamask.io/hc/en-us/articles/13946422437147-How-to-view-testnets-in-MetaMask).


![](/docs/_remote/91716ea1b880be3f761e1a9c.webp)

## 发布前

请确保正确设置价格，添加一段好的描述，并再次检查所有信息和设置是否正确。如果你已填写所有必要信息，你会看到 ***完成*** 作为你的物品状态。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
