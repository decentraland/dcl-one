> Decentraland 移动应用的常见问题以及获取帮助的途径。

# 故障排查

如果 Decentraland 移动应用中有内容无法正常运行，本页涵盖了最常见的情况以及获取更多帮助的方法。

## 应用无法登录

* 请确保你已连接互联网。
* 尝试关闭并重新打开应用。
* 如果你是在桌面端注册的，但找不到自己的账户，请确认你使用的是最初注册时相同的登录方式。

## 场景无法加载或运行缓慢

* 有些场景较大，下载需要一些时间——请稍等片刻，尤其是在移动网络上。
* 尝试切换到其他场景再切回来；这会重置加载。
* 如果某个特定场景一直运行缓慢，创作者可能还没有将其优化为适配移动端。你可以尝试在 Wi‑Fi 连接下打开，或者使用更新一些的设备。

## 我看不到屏幕上的控制按钮

* 请确保你已进入某个场景——控制按钮只有在加载完成后才会显示。
* 如果控制按钮已经淡出，可在屏幕上轻点一下以唤醒它们。

## 场景的界面与控制按钮重叠

移动端客户端会预留屏幕左侧和右下角用于摇杆、聊天、个人资料以及交互按钮。如果某个场景自身的界面覆盖了这些区域，说明创作者尚未为移动端更新它——他们正在处理。

## 在哪里获取帮助

* 在任意 [decentraland.org](https://decentraland.org/) 页面的右下角，点击聊天图标即可联系支持团队，或者前往 [decentraland.org/help](https://decentraland.org/help).
* 加入 [Discord](https://dcl.gg/discord) 上的讨论——那里有专门用于移动端反馈和支持的频道。
* 关注 [@decentraland](https://twitter.com/decentraland) 以获取状态更新。

## 相关

* [入门](/docs/player/player-zh/yi-dong-ying-yong/getting-started/)
* [控制](/docs/player/player-zh/yi-dong-ying-yong/controls/)
* [联系与支持](/docs/player/player-zh/chang-jian-wen-ti/contact-and-support/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/yi-dong-ying-yong/troubleshooting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
