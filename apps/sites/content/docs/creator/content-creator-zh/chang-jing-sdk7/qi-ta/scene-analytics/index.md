> 跟踪已发布场景的玩家访问、参与度和留存率

# 场景分析

作为创作者，跟踪玩家访问以及玩家与你的场景互动的方式非常有价值。与其凭猜测，你可以获得真实数据，并在迭代设计时做出明智的决定。

该 [创作者中心](https://decentraland.org/create) 包含一个 **分析** 部分，其中包含你发布的或你有权限访问的每个场景的详细指标。你无需向场景中添加任何内容；这些数据会自动为所有已发布场景收集。


> [!NOTE]
> **📔 注**: 分析数据每天更新一次。只有场景的所有者和运营者可以查看其数据。


## 查看你场景的分析数据

1. 打开创作者中心并使用你的账号登录。
2. 选择 **分析** 顶部导航栏中的选项卡。你会看到你拥有或运营的所有已发布场景列表，并显示每个场景的核心指标。
3. 点击某个场景以打开其详情页，完整指标集会按选项卡组织显示。

数据按场景跟踪。如果一个 World 包含多个场景，每个场景都会在列表中作为单独条目显示，并拥有自己的指标。

## 场景列表

该列表中每个场景占一行，包含以下字段：

* **唯一访问**: 访问过你场景的不同用户，每人只计一次。
* **新用户**: 首次访问你场景的用户。
* **第7日留存**: 在首次访问后的 7 天内至少返回过一次你场景的新用户占比。
* **收入**: 在你的场景中赚取的 MANA。
* **中位游玩时长**: 每位用户每天在你的场景中停留的中位时间：一半的日访问更长，一半更短。

你可以对列表进行排序或按名称搜索，并将你最关注的场景固定到顶部。

## 详情页

点击列表中的任意场景以打开其详情页。指标旁边会有一张卡片，显示该场景的一般信息：点赞率、访问权限是公开还是私密、发布到哪个 World、最后一次发布者以及最后更新时间。你还可以直接从此页面复制场景 URL、进入场景，或为其创建活动。

使用 **日期范围** 选择器来计算最近 7、30 或 60 天的指标，以及 **导出分析数据** 按钮将数据下载为 .csv 文件。

按周的图表，例如留存队列、每周活跃用户和社交参与访客，无论你选择哪种日期范围，始终涵盖最近 16 个完整周。

尚无数据的指标会显示短横线（`-`）而不是数字。

这些指标分为四个选项卡。

### 概览

你场景的增长、参与度和用户留存概览。

* **总访问量**: 你场景的每次访问，包括同一用户的重复访问。
* **唯一访问**: 访问过你场景的不同用户，每人只计一次。
* **新用户**: 首次访问你场景的用户。
* **同时在线用户**: 同时在你场景中的最高用户数。
* **收入**: 在你的场景中赚取的 MANA。
* **第7日留存**: 在首次访问后的 7 天内至少返回过一次你场景的新用户占比。
* **中位游玩时长**: 每位用户每天在你的场景中停留的中位时间：一半的日访问更长，一半更短。
* **挂机时间**: 用户在不进行交互的情况下在你场景中停留的平均时间。
* **桌面端 / 移动端**: 从桌面端或移动端客户端访问你场景的用户。

### 访问

有多少用户访问了你的场景，以及其中有多少是新访客或回访者。

* **最近 60 天的唯一访问**: 按平台细分的唯一访客（所有平台、桌面端和移动端）。跨平台用户会去重：在桌面端和移动端都游玩的人，在所有平台总数中只计一次，在每个平台总数中也各计一次。
* **每周活跃用户**: 每周的唯一活跃用户。
* **每周用户流向**: 每周用户在以下几类中的分布：
  * **新用户**: 首次访问你场景的用户。
  * **回访用户**: 上周访问并在本周回来的用户。
  * **重新激活用户**: 曾停止访问并在本周回来的用户。

### 互动

用户进入你的场景后如何进行互动。

* **平均每日游玩时长**: 每位用户单次访问在你的场景中停留的平均时间，并附带按周图表及相较上一周的变化。
* **平均每周游玩时长**: 每位用户每周在你的场景中停留的平均时间。
* **社交互动**: 三个社交信号，既显示为每周总量，也显示为当周访客中执行各项操作的占比：
  * **已发送消息**: 你场景中的访客发送的聊天消息。
  * **已使用表情**: 你场景中的访客使用的表情。
  * **新好友关系**: 你场景中访客之间被接受的好友请求。

### 留存

首次会话后返回的用户百分比。

* **平台**: 你的 60 天滚动平均留存，对比桌面端和移动端。
* **按队列周划分的第7日留存**: 按用户首次进入你场景的周进行分组，统计其在首次访问后的 7 天内至少返回过一次的用户百分比。图表绘制了 20% 基准线，作为健康参考值。
* **每周流失率**: 上周活跃用户中本周未返回的百分比。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qi-ta/scene-analytics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
