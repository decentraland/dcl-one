> 从 Creator Hub 或 CLI 在真实移动设备上预览你的场景。

# 在移动设备上预览

你可以直接在 Decentraland 移动应用中，从 Creator Hub 或命令行预览你的场景。这是确认你的 UI、输入处理和性能在真实设备上表现良好的唯一可靠方式。

## 前提条件

* 安装在你手机上的 Decentraland 移动应用——请参见 [下载链接](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/overview/#get-the-mobile-app).
* 手机和你的开发机器必须处于 **同一个局域网** （Wi-Fi）。预览内容由你的电脑提供；二维码会链接到一个局域网 URL。

## 选项 A——从 Creator Hub

1. 在 Creator Hub 中打开你的场景。
2. 点击 **播放** 用于打开的按钮 **播放选项**，然后选择 **显示移动端二维码**.
3. 使用手机相机扫描显示的二维码。该链接会打开 Decentraland 移动应用并加载你的场景预览。点击弹窗周围的深色区域可将其关闭。

<figure><img src="/docs/_remote/c6aa2bdbe9bdd432e1e7e96d.webp" alt="Creator Hub Play Options menu with the Show QR Code for Mobile option"><figcaption><p>Creator Hub 的“播放选项”菜单中的“为移动端显示二维码”选项。</p></figcaption></figure>

## 选项 B——从命令行

在你的场景项目根目录下运行：

```bash
npm run start -- --mobile
```

CLI 会在终端中打印一个指向你场景 LAN URL 的二维码。用手机扫描它即可在 Decentraland 移动应用中加载该场景。

<figure><img src="/docs/_remote/ce46ee596043fea52191ae3d.webp" alt="QR code printed by the Decentraland CLI for mobile preview"><figcaption><p>由 <code>npm run start -- --mobile</code>打印的二维码。用手机扫描它即可在 Decentraland 移动应用中打开该场景。</p></figcaption></figure>


> [!NOTE]
> **💡 提示**：当你传入 `--mobile`时，桌面端浏览器也不会同时启动。如果你想同时在两者上测试，请运行 `npm run start` 在一个终端中用于桌面端的 `npm run start -- --mobile` 在另一个终端中用于移动端的


## 热重载

和桌面预览一样，当你更改场景文件时，移动端预览会自动重新加载。每次编辑后你都不需要重新扫描二维码。

## 故障排除

* **二维码无法打开应用** ——请确保已安装 Decentraland 应用，并且你已经在该设备上至少打开过一次。
* **手机无法连接到预览服务器** ——请确认手机和开发机器连接在同一个 Wi-Fi 网络中。某些公司网络会隔离客户端；如果是这种情况，请使用个人热点或其他网络。
* **场景在手机上能加载，但看起来与桌面端不同** ——这是正常的。请借此时机验证你的 [安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/), [UI 尺寸](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/ui-best-practices/#sizing)，和 [输入绑定](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/input-on-mobile/).

## 相关

* [预览你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/)
* [使用 CLI](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/using-the-cli/)
* [在移动版 Discover 中获得推荐](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/fa-bu/get-featured/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/preview-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
