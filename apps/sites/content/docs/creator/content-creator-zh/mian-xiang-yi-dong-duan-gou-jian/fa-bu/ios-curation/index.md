> 了解 iOS 内容审核如何运作，以及如何提交你的场景或世界进行审核。

# iOS 审核

为了遵守 [Apple 的 App Store 审核指南中关于用户生成内容（UGC）的规定](https://developer.apple.com/app-store/review/guidelines/#user-generated-content)，Decentraland 引入了一个 **人工审核流程** 适用于所有出现在以下位置的场景和世界： **iOS 应用的 Discover（发现）版块**.


> [!NOTE]
> 此审核流程仅适用于 iOS。Android 的 Discover（发现）版块和搜索仍会无任何限制地显示所有已发布内容。


## 运作方式

* 所有内容在出现在 iOS 应用的 Discover（发现）版块之前都必须经过人工审核。
* 未获批准的场景和世界将不会显示在 iOS Discover（发现）版块中。
* 关联到未获批准场景或世界的活动也会在 iOS Discover（发现）版块中隐藏。
* **所有场景仍可访问** 在 iOS 上可随时通过深度链接或 `/goto` 和 `/world` 聊天命令访问——筛选仅影响 Discover 的可见性。

## 提交你的场景或世界以供 iOS 审核

若要请求审核你的场景或世界并将其纳入 iOS Discover（发现）版块，请提交一份 **内容审核请求** 到 Decentraland Discord 服务器：

1. 加入 [Decentraland Discord](https://dcl.gg/discord).
2. 前往 **#rl-mobile-curation** 频道。
3. 发布你的请求，包括：
   * 场景或世界名称
   * 坐标或世界名称（例如 `0,0` 或 `MyWorld.dcl.eth`)
   * 内容的简要描述

提交内容将在 **24 小时内**审核完成后，或者如需进一步跟进，你会在该频道收到通知。

## 相关

* [获得推荐展示](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/fa-bu/get-featured/)
* [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/)
* [在移动端预览](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/fa-bu/ios-curation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
