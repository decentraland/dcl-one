> Decentraland 桌面客户端中可用、但尚未在移动应用中支持的功能，以及已知的跨平台不一致之处。

# 缺失功能


> [!NOTE]
> 此页面跟踪 Decentraland 桌面端（Unity）客户端与移动应用之间的功能差距。其来源于 [godot-explorer 功能对等跟踪器](https://github.com/decentraland/godot-explorer/issues/2402) ，并会定期更新。ETA 为估计值，可能会变更。
> 
> 上次审阅： **2026 年 10 月** （移动应用 v1.15.0）。


## 最近已上线

自移动应用起，这些已不再是差距 **v1.15.0**，它将于 2026 年 10 月推送到应用商店：

* [`stopEmote` 受限操作](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/tou-xiang/avatar-animations/#animate-only-the-upper-body)。场景启动的上半身表情在玩家离开场景时也会暂停，返回时继续，场景卸载时停止 — v1.15.0
* [`补间` `MoveRotateScale` mode](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/#simultaneous-tweens) — v1.15.0
* 其他玩家的位置会像本地玩家一样相对于场景报告给场景（`getPlayer({ userId }).position`, `onEnterScene`，附加在其他玩家身上的物品）— v1.15.0
* 玩家移动，首次 Unity 对等实现：胶囊体大小、台阶高度、跳跃弧线与重力、奔跑速度和空中控制 — v1.15.0
* 从玩家向下发出的射线会报告正确的 `hitDistance` ，即在斜坡几何体上的距离 — v1.15.0
* 上半身表情会被其他移动端玩家识别为上半身表情 — v1.15.0

## 移动端缺失的 SDK 功能

* [自定义字体（`fontSrc` 在 `TextShape`, `标签`, `按钮`, `Input` 和 `下拉菜单`)](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#custom-fonts) — 移动应用会忽略 `fontSrc` 并渲染回退字体 `font` — 暂无 ETA
* [`头像名牌` （场景编写的、位于玩家名牌上方的标签）](https://github.com/decentraland/godot-explorer/issues/2811) — 暂无 ETA
* [补间序列（`TweenSequence`)](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/move-entities/#tween-sequences) — 尚未实现；串联补间不会播放动画 — 暂无 ETA
* [SDK7 UiBackground 使用九宫格切片而非拉伸](https://github.com/decentraland/godot-explorer/issues/2060) — 暂无 ETA
* Draco 网格压缩（`KHR_draco_mesh_compression`）— 移动应用没有运行时 Draco 解码器。经 Draco 压缩的 `.gltf`/`.glb` 模型仅会从场景的优化资源中加载，资源服务器已在那里将其解压。若场景没有优化资源，例如在本地预览中，它们会在没有错误提示的情况下加载失败。为保险起见，请导出不使用 Draco 压缩的模型。此情况的错误对话框已在 [#2432](https://github.com/decentraland/godot-explorer/issues/2432) — 暂无 ETA
* 智能物品—移动端不受官方支持

## 移动端没有的桌面客户端功能

* [近距离语音聊天](https://github.com/decentraland/godot-explorer/issues/888) — 2027 年第一季度
* [世界内指向](https://github.com/decentraland/godot-explorer/issues/1736) — 暂无 ETA
* [名牌颜色更改](https://github.com/decentraland/godot-explorer/issues/1684) — 暂无 ETA
* [社群](https://github.com/decentraland/godot-explorer/issues/656) — 2027 年第一季度
* [照片图库](https://github.com/decentraland/godot-explorer/issues/680) — 2026 年 12 月
* [社区直播](https://github.com/decentraland/godot-explorer/issues/676) — 2027 年第一季度
* [个人资料徽章](https://github.com/decentraland/godot-explorer/issues/678) — 2026 年 12 月
* [每日任务](https://github.com/decentraland/godot-explorer/issues/682) — 暂无 ETA
* [聊天回应](https://github.com/decentraland/godot-explorer/issues/1824) — 2027 年第一季度
* [聊天自动翻译](https://github.com/decentraland/godot-explorer/issues/2260) — 2027 年第一季度
* [聊天：直接消息](https://github.com/decentraland/godot-explorer/issues/1120) — 2027 年第一季度
* [移动端 DCL Cast 支持](https://github.com/decentraland/godot-explorer/issues/1881) — 暂无 ETA
* [不支持 YouTube / Google Drive 视频 URL](https://github.com/decentraland/godot-explorer/issues/2081) — 为符合商店规范，移动端受限
* [背包中的装扮槽位](https://github.com/decentraland/godot-explorer/issues/1625) — 2026 年 12 月
* [向其他玩家赠送可穿戴物品](https://github.com/decentraland/godot-explorer/issues/2957) — 2026 年 12 月
* [同一账号在另一设备登录时显示“会话已结束”通知](https://github.com/decentraland/godot-explorer/issues/2880) — 暂无 ETA
* [发现分类（按类别浏览体验）](https://github.com/decentraland/godot-explorer/issues/3037) — 暂无 ETA

## 跨平台不一致

* **玩家移动和碰撞行为与 Unity 对比**
  * [坡面、边缘、墙壁、落地和移动平台](https://github.com/decentraland/godot-explorer/issues/2852) 和 [滑翔行为](https://github.com/decentraland/godot-explorer/issues/2854) — 修复审查中，暂无 ETA
  * [厚度小于 1 厘米的场景碰撞体只会从上方发生碰撞，而不可见阻挡物和某些 SDK 碰撞层不会阻挡玩家](https://github.com/decentraland/godot-explorer/issues/2853) — 暂无 ETA
  * 生成、落地和跳跃动画，移动混合（[#1553](https://github.com/decentraland/godot-explorer/issues/1553), [#1554](https://github.com/decentraland/godot-explorer/issues/1554), [#2855](https://github.com/decentraland/godot-explorer/issues/2855), [#2856](https://github.com/decentraland/godot-explorer/issues/2856)）— 修复审查中，暂无 ETA
* [某些来自其他客户端的表情在移动端无法播放：来自 Bevy Web 玩家触发的场景表情，以及通过 LiveKit 到达的 Unity 玩家表情](https://github.com/decentraland/godot-explorer/issues/2986) — 修复审查中，暂无 ETA
* [在 Creator Hub 的移动预览中，桌面测试者看不到移动端玩家的头像](https://github.com/decentraland/godot-explorer/issues/2773) — 修复进行中，暂无 ETA
* [头像牙齿渲染为深灰/灰色而不是白色](https://github.com/decentraland/godot-explorer/issues/1994) — 暂无 ETA

## 输入/平台限制

* **仅支持触摸输入** — 不支持鼠标悬停状态、键盘快捷键或右键点击。
* **手势支持有限** — 缩放捏合手势（在第一人称和第三人称摄像机之间切换）已在 v1.14.0 中加入；目前未计划其他手势。

## 报告缺失的功能

如果你遇到此处未列出的限制，请 [报告它](/docs/creator/content-creator-zh/chang-jing-sdk7/tiao-shi/report-bug/) 以便我们记录并优先处理。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/missing-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
