> 将可穿戴物上传到编辑器的指南

# 上传可穿戴物

当你导出可穿戴物品后，需要将其上传到构建器中。本说明将介绍上传可穿戴物品的流程。

## 上传你的文件

请记住，在上传文件之前，你需要先创建一个合集。如果你不知道怎么做，请查看 [创建合集](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/)。要上传你的可穿戴物品，只需将文件拖放到 ***新建物品*** 窗口中，或者浏览你的电脑。系统会自动检测该文件是表情还是可穿戴物品。 **请记住，合集的最大文件大小为 3MB**.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

上传文件时，系统会要求你选择身体形态、输入名称，并定义稀有度和类别。你还可以为可穿戴物品添加缩略图。

### **身体形态**

为了确保你的可穿戴物品能被目标头像穿戴，你需要为每种身体形态分别上传独立的 GLB 文件。如果你的可穿戴物品有两个独立版本，一个用于男性，一个用于女性，你可以在上传过程中先添加其中一个版本，之后再使用编辑器添加另一个。如果你的可穿戴物品设计为男女通用，请确保上传一个同时适配男性和女性版本的单个 GLB 文件。

![](/docs/_remote/9c4ad1dbfd25f7f986e2b0d5.webp)

### **稀有度**

请选择你的物品稀有度。

![](/docs/_remote/a57649e01874c1a4789f0456.webp)

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

### **类别**

可穿戴物品会根据其所修改的头像部位被组织到不同的类别中。请为你的物品选择合适的类别。

![](/docs/_remote/ac233f1f8c3b34463031b787.webp)

### **自定义缩略图**

你可以点击相机图标并浏览你的电脑，添加自己的自定义缩略图。 **缩略图必须是 256px 的正方形 .png 文件，且背景透明。** 任何缩略图没有透明背景的合集都不会被审核委员会接受。

![](/docs/_remote/5a563c26949da7b96f8c2028.webp)


> [!WARNING]
> ⚠️ 为你的可穿戴物品制作一张优秀的渲染图，对于让它在市场中更吸引潜在用户至关重要。 **务必避免添加除可穿戴物品本身之外的任何图形，因为这可能会导致审核委员会拒绝它。**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### **弹簧骨骼**

如果你的可穿戴物品包含名称中带有 `springbone` 的骨骼，构建器会自动检测它们并显示一个 **弹簧骨骼** 配置面板。在此面板中，你可以为每条弹簧骨链调整物理参数，如刚度、重力和拖拽。详见 [弹簧骨骼](/docs/creator/wearables-and-emotes/wearables/spring-bones/) ，了解如何在模型中设置弹簧骨。

### **属性**

在缩略图下方，你会看到可穿戴物品的属性、模型三角形数量、材质数量和纹理数量。

![](/docs/_remote/c1c0778d6b438f3e33e8c6cf.webp)

## **上传嘴巴、眼睛和眉毛**

嘴巴、眼睛和眉毛类别在编辑器中的行为不同，因为这些只是 .png 文件。要上传它们，只需将 png 文件拖放进去，作为带透明背景的图片（256X256 像素）。嘴巴会自动根据肤色着色，眉毛也会根据发色着色。


> [!WARNING]
> 如果你希望资源带有遮罩，使嘴巴或眉毛的某一部分不受着色影响，请上传一个同时包含 png 和 mask 文件的 zip 文件。请记住，mask 文件必须带有“\\\_mask”后缀才能生效。


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

在完成可穿戴物品上传后，你会看到一个类似这样的界面，其中显示你合集中的物品。

![](/docs/_remote/b1d31fcc4dddce1716af7bc8.webp)

## **设置可穿戴物品的价格**

当你发布合集并通过审核后，就可以为你的合集启用销售。

![](/docs/_remote/dd9d2d46268767048dd15f53.webp)

你可以点击 ***挂牌出售***&#x6765;设置可穿戴物品的价格。所有这些都可以随时编辑，所以如果你之后想修改也不用担心。价格以 MANA 计价。请记住，当你铸造可穿戴物品时，它们会直接在 Matic/Polygon 上铸造。当用户购买你的物品时，交易将以 Matic/Polygon MANA 完成。

你也可以 ***设为免费***，这意味着价格将设为 0 MANA，受益地址将为空。请注意，将其设为免费（主销售）并不会阻止它在二级市场上以任何价格出售。

不要忘记设置受益地址，也就是接收你销售所得 MANA 的地址。你可以使用任何你喜欢的 Ethereum 地址。若要自动填入你当前登录的地址，请点击 ***我是受益人。***

![](/docs/_remote/f25c9cc68be82373f6fe392e.webp)

将物品上架销售后，你会返回到合集中的可穿戴物品列表。点击该物品后，你会看到它的基本信息。点击按钮 ***预览*** 即可在编辑器中查看它。

![](/docs/_remote/528700c2d810068ed39ccdb2.webp)

## 编辑器

当你点击 ***预览***&#x540E;，编辑器就会打开。你可以编辑可穿戴物品的所有信息，也可以添加新的信息，例如描述、标签和覆盖项。你还可以在这里为你的可穿戴物品添加其他身体形态的版本。

![](/docs/_remote/c21cb9360544651f12054903.webp)

## **说明**

这是一段简短说明，用于描述你的物品，并会显示在市场中。

## **覆盖项**

覆盖项决定你的物品会隐藏哪些可穿戴类别或头像身体部位。例如，一个带有附加头发的帽子可能需要隐藏 *头发* 类别。深海潜水员头盔可能需要隐藏耳环、眼镜、王冠等头部配饰，因为这些都不会可见。每个覆盖项可以选择多个选项。

* **基础身体**：这指的是头像的核心部位，例如 *头部* 和 *手部*。例如，如果你正在创建一个 **手部穿戴** 物品，比如机器人机械手，你很可能需要隐藏 *手部* 以防止重叠和穿模。
* **穿戴物**：这包括其他可穿戴类别。你可以隐藏多个类别。有关每个类别以及物品如何交互的更多详情，请参阅 [**创建可穿戴物品**](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).


> [!WARNING]
> 注意：你选择的覆盖项将作为可穿戴物品的建议默认设置。不过，用户可以从背包中自定义要隐藏或显示哪些可穿戴物品。


## **标签**

标签只是一些描述性词语，用户在搜索或筛选物品时可以使用。它们对于比赛或活动很有帮助！

## **VRM 导出权限**

启用此属性后，允许你物品的拥有者将其包含在 VRM 头像导出中，以便他们可以在 Decentraland 之外展示它。

## **轮廓**

轮廓开关用于控制你的可穿戴物品是否兼容 Decentraland 的轮廓渲染系统（用于中/高画质设置）。默认情况下，所有新可穿戴物品都已启用此功能。

如果你的可穿戴物品具有以下情况，应该禁用此选项：

* 模型中已手动创建轮廓
* 使用了反向法线
* 应用轮廓效果时出现视觉瑕疵

大多数可穿戴物品在默认设置（启用）下都能正常工作。只有当你注意到可穿戴物品存在与轮廓相关的视觉问题时，才应将其禁用。一旦禁用轮廓，你需要点击 ***保存***.

![](/docs/_remote/69f438abb59b36e0dc7bcc84.webp)

## **添加另一个版本**

如果你的可穿戴物品对男性和女性有不同版本，你需要再上传一个文件。到目前为止，你只上传了一个。在示例中，它是 Krampus Sweater 的女性版本。要添加另一个版本，请点击右上角旁边的三个点（*…*） ***属性*** 并选择 ***添加男性/女性*** 版本。在下面的示例中，我们需要男性版本。

![](/docs/_remote/21001d89e30650778f2bd9b8.webp)

点击后，你会看到“添加男性/女性版本”窗口，将另一个版本文件拖放进去上传。上传完成后，会弹出另一个窗口显示该可穿戴物品，如果一切正常，只需点击保存。

![](/docs/_remote/983512d9a30c68af5edb426d.webp) ![](/docs/_remote/7debe500c2fea74349f7fa23.webp)

## **预览**

要预览你的可穿戴物品，将鼠标悬停在左上角的可穿戴物品图标上，然后点击眼睛符号。

![](/docs/_remote/4634ace580b72aed8b781aed.webp)

点击左下角的图标后，你就可以编辑头像。如果你有可穿戴物品的男性和女性版本，这非常有用，因此请务必在编辑器中检查两个版本的外观，测试不同的表情动作以识别是否存在蒙皮问题，并搭配其他可穿戴物品查看它与不同服装的匹配效果。完成可穿戴物品编辑后，点击 ***保存***.

![](/docs/_remote/cec3d31a2ab318de454d52a0.webp)

## **在世界中测试**

即使已经在编辑器中测试过可穿戴物品，仍然很重要去检查它在 Decentraland 中实际会如何显示和表现。要在世界中测试，请前往合集选项卡。选择所需的合集并点击按钮 ***在世界中查看***.

![](/docs/_remote/4a233b404952959b105fa854.webp)

点击后，会出现以下弹窗。选择 ***空地块*** 会把你传送到内容不多的地方，这样加载会更快。选择 ***创世广场*** 会带你前往主广场。

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

一旦你选择“在世界中查看”，浏览器会打开一个新标签页，你会看到这条消息。

![](/docs/_remote/a95471fb4beb838e7547a9bf.webp)

点击 ***TRUST PEER-TESTING.DECENTRALAND.ORG*** 并会弹出一个窗口。只需点击“打开 Decentraland”。要测试你的可穿戴物品，前往背包并将其装备上。

![](/docs/_remote/bd6c470a523472534ab4282f.webp)

## **发布前**

请确保正确设置价格，添加一段好的描述，并再次检查所有信息和设置是否正确。如果你已填写所有必要信息，你会看到 ***完成*** 作为你的物品状态。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chuan-dai-wu-yu-biao-qing-dong-zuo/guan-li-he-ji/uploading-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
