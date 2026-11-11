> Decentraland iOS 和 Android 移动应用概览，包含下载链接以及让场景支持移动端的检查清单。

# 概览

Decentraland 现已可在移动端使用。玩家除了可以通过桌面客户端和网页外，还可以在 iOS 和 Android 设备上探索你的场景。作为创作者，你可以调整你的场景，使其在触控设备上看起来出色并运行良好。

![Decentraland 移动应用 — Genesis 城市](/docs/_remote/a10f68e6dd27dfa895f53969.webp)



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/FBr2gye3qh8" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=FBr2gye3qh8>">Watch on YouTube</a></p>



## 获取移动应用

* [下载 iOS 版（App Store）](https://apps.apple.com/app/decentraland/id6478403840?utm_source=docs&utm_medium=internal&utm_content=ios)
* [下载 Android 版（Google Play）](https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&pcampaignid=web_share&utm_source=docs&utm_medium=internal&utm_content=android)

## 面向移动端构建

### 参考

* [示例场景](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/sample-scenes/) — 由 Decentraland 团队构建并针对移动端游玩优化的开源场景。
* [缺失功能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/missing-features/) — 桌面客户端上可用、但移动应用尚不支持的功能。
* [硬件要求](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/hardware-requirements/) — 在移动端运行 Decentraland 的最低和推荐硬件配置。

### 开发

* [从代码中检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/) — 使用 `isMobile()` 按平台分支你的场景逻辑和 UI。
* [在移动端预览你的场景](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/) — 可通过 Creator Hub 或 CLI 直接在手机上预览。
* [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/) — 设备的硬件边距（刘海、主屏指示条）以及客户端自身控件所占区域；使用 `screenInset`.
* [移动端 UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/) — DO 和 DON'T、尺寸建议以及当前限制。
* [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/) — 适合触控的输入映射，以及哪些 `InputAction`s 应避免。
* [优化性能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/optimize-performance/) — 移动场景限制、如何在 Creator Hub 中预览它们，以及性能目标。

### 发布

* [在移动端 Discover 中获得推荐](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/fa-bu/get-featured/) — 移动端 Discover 板块的提交要求。
* [iOS 策展](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/fa-bu/ios-curation/) — iOS 内容策展如何运作，以及如何提交你的场景或世界以供审核。

## 快速检查清单

在发布一个应能在移动端良好运行的场景之前，请确保：

* [ ] 你已经在真实设备上预览过该场景 — 参见 [在移动端预览你的场景](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile/).
* [ ] 所有关键 UI 元素都保留在 [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/).
* [ ] 你的 UI 尺寸足够大，便于触控 — 请遵循 [UI 最佳实践](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/).
* [ ] 关键操作没有绑定到 `IA_SECONDARY` 或 `IA_ACTION_3`–`IA_ACTION_6` （即 `F` 和 `1`/`2`/`3`/`4` 按键），这些按钮在移动端默认隐藏，除非你的场景显示它们 — 参见 [移动端输入](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/).
* [ ] 你的场景的 [性能](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/optimize-performance/) 在中端手机上的高画质配置文件下应高于 90% — 参见 [硬件要求](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/hardware-requirements/) 参考设备。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
