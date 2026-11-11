> 过去一年中 SDK 和 Creator Hub 的变化，已筛选为影响创作者的改动。

# 发行说明

本页列出了以下内容的更改： [SDK](#sdk) 以及 [Creator Hub](#creator-hub) 在过去一年中会影响你构建场景方式的更改。构建工具、测试、跟踪和重构等内部工作不包括在内。每一项都链接到其在 GitHub 上的拉取请求，你可以在那里找到更多详细信息。

如需完整、未筛选的说明，请查看 [SDK 发布说明](https://github.com/decentraland/js-sdk-toolchain/releases) 以及 [Creator Hub 发布说明](https://github.com/decentraland/creator-hub/releases) 在 GitHub 上。


> [!NOTE]
> Creator Hub 发布说明汇总了来自 `@dcl/inspector` （场景编辑器）和 `@dcl/asset-packs` （智能物件）包的更改。这些更改会列在首次发布它们的 Creator Hub 版本下。


## SDK

### 7.31.0（2026 年 10 月 22 日）

* **自定义字体**：新的 `fontSrc` 属性可使用一个 `.ttf` 来自你场景中的字体文件，在 `TextShape`, `UiText`, `UiInput` 和 `UiDropdown` 以及 React ECS 上的 `Label`, `Button`, `Input` 和 `Dropdown`。参见 [自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/text/#custom-fonts) 和 [UI 中的自定义字体](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/ui_text/#custom-fonts). [#1656](https://github.com/decentraland/js-sdk-toolchain/pull/1656)
* **发布大型场景**: `npm run deploy` 会分批上传 50 MiB 或更大的场景，因此大约 200 MB 以上的场景也可以发布，而且浏览器中的部署页面会显示上传进度，并允许你取消。参见 [发布大型场景](/docs/creator/content-creator-zh/chang-jing-sdk7/fa-bu/publishing/#publish-large-scenes). [#1635](https://github.com/decentraland/js-sdk-toolchain/pull/1635)
* **修复**：本地预览只会为每个场景实体服务一次，因此跨越许多地块的场景在预览中会加载 [#1650](https://github.com/decentraland/js-sdk-toolchain/pull/1650).

### 7.30.0 和 7.30.1（2026 年 10 月 8 日）

* **玩家语言**: `getPlayerLanguage()` 返回玩家客户端设置的语言，格式为 BCP 47 标签，并且 `onPlayerLanguageChanged` 会报告会话中途发生的更改。参见 [获取玩家语言](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/runtime-data/#get-player-language). [#1622](https://github.com/decentraland/js-sdk-toolchain/pull/1622)
* **等待一个 Explorer 面板关闭**：新的 `openExplorerUiAndWait()` 位于 `@dcl/sdk/explorer-ui` 中的辅助函数会打开一个面板，并在玩家关闭后返回，支持可选的时间限制。参见 [等待玩家关闭面板](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/event-listeners/#wait-until-the-player-closes-a-panel). [#1543](https://github.com/decentraland/js-sdk-toolchain/pull/1543)
* **修复**：bundle 监视器会忽略 `node_modules` [#1640](https://github.com/decentraland/js-sdk-toolchain/pull/1640)；预览会在每次 watch 重建时重新检查编辑器场景 [#1644](https://github.com/decentraland/js-sdk-toolchain/pull/1644).

### 7.29.0（2026 年 9 月 15 日）

* **UI 模块之间的堆叠顺序**：一个 `zIndex` 渲染器选项决定哪个 UI 模块绘制在前。若没有它，模块将按其首次出现的顺序堆叠，主 UI 位于最底层。参见 [UI 模块之间的堆叠顺序](/docs/creator/content-creator-zh/chang-jing-sdk7/2d-ui/onscreen-ui/#stacking-order-between-ui-modules). [#1607](https://github.com/decentraland/js-sdk-toolchain/pull/1607)
* **修复**：移除指针事件处理程序现在会移除它创建的条目，因此被禁用的 UI 按钮不再宣告自己无法处理的点击，并且重新注册处理程序也不会再堆积重复条目。 [#1577](https://github.com/decentraland/js-sdk-toolchain/pull/1577)

### 7.28.0（2026 年 9 月 10 日）

* **指针事件使用摄像机距离**: `maxCameraDistance` 替代 `maxPlayerDistance` 在指针事件中。旧字段仍可使用，但已弃用。悬停提示和点击现在取决于摄像机距离物件有多远，这与玩家所见一致。 [#1560](https://github.com/decentraland/js-sdk-toolchain/pull/1560)
* **AvatarNametag 组件**：自定义显示在头像上方的姓名标签。 [#1600](https://github.com/decentraland/js-sdk-toolchain/pull/1600)
* **传送到某个 realm**: `teleportTo()` 可以指定目标 realm。 [#1601](https://github.com/decentraland/js-sdk-toolchain/pull/1601)
* **修复**：被禁用的 UI `Button` 不再使场景颜色变暗 [#1578](https://github.com/decentraland/js-sdk-toolchain/pull/1578)；某个实体的指针输入不再会在同一帧中隐藏另一个实体的输入 [#1576](https://github.com/decentraland/js-sdk-toolchain/pull/1576)；未接线的 `ActionCallback` 参数在 Script 智能物件中会解析为 `undefined` ，而不是失败 [#1605](https://github.com/decentraland/js-sdk-toolchain/pull/1605)；复合体会正确实例化到根实体上 [#1587](https://github.com/decentraland/js-sdk-toolchain/pull/1587)；可选组件字段设为 `0` 或 `false` 现在会被保存 [#1582](https://github.com/decentraland/js-sdk-toolchain/pull/1582)；从原始或数组模式定义的组件可以被读回 [#1593](https://github.com/decentraland/js-sdk-toolchain/pull/1593)；若干序列化和网络健壮性修复 [#1592](https://github.com/decentraland/js-sdk-toolchain/pull/1592), [#1595](https://github.com/decentraland/js-sdk-toolchain/pull/1595), [#1568](https://github.com/decentraland/js-sdk-toolchain/pull/1568), [#1570](https://github.com/decentraland/js-sdk-toolchain/pull/1570).

### 7.27.0（2026 年 8 月 27 日）

* **事件辅助改进**：指针、触发器、音频及其他事件辅助系统现在都有唯一名称和一致的行为。 [#1536](https://github.com/decentraland/js-sdk-toolchain/pull/1536)
* **加载屏幕状态**: `EngineInfo` 会告诉你加载屏幕是否仍然可见。 [#1555](https://github.com/decentraland/js-sdk-toolchain/pull/1555)
* **更快的构建**：在构建和部署时，项目文件会并发处理。 [#1481](https://github.com/decentraland/js-sdk-toolchain/pull/1481)
* **修复**：点击事件会被所有玩家检测到，而不只是第一个 [#1468](https://github.com/decentraland/js-sdk-toolchain/pull/1468)；计时器在回调抛出异常后仍会继续工作 [#1470](https://github.com/decentraland/js-sdk-toolchain/pull/1470)；未挂载的 React ECS UI 实体会被释放，修复了内存泄漏 [#1471](https://github.com/decentraland/js-sdk-toolchain/pull/1471)；observable 订阅不再相互干扰 [#1467](https://github.com/decentraland/js-sdk-toolchain/pull/1467)；部署时的登录步骤会正确转发请求头 [#1551](https://github.com/decentraland/js-sdk-toolchain/pull/1551)；渲染器保留的实体 ID 从不会被生成或回收 [#1544](https://github.com/decentraland/js-sdk-toolchain/pull/1544), [#1460](https://github.com/decentraland/js-sdk-toolchain/pull/1460).

### 7.26.0（2026 年 8 月 13 日）

* **TouchScreenControls 组件**：显示、隐藏和自定义移动端屏幕控制。参见 [屏幕控制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/touch-screen-controls/). [#1441](https://github.com/decentraland/js-sdk-toolchain/pull/1441), [#1510](https://github.com/decentraland/js-sdk-toolchain/pull/1510)
* **从你的场景中打开 Explorer UI**： `openExplorerUi()` 受限操作会打开地图、背包、设置和其他 Explorer 界面，并且 `ExplorerUiEventsResult` 会在它们关闭时报告。 [#1503](https://github.com/decentraland/js-sdk-toolchain/pull/1503), [#1511](https://github.com/decentraland/js-sdk-toolchain/pull/1511)
* **音频事件和表情状态**：新的 `audioEventSystem` 会响应音频播放事件，并且 `AvatarEmoteCommand` 暴露表情 `state`. [#1530](https://github.com/decentraland/js-sdk-toolchain/pull/1530)
* **UI 屏幕默认值**：UI 使用默认的虚拟屏幕和屏幕内边距区域，而且设备像素比不再影响布局。 [#1489](https://github.com/decentraland/js-sdk-toolchain/pull/1489)
* **Bevy Web 预览**：浏览器预览使用 Bevy Web explorer，而不是旧版 web explorer。 [#1502](https://github.com/decentraland/js-sdk-toolchain/pull/1502)
* **预览中的优化资源**：预览可以将你的模型作为资源包加载，与发布后的外观一致。参见 [使用优化资源进行预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets). [#1539](https://github.com/decentraland/js-sdk-toolchain/pull/1539)
* **修复**: `npm run start` 在初始构建失败时仍会保持运行，因此你可以修复错误，然后它会重新构建 [#1501](https://github.com/decentraland/js-sdk-toolchain/pull/1501)；忙碌端口可被正确检测到 [#1542](https://github.com/decentraland/js-sdk-toolchain/pull/1542).

### 7.24.5（2026 年 7 月 15 日）

* **广告牌目标**：一个 `Billboard` 可以面向一个目标实体，而不是摄像机。 [#1445](https://github.com/decentraland/js-sdk-toolchain/pull/1445)
* **更小的场景 bundle**：第二份 `@dcl/ecs` 不再被打包进每个场景中。 [#1450](https://github.com/decentraland/js-sdk-toolchain/pull/1450)
* **修复**: `MessageBus` 消息按顺序发送。 [#1462](https://github.com/decentraland/js-sdk-toolchain/pull/1462)

### 7.24.4（2026 年 7 月 3 日）

* **React ECS 的 InteractableArea**：用于屏幕上可交互区域的新 UI 组件。 [#1411](https://github.com/decentraland/js-sdk-toolchain/pull/1411)

### 7.24.3（2026 年 7 月 1 日）

* **不再有 45x45 地块限制** 用于场景。 [#1429](https://github.com/decentraland/js-sdk-toolchain/pull/1429)
* **二段跳和滑翔**: `AvatarLocomotionSettings` 具有切换二段跳和滑翔的字段。 [#1402](https://github.com/decentraland/js-sdk-toolchain/pull/1402)
* **修复**：UI 虚拟屏幕会考虑设备像素比。 [#1433](https://github.com/decentraland/js-sdk-toolchain/pull/1433)

### 7.24.2（2026 年 6 月 25 日）

* **修复**：重新武装的 `setTimeout` 在长帧之后不再使场景卡住 [#1420](https://github.com/decentraland/js-sdk-toolchain/pull/1420)；React ECS 保留一个可选属性，该属性会在首次设置时 [#1422](https://github.com/decentraland/js-sdk-toolchain/pull/1422)；部署和其他 CLI 请求中的“过早关闭”错误已消失 [#1428](https://github.com/decentraland/js-sdk-toolchain/pull/1428).

### 7.24.1（2026 年 6 月 12 日）

* **玩家碰撞层**：用于主玩家的新碰撞层。 [#1414](https://github.com/decentraland/js-sdk-toolchain/pull/1414)
* **修复**：重新触发一个 `AudioSource` 不再播放重复的声音。 [#1382](https://github.com/decentraland/js-sdk-toolchain/pull/1382)

### 7.24.0（2026 年 6 月 4 日）

* **ScreenInsetArea**：React ECS 组件，可在移动端将 UI 保持在安全区域内。参见 [移动端安全区域](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/safe-area/). [#1386](https://github.com/decentraland/js-sdk-toolchain/pull/1386)
* **复合体提供程序**: `getCompositeProvider()` 和 `setCompositeProvider()` 在引擎上，用于更高级地控制复合体如何加载。 [#1333](https://github.com/decentraland/js-sdk-toolchain/pull/1333)

### 7.23.2（2026 年 5 月 13 日）

* **修复**：当玩家停留在触发区域内时，不再反复发送网络消息。 [#1397](https://github.com/decentraland/js-sdk-toolchain/pull/1397)

### 7.23.1（2026 年 5 月 5 日）

* **音频分析**：从正在播放的音频源读取实时数据以驱动效果。 [#1256](https://github.com/decentraland/js-sdk-toolchain/pull/1256)

### 7.23.0（2026 年 5 月 5 日）

* **AvatarLocomotionSettings 组件**：控制玩家的行走、奔跑和跳跃设置。 [#1389](https://github.com/decentraland/js-sdk-toolchain/pull/1389)
* **修饰键输入操作**: `IA_MODIFIER` 可在输入命令列表中使用。 [#1390](https://github.com/decentraland/js-sdk-toolchain/pull/1390)

### 7.22.5（2026 年 4 月 23 日）

* **代码专用场景的更小 bundle**：只有当场景使用复合体时，智能物件库才会被打包。 [#1381](https://github.com/decentraland/js-sdk-toolchain/pull/1381)

### 7.22.4（2026 年 4 月 16 日）

* **部署中排除 sourcemap** 通过 `.dclignore`. [#1373](https://github.com/decentraland/js-sdk-toolchain/pull/1373)
* **粒子系统 burst** 属性已更新。 [#1375](https://github.com/decentraland/js-sdk-toolchain/pull/1375)

### 7.22.3（2026 年 4 月 8 日）

* **ParticleSystem 组件**：从实体发射粒子。参见 [粒子系统](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system/). [#1369](https://github.com/decentraland/js-sdk-toolchain/pull/1369)
* **更少的网络流量**：向可变组件写入未更改的值不再发送消息。 [#1359](https://github.com/decentraland/js-sdk-toolchain/pull/1359)
* **更清晰的部署错误** 当 World 不存在、你没有权限，或者 catalyst 无法访问时。 [#1361](https://github.com/decentraland/js-sdk-toolchain/pull/1361), [#1370](https://github.com/decentraland/js-sdk-toolchain/pull/1370)

### 7.22.2（2026 年 3 月 26 日）

* **邻近交互**：响应玩家接近某个实体。参见 [邻近事件](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events/). [#1356](https://github.com/decentraland/js-sdk-toolchain/pull/1356)

### 7.22.1（2026 年 3 月 26 日）

* **更新的登录页面** 用于通过 CLI 部署时。 [#1362](https://github.com/decentraland/js-sdk-toolchain/pull/1362)

### 7.22.0（2026 年 3 月 25 日）

* **物理力和冲量**：用力和冲量推动玩家。参见 [玩家物理](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/player-physics/). [#1338](https://github.com/decentraland/js-sdk-toolchain/pull/1338)
* **多场景 World**：将多个场景部署到同一个 World。 [#1355](https://github.com/decentraland/js-sdk-toolchain/pull/1355)
* **从 CI 部署** 流水线。 [#1357](https://github.com/decentraland/js-sdk-toolchain/pull/1357)
* **修复**：在 React ECS 中重新排序带键列表 [#1351](https://github.com/decentraland/js-sdk-toolchain/pull/1351)；操作类型被排除在网络同步之外 [#1354](https://github.com/decentraland/js-sdk-toolchain/pull/1354).

### 7.21.0（2026 年 3 月 10 日）

* **在你的场景中禁用二段跳和滑翔。** 在你的场景中。 [#1323](https://github.com/decentraland/js-sdk-toolchain/pull/1323)
* **Tween MoveRotateScale 模式**：在单个 tween 中为位置、旋转和缩放做动画。 [#1339](https://github.com/decentraland/js-sdk-toolchain/pull/1339)
* **平台检测**：了解场景是在桌面端、Web 端还是移动端运行。参见 [检测平台](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/detect-platform/). [#1340](https://github.com/decentraland/js-sdk-toolchain/pull/1340)

### 7.20.4（2026 年 3 月 3 日）

* **多实例预览**：运行多个玩家实例连接到一个预览，以测试多人模式。 [#1336](https://github.com/decentraland/js-sdk-toolchain/pull/1336)
* **修复**：实体 ID 冲突 [#1332](https://github.com/decentraland/js-sdk-toolchain/pull/1332); `UiInput` 不再将其值回显 [#1331](https://github.com/decentraland/js-sdk-toolchain/pull/1331).

### 7.20.2 和 7.20.3（2026 年 2 月 23 日）

* **移动端 QR 包含位置** 以便在该处生成。 [#1328](https://github.com/decentraland/js-sdk-toolchain/pull/1328)
* **修复**：具有相同时间戳的值（例如指针事件结果）会被正确排序 [#1322](https://github.com/decentraland/js-sdk-toolchain/pull/1322)；移动端预览端点已修复（[7.20.3 发布](https://github.com/decentraland/js-sdk-toolchain/releases/tag/7.20.3)).

### 7.20.1（2026 年 2 月 16 日）

* **在部署到多场景 World 时流程更顺畅。** [#1317](https://github.com/decentraland/js-sdk-toolchain/pull/1317)

### 7.20.0（2026 年 2 月 5 日）

* **资源预加载**：将资源标记为在需要前加载。 [#1287](https://github.com/decentraland/js-sdk-toolchain/pull/1287)

### 7.19.0（2026 年 1 月 29 日）

* **SDK 辅助函数**：用于延迟指令等常见任务的快捷方式。 [#1275](https://github.com/decentraland/js-sdk-toolchain/pull/1275)
* **指针事件最大玩家距离**：限制玩家可交互的最远距离。 [#1293](https://github.com/decentraland/js-sdk-toolchain/pull/1293)

### 7.18.0（2026 年 1 月 26 日）

* **多个 UI 渲染器**: `addUiRenderer()` 和 `removeUiRenderer()` 在 React ECS 渲染器上。 [#1285](https://github.com/decentraland/js-sdk-toolchain/pull/1285)
* **自动 UI 缩放系数** 在 React ECS 中。 [#1290](https://github.com/decentraland/js-sdk-toolchain/pull/1290)
* **来自代码的 Script 实例**：可从代码中的任何位置调用智能物件的 Script 组件，并使用 `ActionCallback`. [#1284](https://github.com/decentraland/js-sdk-toolchain/pull/1284)
* **Markdown 文件被排除在部署之外。** [#1282](https://github.com/decentraland/js-sdk-toolchain/pull/1282)

### 7.17.0（2026 年 1 月 5 日）

* **构建错误会显示类型检查输出。** [#1263](https://github.com/decentraland/js-sdk-toolchain/pull/1263)
* **移动端预览会打印坐标和深度链接。** [#1268](https://github.com/decentraland/js-sdk-toolchain/pull/1268)

### 7.16.0（2025 年 12 月 30 日）

* **可见性会传播到子级**：隐藏一个带有 `VisibilityComponent` 的实体也会隐藏其子级。 [#1262](https://github.com/decentraland/js-sdk-toolchain/pull/1262)
* **在移动端预览**：在移动应用中运行预览。 [#1261](https://github.com/decentraland/js-sdk-toolchain/pull/1261)
* **Bevy Web 浏览器预览** 受支持。 [#1241](https://github.com/decentraland/js-sdk-toolchain/pull/1241)
* **`npm run start` 默认打开新的桌面端 Explorer。** [#1264](https://github.com/decentraland/js-sdk-toolchain/pull/1264)
* **修复**：智能可穿戴项目模板再次可下载。 [#1255](https://github.com/decentraland/js-sdk-toolchain/pull/1255)

### 7.15.0 和 7.15.1（2025 年 12 月 8 日）

* **空间音频** 用于 `AudioSource` 和 `VideoPlayer`. [#1243](https://github.com/decentraland/js-sdk-toolchain/pull/1243)
* **Script 组件支持** 适用于带代码的智能物件。 [#1245](https://github.com/decentraland/js-sdk-toolchain/pull/1245)
* **修复**：缺少 Desktop Client 时，Windows 会提示下载它 [#1244](https://github.com/decentraland/js-sdk-toolchain/pull/1244)；智能物件可从嵌套的 `node_modules` [#1258](https://github.com/decentraland/js-sdk-toolchain/pull/1258).

### 7.12.1（2025 年 11 月 3 日）

* **`code-to-composite` 命令**：将场景代码转换为编辑器可打开的复合体。 [#1232](https://github.com/decentraland/js-sdk-toolchain/pull/1232)

### 7.12.0（2025 年 10 月 30 日）

* **CopyToClipboard 受限操作**. [#1234](https://github.com/decentraland/js-sdk-toolchain/pull/1234)

### 7.11.0（2025 年 10 月 15 日）

* **来自内容服务器的错误更清晰** 在部署时。 [#1217](https://github.com/decentraland/js-sdk-toolchain/pull/1217)
* **`dclcontext` 以及 thumbnails 文件夹被排除在部署之外。** [#1227](https://github.com/decentraland/js-sdk-toolchain/pull/1227), [#1229](https://github.com/decentraland/js-sdk-toolchain/pull/1229)

### 7.10.6（2025 年 10 月 9 日）

* **Tween 连续模式**：持续旋转或移动，不限时长。 [#1221](https://github.com/decentraland/js-sdk-toolchain/pull/1221)

### 7.10.5（2025 年 10 月 6 日）

* **`get-context-files` 命令**：将 AI 上下文文档下载到你的场景中，以便编码助手了解 SDK。 [#1220](https://github.com/decentraland/js-sdk-toolchain/pull/1220)

### 7.10.3（2025 年 9 月 29 日）

* **触发区域组件**：检测玩家进入、停留和离开某个区域。参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/trigger-areas/). [#1210](https://github.com/decentraland/js-sdk-toolchain/pull/1210)
* **Tags 组件**：给实体添加标签，并按标签查询它们。 [#1216](https://github.com/decentraland/js-sdk-toolchain/pull/1216)

## Creator Hub

### 0.52.0（2026 年 10 月 20 日）

* **Avatar Animations 资源包**：45 个可直接使用的头像动画，加上每个场景都已可播放的 32 个内置动画。将卡片悬停可观看，拖入一个即可将其文件复制到你的场景中，然后从代码中播放。参见 [头像动画](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/#avatar-animations). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **在 AI Assistant 提示中提及资源**：双击 **本地资源** 中的一个资源，就能在下一条消息中提及它，以及你所选中的内容。参见 [在提示中指向一个资源](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/#point-at-an-asset-in-your-prompt). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **超过 400 个新资源，以及 Buildings 和 Materials 类别**：来自 Genesis Plaza 和 Decentraland 模板场景的模型，以及会飞溅的 Water Plane 和 Water Cylinder 智能物件，还有一个当玩家站在其上时会按下的 Floor Pressure Button。参见 [水](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/#water). [#1696](https://github.com/decentraland/creator-hub/pull/1696)
* **可折叠的资源面板**: 面板标签栏右侧的下拉箭头会将其收缩为仅显示标签页，使 3D 视图占满整个窗口高度。参见 [添加项目](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/#add-items). [#1691](https://github.com/decentraland/creator-hub/pull/1691)
* **通过终端设置 AI Assistant**: Claude 和 Cursor 现在可以像 Gemini 一样通过终端安装并登录。每个设置界面都会显示该供应商针对你的操作系统的安装命令，并链接到其安装指南，而且 **重新检查** 无需重启即可找到新的安装。参见 [连接编码 CLI](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/#connect-a-coding-cli). [#1684](https://github.com/decentraland/creator-hub/pull/1684)
* **发布后的可分享链接**: 成功界面会显示并复制一个任何人都能在浏览器中打开的网页链接，而不是需要桌面客户端的链接。参见 [发布你的场景](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/fa-bu/publish-scene/). [#1688](https://github.com/decentraland/creator-hub/pull/1688)
* **Collections 卡片会打开创作者应用**: 穿戴物和表情 **合集** 卡片位于 **更多** 页面现在会打开新的创作者应用，而不是旧版 Builder。 [#1687](https://github.com/decentraland/creator-hub/pull/1687)
* **修复**: 一个发布失败提示，说明场景缩略图“必须是部署中包含的文件”，受影响的场景会在下次保存缩略图时自行修复 [#1657](https://github.com/decentraland/creator-hub/pull/1657)；在 Layout 标签页中调整大小后的布局会保留，因此在 Bevy 渲染器下发布时不会再将场景重置为单个地块 [#1682](https://github.com/decentraland/creator-hub/pull/1682)；AI Assistant 可靠性提升，静默轮次现在会显示错误，过期的问题会关闭， **停止** 在 Windows 上真正结束一个轮次，以及在轮次中发送的消息会返回到编辑器而不是丢失 [#1681](https://github.com/decentraland/creator-hub/pull/1681)；当代理报告你尚未登录时的登录引导 [#1685](https://github.com/decentraland/creator-hub/pull/1685).

### 0.51.0（2026 年 10 月 8 日）

* **将文件附加到 AI Assistant 提示词**: 将文件拖到消息框上，使用回形针按钮选择它们，或从剪贴板粘贴图片。支持图片、3D 模型、音频以及其他任何文件，每条消息最多 8 个。参见 [将文件附加到提示词](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/#attach-files-to-a-prompt). [#1670](https://github.com/decentraland/creator-hub/pull/1670)
* **从媒体拖放创建快速项**: 从……拖拽图片、声音或视频 **本地资源** 到画布上现在会创建一个已预先指向该文件的现成项目。参见 [导入自定义资源](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/import-items/). [#1616](https://github.com/decentraland/creator-hub/pull/1616)
* **工具栏按钮标为“再次播放”**，其菜单分为一个用于选择预览位置的主面板，以及一个 **桌面客户端** 仅在该处适用的选项子菜单。调试控制台现在默认打开，因此不再是菜单选项。参见 [预览你的场景](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/). [#1671](https://github.com/decentraland/creator-hub/pull/1671), [#1656](https://github.com/decentraland/creator-hub/pull/1656)
* **在 UI 编辑器中输入精确缩放级别**: 缩放百分比现在是一个字段。单击它，输入 10 到 200 之间的值并按 Enter。参见 [UI 编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor/). [#1659](https://github.com/decentraland/creator-hub/pull/1659)
* **编辑时静音场景音频**: 编辑器工具栏中的扬声器按钮会在 Bevy 渲染器下静音所有音频源和流。参见 [场景渲染器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/about-editor/#scene-renderer). [#1669](https://github.com/decentraland/creator-hub/pull/1669)
* **场景内边距选项已重命名**: 在 UI 编辑器中，“Gameplay Safe Area”现在变为“Interactable Safe Area”。它写入相同的 `screenInset: 'interactable'` 值，与之前相同。参见 [UI 编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor/). [#1673](https://github.com/decentraland/creator-hub/pull/1673)
* **修复**: 触发区域重复创建和形状编辑字段丢失 [#1658](https://github.com/decentraland/creator-hub/pull/1658)；MobileHUD 预览现在以移动屏幕为框架，而不是当前活动的 GUI [#1648](https://github.com/decentraland/creator-hub/pull/1648)；重命名场景不再丢失按键输入，编辑器标题栏会更新 [#1651](https://github.com/decentraland/creator-hub/pull/1651)；调试控制台在……之后不再消失 **从磁盘重新加载场景** [#1652](https://github.com/decentraland/creator-hub/pull/1652)；Bevy 编辑器撤销清理和依赖提示 [#1669](https://github.com/decentraland/creator-hub/pull/1669)；Decentraland 标志会显示在智能项工具提示中 [#1677](https://github.com/decentraland/creator-hub/pull/1677)；被删除的实体仍可通过撤销恢复 [#1679](https://github.com/decentraland/creator-hub/pull/1679)；较长的复选框标签会换行 [#1676](https://github.com/decentraland/creator-hub/pull/1676).

### 0.50.0（2026 年 9 月 23 日）

* **拖到项目文件夹中的资源会自动显示**: 复制到项目的 `资源` 文件夹中从编辑器外复制进来的文件会出现在 **本地资源** 无需按 **刷新**。参见 [导入自定义资源](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/import-items/). [#1606](https://github.com/decentraland/creator-hub/pull/1606)
* **UI 编辑器已稳定**: 它不再隐藏在实验性设置后面。可通过 2D/3D 切换打开它。你的场景需要 `@dcl/sdk` 7.26.0 或更新版本。参见 [UI 编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor/). [#1643](https://github.com/decentraland/creator-hub/pull/1643)
* **AI Assistant 已稳定** 并且默认启用，拥有自己的 **AI** 设置中的标签页。参见 [AI Assistant](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/). [#1646](https://github.com/decentraland/creator-hub/pull/1646)
* **在编辑器中优化 3D 模型**: 一个 **优化** 标题栏和……中的操作 **本地资源** 面板会压缩你的模型，并带有 **还原** 可将它们恢复。参见 [优化模型](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/optimize-models.md). [#1566](https://github.com/decentraland/creator-hub/pull/1566)
* **触发区域智能项**: 放置一个不可见方块，选择它是仅对玩家本人响应还是对所有人响应，并用通俗语言描述当有人进入或离开时应发生什么。参见 [触发区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/trigger-area/). [#1604](https://github.com/decentraland/creator-hub/pull/1604)
* **UI 编辑器中的可自定义 MobileHUD**: 选择主按钮，显示或隐藏其他按钮、虚拟摇杆和准星，并为它们设置自定义图标，画布上会实时预览。参见 [自定义移动端控制](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor/#customize-the-mobile-controls). [#1615](https://github.com/decentraland/creator-hub/pull/1615)
* **16:9 场景缩略图**: 缩略图预览为全宽，其他宽高比的图片会被拒绝，自动生成的缩略图以 16:9 采集。参见 [场景缩略图](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/pei-zhi/scene-settings/#scene-thumbnail). [#1601](https://github.com/decentraland/creator-hub/pull/1601)
* **组件设置与 SDK 对齐**: 检查器现在公开视频播放速率、起始位置和空间音频，文本换行和框大小，音频音高，以及材质移动和旋转模式，并提供清除颜色和纹理的方法。通过代码设置的属性在你编辑组件时不再丢失。参见 [组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/components/). [#1470](https://github.com/decentraland/creator-hub/pull/1470)
* **可拆卸调试控制台**: 将控制台弹出到独立窗口并将其放在第二个显示器上。参见 [Creator Hub 控制台](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#the-creator-hub-console). [#1642](https://github.com/decentraland/creator-hub/pull/1642)
* **AI Assistant 可以在 Cursor 和 Gemini 上使用 Creator Hub 场景工具**，不只是 Claude 和 Codex。参见 [AI Assistant](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/). [#1614](https://github.com/decentraland/creator-hub/pull/1614)
* **修复**: **显示移动端二维码** 无论选择哪种预览客户端都可使用，而 Bevy（Web）预览会复用其标签页 [#1608](https://github.com/decentraland/creator-hub/pull/1608)；发布不再等待 LOD 生成，因此 **正在优化** 步骤消失了 [#1603](https://github.com/decentraland/creator-hub/pull/1603)；UI 编辑器不再留下孤立的 `./ui` 在旧场景中的导入 [#1609](https://github.com/decentraland/creator-hub/pull/1609)；Bevy 渲染器不再显示“代码元素”预览警告 [#1607](https://github.com/decentraland/creator-hub/pull/1607)；Admin Toolkit 不再更改场景其余 UI 的 z 顺序 [#1612](https://github.com/decentraland/creator-hub/pull/1612)；键盘快捷键覆盖层在 Bevy 下可正常工作，导航时滚动会重置，实体树图标切换行为也正常 [#1617](https://github.com/decentraland/creator-hub/pull/1617)；AI Assistant 编辑器、聊天下拉菜单和通知的润色 [#1632](https://github.com/decentraland/creator-hub/pull/1632), [#1621](https://github.com/decentraland/creator-hub/pull/1621).

### 0.49.0（2026 年 9 月 16 日）

* **移动设置智能项**: 通过滑块而非代码更改玩家移动速度和跳跃高度。参见 [移动设置](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/smart-items/#locomotion-settings). [#1584](https://github.com/decentraland/creator-hub/pull/1584)
* **AI Assistant 支持 Cursor 和 Gemini**，以及 Claude 和 Codex。参见 [AI Assistant](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/). [#1591](https://github.com/decentraland/creator-hub/pull/1591)
* **输入精确变换数值**: 按下工具键、轴和数值即可精确移动、旋转或缩放，而不是拖拽。 **F** 不再是自由操纵器的快捷键。参见 [输入精确数值](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/ru-men/scene-editor-essentials/#type-an-exact-amount). [#1599](https://github.com/decentraland/creator-hub/pull/1599)
* **修复**: UI 编辑器不再用全屏容器让视图变得模糊，嵌套的 GUI 预览值和自由放置可正常工作 [#1590](https://github.com/decentraland/creator-hub/pull/1590)；Animator 组件会显示在 Bevy 编辑器下 [#1587](https://github.com/decentraland/creator-hub/pull/1587)；Bevy 编辑器热重载由真实的构建事件驱动 [#1592](https://github.com/decentraland/creator-hub/pull/1592)；AI Assistant 的后续轮次在 Codex 下再次可用 [#1600](https://github.com/decentraland/creator-hub/pull/1600)；下拉菜单和三点菜单不再被裁剪 [#1595](https://github.com/decentraland/creator-hub/pull/1595)；一键创建场景，实体名称保留连续空格，场景卡片菜单不再重新排列网格，资源工具提示在搜索结果中正确显示，并且你可以从本地资源树拖拽 [#1593](https://github.com/decentraland/creator-hub/pull/1593).

### 0.48.0（2026 年 9 月 10 日）

* **UI 编辑器移动端预览**：真实手机框架，带准确的安全区域，使用与客户端相同的设备预设，另外还可切换可选的 HUD 指引（虚拟摇杆、按钮、聊天）。 [#1579](https://github.com/decentraland/creator-hub/pull/1579)
* **AI Assistant 重新设计**：更简洁的面板、可靠的关闭按钮、持久的介绍消息和更好的设置。参见 [AI Assistant](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/ai-assistant/). [#1586](https://github.com/decentraland/creator-hub/pull/1586)
* **分析显示中位游玩时长** 而不是平均值。 [#1564](https://github.com/decentraland/creator-hub/pull/1564)
* **更改**：Bevy 渲染器标记为“实验性” [#1583](https://github.com/decentraland/creator-hub/pull/1583)；场景设置字段现在是“Creator name” [#1561](https://github.com/decentraland/creator-hub/pull/1561)；发布界面上的资源包延迟警告已消失 [#1582](https://github.com/decentraland/creator-hub/pull/1582).
* **修复**：在 Windows 上，AI Assistant 的轮次在命令行过长时不再失败。 [#1588](https://github.com/decentraland/creator-hub/pull/1588)

### 0.47.0（2026 年 9 月 4 日）

* **场景代理**：内置于编辑器的 AI 助手。它会读取你的场景，编写 SDK7 代码，在视口中实时创建和编辑实体、组件、智能项和脚本，并可运行预览来检查成果。它驱动你安装的 Claude 或 Codex CLI，因此无需设置 API 密钥。 [#1499](https://github.com/decentraland/creator-hub/pull/1499)
* **智能项运行在 Script 组件上**：每个智能项的行为现在以代码形式保存在 `.ts` 文件中，位于该项的文件夹内，其主要函数作为操作暴露出来。你可以阅读并调整它。 [#1354](https://github.com/decentraland/creator-hub/pull/1354)
* **Script 智能项的滑块输入**：数值参数可以声明范围，并显示为滑块。参见 [使用 Script 组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/). [#1513](https://github.com/decentraland/creator-hub/pull/1513)
* **UI 编辑器**：节点自身的位置（固定或流式）与其排列子节点的方式无关，节点带有 Figma 风格图标。 [#1552](https://github.com/decentraland/creator-hub/pull/1552)
* **一个共享的教程列表** 位于场景侧边栏和学习页面。 [#1551](https://github.com/decentraland/creator-hub/pull/1551)
* **修复**: Intel Mac 再次可启动 [#1545](https://github.com/decentraland/creator-hub/pull/1545)；UI 编辑器会记住 2D 或 3D 模式 [#1532](https://github.com/decentraland/creator-hub/pull/1532)；导入的动画默认权重为 1 [#1557](https://github.com/decentraland/creator-hub/pull/1557)；Script 组件会出现在实体面板中 GLTF 之后 [#1556](https://github.com/decentraland/creator-hub/pull/1556)；自定义项可在 Bevy 渲染器下加载 [#1555](https://github.com/decentraland/creator-hub/pull/1555).

### 0.46.0（2026 年 8 月 27 日）

* **全屏容器** 在 UI 编辑器中：一键填满整个屏幕的容器。 [#1540](https://github.com/decentraland/creator-hub/pull/1540)

### 0.45.0（2026 年 8 月 27 日）

* **UI 编辑器（实验性）**：在带有节点树和属性面板的 2D 画布上可视化设计场景的屏幕界面。你的 `@dcl/react-ecs` 代码仍然是唯一的事实来源。在 Settings > Experimental 中启用它。需要 SDK 7.26.0 或更高版本。参见 [UI 编辑器](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/ui-editor/). [#1523](https://github.com/decentraland/creator-hub/pull/1523), [#1522](https://github.com/decentraland/creator-hub/pull/1522), [#1521](https://github.com/decentraland/creator-hub/pull/1521), [#1520](https://github.com/decentraland/creator-hub/pull/1520), [#1524](https://github.com/decentraland/creator-hub/pull/1524)
* **头像和摄像机修饰区域**：更改头像在区域内的外观以及摄像机的行为。可将它们作为组件或智能项添加。参见 [修饰区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/jiao-hu-xing/modifier-areas/). [#1471](https://github.com/decentraland/creator-hub/pull/1471)
* **Friendzone Buildathon 2026 工作坊** 位于 Learn > Videos。 [#1510](https://github.com/decentraland/creator-hub/pull/1510)
* **修复**: 发布到 LAND 不再卡在“Publishing…” [#1496](https://github.com/decentraland/creator-hub/pull/1496)；从 Bevy 编辑器发布到 World 时，确认按钮可正常工作 [#1493](https://github.com/decentraland/creator-hub/pull/1493)；Analytics 会列出每个已部署场景 [#1492](https://github.com/decentraland/creator-hub/pull/1492)；新的 NAME 和 World 权限无需重启即可出现 [#1509](https://github.com/decentraland/creator-hub/pull/1509)；Windows 上“Creator Hub cannot be closed”更新对话框已消失 [#1497](https://github.com/decentraland/creator-hub/pull/1497)；More 标签页移除了 Legacy Web Editor 卡片，并将 Names 链接到 marketplace [#1495](https://github.com/decentraland/creator-hub/pull/1495).

### 0.44.2（2026 年 8 月 14 日）

* **分析**：针对你拥有或协作的每个 World 和 Genesis City 场景的访客、留存、访问和参与情况，按桌面和移动端拆分。参见 [场景分析](/docs/creator/content-creator-zh/chang-jing-sdk7/qi-ta/scene-analytics/). [#1447](https://github.com/decentraland/creator-hub/pull/1447)
* **重新设计的主页。** [#1481](https://github.com/decentraland/creator-hub/pull/1481)
* **修复**: GLB 导入不再卡在加载转圈 [#1487](https://github.com/decentraland/creator-hub/pull/1487)；本地多人服务器会在场景预览中再次启动 [#1455](https://github.com/decentraland/creator-hub/pull/1455).

### 0.44.0（2026 年 8 月 4 日）

* **Bevy 渲染器（实验性）**：编辑器视口的替代渲染器。 [#1330](https://github.com/decentraland/creator-hub/pull/1330), [#1363](https://github.com/decentraland/creator-hub/pull/1363)
* **优化资源预览选项**：使用资源包进行预览，使模型看起来与发布后相同。参见 [使用优化资源进行预览](/docs/creator/content-creator-zh/chang-jing-sdk7/ru-men/preview-scene/#preview-with-optimized-assets). [#1396](https://github.com/decentraland/creator-hub/pull/1396)
* **Explorer MCP 开关** 位于预览选项中。 [#1440](https://github.com/decentraland/creator-hub/pull/1440)
* **实体树搜索。** [#1385](https://github.com/decentraland/creator-hub/pull/1385)
* **重命名项目文件夹** 从场景视图中。 [#1357](https://github.com/decentraland/creator-hub/pull/1357)
* **“隐藏地形”场景设置。** [#1383](https://github.com/decentraland/creator-hub/pull/1383)
* **Admin 工具重新换肤** 采用新的 Decentraland 设计。 [#1360](https://github.com/decentraland/creator-hub/pull/1360)
* **移除了 Test Server 发布选项。** [#1441](https://github.com/decentraland/creator-hub/pull/1441)
* **修复**: 移动端二维码预览不再打开桌面客户端 [#1452](https://github.com/decentraland/creator-hub/pull/1452)；实体树中的锁定和隐藏切换可再次关闭 [#1429](https://github.com/decentraland/creator-hub/pull/1429)；自定义项包含 Placeholder 组件资源 [#1287](https://github.com/decentraland/creator-hub/pull/1287)；DCL Cast 管理面板会自动打开，保留幻灯片状态并同步给后加入者 [#1358](https://github.com/decentraland/creator-hub/pull/1358)；管理工具会跳过在权威服务器上的初始化 [#1381](https://github.com/decentraland/creator-hub/pull/1381).

### 0.43.0（2026 年 7 月 6 日）

* **更简单的登录**：只需一次浏览器确认，无需输入代码。 [#1338](https://github.com/decentraland/creator-hub/pull/1338)
* **修复**：部署超时时显示清晰的错误 [#1355](https://github.com/decentraland/creator-hub/pull/1355)；覆盖负坐标和正坐标的场景的地块数量正确 [#1352](https://github.com/decentraland/creator-hub/pull/1352)；重新导入时显示“场景已导入”消息 [#1350](https://github.com/decentraland/creator-hub/pull/1350).

### 0.42.0（2026 年 6 月 26 日）

* **粒子系统组件** 在编辑器中。参见 [粒子系统](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/particle-system/). [#1311](https://github.com/decentraland/creator-hub/pull/1311)
* **生成实体操作**：从操作链中生成一个智能项，而不是预先放置隐藏副本。 [#1322](https://github.com/decentraland/creator-hub/pull/1322)
* **复制和粘贴组件值** 从组件的三点菜单中。 [#1321](https://github.com/decentraland/creator-hub/pull/1321)
* **最多 100 位协作者** 每个 World。 [#1310](https://github.com/decentraland/creator-hub/pull/1310)
* **协作者可以取消发布 World 场景。** [#1209](https://github.com/decentraland/creator-hub/pull/1209)
* **更改**：Teen 年龄分级已移除 [#1329](https://github.com/decentraland/creator-hub/pull/1329)；警告说明单人模式不支持直播 [#1336](https://github.com/decentraland/creator-hub/pull/1336).
* **修复**：Wind Area 和 Impulse Area 仅对当前玩家响应 [#1323](https://github.com/decentraland/creator-hub/pull/1323)；管理工具 UI 适配移动屏幕 [#1256](https://github.com/decentraland/creator-hub/pull/1256)；多个组件的控件 [#1282](https://github.com/decentraland/creator-hub/pull/1282)；检查器启动错误 [#1334](https://github.com/decentraland/creator-hub/pull/1334).

### 0.41.1（2026 年 4 月 29 日）

* **刷新按钮** 位于顶部栏，会重启编辑器；在使用 AI 工具编辑场景后很方便。 [#1312](https://github.com/decentraland/creator-hub/pull/1312)
* **控制台自动滚动** 并支持复制文本。 [#1305](https://github.com/decentraland/creator-hub/pull/1305)

### 0.41.0（2026 年 4 月 27 日）

* **移动端调试会话**：通过二维码与移动客户端配对，并将实体状态、控制台日志和性能数据流式传输到编辑器中，支持帧擦除以及暂停、继续和重新加载命令。 [#1306](https://github.com/decentraland/creator-hub/pull/1306)
* **DCL Cast 的演讲者展示和演示面板**：通过 Admin Tools 管理直播演讲者和幻灯片。 [#1213](https://github.com/decentraland/creator-hub/pull/1213)
* **实体级错误** 在实体树中：红色图标标记具有无效组件值的实体。 [#1258](https://github.com/decentraland/creator-hub/pull/1258)
* **“禁用附近语音聊天”** 场景限制。 [#1289](https://github.com/decentraland/creator-hub/pull/1289)
* **“查看存储”按钮** 用于 Worlds，可查看和编辑权威服务器数据。 [#1307](https://github.com/decentraland/creator-hub/pull/1307)
* **Wind Area 和 Impulse Area 智能项** 会推动玩家。 [#1296](https://github.com/decentraland/creator-hub/pull/1296), [#1308](https://github.com/decentraland/creator-hub/pull/1308)
* **奖励智能项**：气球、两个穿戴物发放器，以及一个 Clap 领取发放器，它们会发放奖励活动中的穿戴物。 [#1124](https://github.com/decentraland/creator-hub/pull/1124)
* **修复**：离开字段时旋转值不再漂移 [#1266](https://github.com/decentraland/creator-hub/pull/1266)；“固定天空盒”复选框不再与其他复选框互相翻转 [#1254](https://github.com/decentraland/creator-hub/pull/1254)；部署到 World 时只会取消地块重叠的场景 [#1271](https://github.com/decentraland/creator-hub/pull/1271)；重复项会显示在树中的原项旁边 [#1150](https://github.com/decentraland/creator-hub/pull/1150)；更新了触发区域模型 [#1285](https://github.com/decentraland/creator-hub/pull/1285).

### 0.39.1（2026 年 3 月 30 日）

* **“在 Places 中显示”** World 设置中的复选框。 [#1264](https://github.com/decentraland/creator-hub/pull/1264)
* **修复**: 当场景代码损坏时，预览会显示错误，而不是静默失败 [#1238](https://github.com/decentraland/creator-hub/pull/1238)；部署文件中的多场景标志已验证 [#1270](https://github.com/decentraland/creator-hub/pull/1270).

### 0.39.0（2026 年 3 月 26 日）

* **修复**：Windows 安装程序更新 [#1252](https://github.com/decentraland/creator-hub/pull/1252)；多场景部署 [#1262](https://github.com/decentraland/creator-hub/pull/1262)；未正常工作的智能项 [#1260](https://github.com/decentraland/creator-hub/pull/1260)；触发区域和点击区域会显示指针网格 [#1255](https://github.com/decentraland/creator-hub/pull/1255).

### 0.38.0（2026 年 3 月 20 日）

* **修复**：资源包可以再次滚动 [#1239](https://github.com/decentraland/creator-hub/pull/1239)；在新场景中生成自定义智能项时会保留音频文件 [#1234](https://github.com/decentraland/creator-hub/pull/1234).

### 0.37.0（2026 年 3 月 16 日）

* **内嵌调试控制台** 作为资源面板中的一个标签页。 [#1200](https://github.com/decentraland/creator-hub/pull/1200)
* **修复**：自定义项保留其纹理文件 [#1217](https://github.com/decentraland/creator-hub/pull/1217)；操作柄在首次选择时即可正常工作 [#1218](https://github.com/decentraland/creator-hub/pull/1218)；显示图片操作默认居中对齐 [#1221](https://github.com/decentraland/creator-hub/pull/1221)；视频源说明不再只提及 Vimeo [#1216](https://github.com/decentraland/creator-hub/pull/1216).

### 0.36.0（2026 年 3 月 10 日）

* **修复**：在实体树中选择生成点 [#1199](https://github.com/decentraland/creator-hub/pull/1199)；World 内容分级中移除了“Restricted” [#1205](https://github.com/decentraland/creator-hub/pull/1205)；Manage Published 仅列出包含已发布场景的 Worlds [#1208](https://github.com/decentraland/creator-hub/pull/1208)；超过 World 限制时的部署错误 [#1210](https://github.com/decentraland/creator-hub/pull/1210).

### 0.35.0（2026 年 3 月 5 日）

* **生成点可视化** 在编辑器中。参见 [生成区域](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/gou-jian/spawn-areas/). [#1129](https://github.com/decentraland/creator-hub/pull/1129)
* **动态资源目录**：最新项会在运行时加载，并进行 SDK 兼容性检查。 [#1172](https://github.com/decentraland/creator-hub/pull/1172)
* **多实例预览** 切换以测试多人模式。需要 SDK 7.20.4 或更高版本。 [#1187](https://github.com/decentraland/creator-hub/pull/1187)
* **World 权限的访问标签页**，以及 World Settings 中的多人模式和天空盒设置。 [#1186](https://github.com/decentraland/creator-hub/pull/1186), [#1189](https://github.com/decentraland/creator-hub/pull/1189)
* **UI 刷新** 涉及颜色、组件样式和实体检查器。 [#1178](https://github.com/decentraland/creator-hub/pull/1178)
* **修复**: TextShape 正确初始化 [#1194](https://github.com/decentraland/creator-hub/pull/1194) 并且渲染时不会裁剪 [#1185](https://github.com/decentraland/creator-hub/pull/1185)；重复的地面瓦片 [#1179](https://github.com/decentraland/creator-hub/pull/1179)；高级模式下的场景布局更新 [#1181](https://github.com/decentraland/creator-hub/pull/1181)；选择高亮不再卡顿或让对象变黑 [#1196](https://github.com/decentraland/creator-hub/pull/1196)；托管项目可加载 [#1182](https://github.com/decentraland/creator-hub/pull/1182)；场景管理员 UI 可在所有屏幕上缩放 [#1190](https://github.com/decentraland/creator-hub/pull/1190).

### 0.34.0（2026 年 2 月 25 日）

* **多场景 World**：将多个场景发布到一个 World。 [#1089](https://github.com/decentraland/creator-hub/pull/1089), [#1163](https://github.com/decentraland/creator-hub/pull/1163)
* **管理 Worlds**：World 设置和管理页面。 [#1132](https://github.com/decentraland/creator-hub/pull/1132), [#1049](https://github.com/decentraland/creator-hub/pull/1049), [#1112](https://github.com/decentraland/creator-hub/pull/1112)
* **协作者权限** 适用于 Worlds。 [#1128](https://github.com/decentraland/creator-hub/pull/1128), [#1083](https://github.com/decentraland/creator-hub/pull/1083), [#1125](https://github.com/decentraland/creator-hub/pull/1125)
* **从 World 中取消发布场景。** [#1123](https://github.com/decentraland/creator-hub/pull/1123)
* **占位组件**：用于坐点、触发区域等的不可见实体，不会影响场景指标。 [#1154](https://github.com/decentraland/creator-hub/pull/1154)
* **记录和删除操作** 用于智能项。 [#1131](https://github.com/decentraland/creator-hub/pull/1131)
* **修复**: 触发区域只会为你自己的玩家激活 [#1142](https://github.com/decentraland/creator-hub/pull/1142); 场景网页流量的开发者工具再次可用 [#1109](https://github.com/decentraland/creator-hub/pull/1109); “切换摄像机”中的虚拟摄像机时长和自选 [#1160](https://github.com/decentraland/creator-hub/pull/1160); 缩放操控器 [#1082](https://github.com/decentraland/creator-hub/pull/1082); 照片墙智能物品 [#1110](https://github.com/decentraland/creator-hub/pull/1110); “自定义代码”警告更准确 [#1171](https://github.com/decentraland/creator-hub/pull/1171); 世界访问问题 [#1133](https://github.com/decentraland/creator-hub/pull/1133); 场景缩略图 [#1168](https://github.com/decentraland/creator-hub/pull/1168); 已移除未使用的保存图标和“连接缓慢”警告 [#1152](https://github.com/decentraland/creator-hub/pull/1152), [#1169](https://github.com/decentraland/creator-hub/pull/1169).

### 0.33.1（2026年2月18日）

* **在移动应用中预览**，可在预览设置中配置。 [#1121](https://github.com/decentraland/creator-hub/pull/1121)
* **脚本组件工具提示** 来自脚本文件中的注释，以及改进后的模板。 [#1130](https://github.com/decentraland/creator-hub/pull/1130), [#1019](https://github.com/decentraland/creator-hub/pull/1019)
* **字体选择** 在 TextShape 组件中。 [#1096](https://github.com/decentraland/creator-hub/pull/1096)
* **修复**: 旋转操控器 [#1081](https://github.com/decentraland/creator-hub/pull/1081); 编辑灯光智能物品 [#1139](https://github.com/decentraland/creator-hub/pull/1139); 发布时误报的“signal is aborted”错误 [#1144](https://github.com/decentraland/creator-hub/pull/1144); 复制后操控器不再切换到自由移动 [#1088](https://github.com/decentraland/creator-hub/pull/1088); 拍手计量器和照片墙智能物品 [#1101](https://github.com/decentraland/creator-hub/pull/1101).

### 0.33.0（2026年2月4日）

* **将脚本方法作为动作**: 动作类型下拉菜单包含来自脚本组件的方法。 [#1026](https://github.com/decentraland/creator-hub/pull/1026), [#1071](https://github.com/decentraland/creator-hub/pull/1071)
* **`isAdmin` 函数** 以判断玩家是否为场景管理员。 [#1037](https://github.com/decentraland/creator-hub/pull/1037)
* **Worlds 存储页面。** [#1036](https://github.com/decentraland/creator-hub/pull/1036)
* **离线模式** 配有“无连接”页面。 [#991](https://github.com/decentraland/creator-hub/pull/991)
* **修复**: 资源搜索会考虑标签和描述 [#1022](https://github.com/decentraland/creator-hub/pull/1022); 缩放操控器响应左右拖动，且无法将尺寸设为 0 [#1065](https://github.com/decentraland/creator-hub/pull/1065); 低于 1 的值进行按比例缩放 [#1076](https://github.com/decentraland/creator-hub/pull/1076); 锁定状态不会被子级继承 [#1085](https://github.com/decentraland/creator-hub/pull/1085); 照片壁画支持来自 Worlds 的图片 [#1074](https://github.com/decentraland/creator-hub/pull/1074); 仅含图片的自定义物品会创建自己的文件夹 [#1033](https://github.com/decentraland/creator-hub/pull/1033); 滑块中的小数值 [#1075](https://github.com/decentraland/creator-hub/pull/1075).

### 0.32.1（2026年1月21日）

* **新动作**: 冻结玩家、解冻玩家、面向物品、更改碰撞、更改天空盒、重置天空盒，以及一个用于“切换摄像机”的摄像机智能物品。 [#997](https://github.com/decentraland/creator-hub/pull/997), [#989](https://github.com/decentraland/creator-hub/pull/989)
* **一次更改多个状态** 通过选择多个物品。 [#975](https://github.com/decentraland/creator-hub/pull/975)
* **重新发布快捷方式**: 无需经过配置菜单即可重新部署到同一个世界或坐标。 [#1003](https://github.com/decentraland/creator-hub/pull/1003)
* **新的脚本化智能物品**: 拍手计量器、可穿戴扫描器、僵尸和照片壁画。挂锁可以在另一个实体上触发“打开”。 [#1008](https://github.com/decentraland/creator-hub/pull/1008)
* **`ActionRef` 和 `ActionCallback` 输入类型** 供脚本使用，用代码处理另一个物品的动作。 [#1054](https://github.com/decentraland/creator-hub/pull/1054)
* **工具提示** 涵盖组件、动作和智能物品。 [#1009](https://github.com/decentraland/creator-hub/pull/1009)
* **管理项目页面。** [#1032](https://github.com/decentraland/creator-hub/pull/1032)
* **修复**: 视频屏幕模型为双面 [#1007](https://github.com/decentraland/creator-hub/pull/1007); 场景指标会正确统计三角形、材质和纹理 [#1020](https://github.com/decentraland/creator-hub/pull/1020).

### 0.32.0（2026年1月5日）

* **一次编辑多个实体上的组件**: 标签、材质、AudioSource、AudioStream、MeshCollider、TextShape、可见性和光源。 [#949](https://github.com/decentraland/creator-hub/pull/949), [#950](https://github.com/decentraland/creator-hub/pull/950), [#958](https://github.com/decentraland/creator-hub/pull/958), [#959](https://github.com/decentraland/creator-hub/pull/959), [#960](https://github.com/decentraland/creator-hub/pull/960), [#963](https://github.com/decentraland/creator-hub/pull/963), [#964](https://github.com/decentraland/creator-hub/pull/964)
* **创建者字段** 在实体检查器中。 [#944](https://github.com/decentraland/creator-hub/pull/944)
* **新的应用设置界面。** [#988](https://github.com/decentraland/creator-hub/pull/988)
* **清理未使用资源**: 全选、中间状态和撤销。 [#993](https://github.com/decentraland/creator-hub/pull/993)
* **更多组件** 可在编辑器中使用。 [#999](https://github.com/decentraland/creator-hub/pull/999)
* **修复**: 隐藏的物品仍保持不可见 [#974](https://github.com/decentraland/creator-hub/pull/974); 透明材质 [#976](https://github.com/decentraland/creator-hub/pull/976); 轴向指示器显示北方 [#978](https://github.com/decentraland/creator-hub/pull/978); 使用过期身份进行部署 [#981](https://github.com/decentraland/creator-hub/pull/981); 发布弹窗不再在点击背景时关闭 [#982](https://github.com/decentraland/creator-hub/pull/982); 部署错误显示 [#985](https://github.com/decentraland/creator-hub/pull/985); 智能物品下拉菜单重叠 [#995](https://github.com/decentraland/creator-hub/pull/995); 撤销不再创建无法删除的模板 [#998](https://github.com/decentraland/creator-hub/pull/998); 位置操控器性能 [#972](https://github.com/decentraland/creator-hub/pull/972).

### 0.31.0 和 0.31.1（2025年12月16日）

* **平面位置操控器。** [#943](https://github.com/decentraland/creator-hub/pull/943)
* **摄像机平移** 以及一个 **轴向辅助器** 在视口中。 [#952](https://github.com/decentraland/creator-hub/pull/952), [#951](https://github.com/decentraland/creator-hub/pull/951)
* **DCL Cast** 用于将视频流传入场景的智能物品。 [#957](https://github.com/decentraland/creator-hub/pull/957)
* **新动作** 在智能物品中。 [#955](https://github.com/decentraland/creator-hub/pull/955)
* **点击空白区域即可取消选择。** [#961](https://github.com/decentraland/creator-hub/pull/961)
* **“链接已复制”反馈** 在分享按钮上。 [#965](https://github.com/decentraland/creator-hub/pull/965)
* **修复**: 导入多个文件 [#931](https://github.com/decentraland/creator-hub/pull/931); 缩放多个对象时的最终位置 [#962](https://github.com/decentraland/creator-hub/pull/962).

### 0.30.0（2025年12月9日）

* **脚本组件**: 将自定义代码附加到任何物品。参见 [使用 Script 组件](/docs/creator/content-creator-zh/chang-jing-bian-ji-qi/shi-yong-dai-ma-kuo-zhan/script-component/). [#886](https://github.com/decentraland/creator-hub/pull/886)
* **按 F** 将摄像机居中到所选实体。 [#948](https://github.com/decentraland/creator-hub/pull/948)
* **场景包含自定义代码时的警告。** [#903](https://github.com/decentraland/creator-hub/pull/903)
* **自定义资源选项卡会显示教程** 当为空时。 [#925](https://github.com/decentraland/creator-hub/pull/925)
* **修复**: 一起旋转多个物品 [#946](https://github.com/decentraland/creator-hub/pull/946); 场景、摄像机和玩家实体上隐藏了“添加组件” [#923](https://github.com/decentraland/creator-hub/pull/923); 场景加载失败时的错误更清晰 [#921](https://github.com/decentraland/creator-hub/pull/921); Windows 会提示下载启动器 [#899](https://github.com/decentraland/creator-hub/pull/899); 所有重复项都保持选中 [#929](https://github.com/decentraland/creator-hub/pull/929); 视频播放器占位符方向 [#909](https://github.com/decentraland/creator-hub/pull/909); 重复编号 [#933](https://github.com/decentraland/creator-hub/pull/933); 缺少时会创建自定义物品文件夹 [#940](https://github.com/decentraland/creator-hub/pull/940); 清理未使用资源会移除空文件夹 [#924](https://github.com/decentraland/creator-hub/pull/924); 发布带有自定义代码的场景 [#941](https://github.com/decentraland/creator-hub/pull/941); 智能物品基础视图中的复选框字段 [#922](https://github.com/decentraland/creator-hub/pull/922).

### 0.29.3（2025年11月20日）

* **修复**: 新导入的场景不再卡在加载中 [#908](https://github.com/decentraland/creator-hub/pull/908); 不再出现多个 Finder 或 Explorer 窗口 [#906](https://github.com/decentraland/creator-hub/pull/906); 部署流程 [#896](https://github.com/decentraland/creator-hub/pull/896); 视频屏幕智能物品 [#898](https://github.com/decentraland/creator-hub/pull/898).

### 0.29.1 和 0.29.2（2025年11月13日）

* **修复**: 视频播放器渲染及其在组件列表中的条目，支持拖放 [#872](https://github.com/decentraland/creator-hub/pull/872), [#873](https://github.com/decentraland/creator-hub/pull/873); NFT 智能物品 [#885](https://github.com/decentraland/creator-hub/pull/885); 清理未使用资源不再破坏模型或缩略图 [#887](https://github.com/decentraland/creator-hub/pull/887), [#882](https://github.com/decentraland/creator-hub/pull/882); 依赖项更新时编辑器会变灰 [#891](https://github.com/decentraland/creator-hub/pull/891).

### 0.29.0（2025年11月5日）

* **清理未使用资源** 工具。 [#857](https://github.com/decentraland/creator-hub/pull/857)
* **场景帮助面板**: 场景的 Markdown 帮助文件会显示在编辑器中，对模板很有用。 [#833](https://github.com/decentraland/creator-hub/pull/833)
* **打开场景文件夹的快捷方式** 在 Finder 或 Explorer 中。 [#853](https://github.com/decentraland/creator-hub/pull/853)
* **修复**: 模板下载 [#845](https://github.com/decentraland/creator-hub/pull/845); NFT 智能物品合约字段 [#839](https://github.com/decentraland/creator-hub/pull/839); 摄像机和玩家实体不能设置父级 [#855](https://github.com/decentraland/creator-hub/pull/855).

### 0.28.0（2025年10月23日）

* **灯光和节点修饰器** 在编辑器中。参见 [灯光](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/lights/). [#813](https://github.com/decentraland/creator-hub/pull/813)
* **修复**: 复制的物品可以移动。 [#815](https://github.com/decentraland/creator-hub/pull/815)

### 0.27.5（2025年10月15日）

* **实体标记。** [#790](https://github.com/decentraland/creator-hub/pull/790)
* **AI 上下文文件** 在创建或更新时会下载到每个场景中，因此编程助手能了解 SDK。 [#817](https://github.com/decentraland/creator-hub/pull/817)
* **提交事件** 卡片位于“更多”部分。 [#811](https://github.com/decentraland/creator-hub/pull/811)
* **修复**: 动画权重可接受小数 [#826](https://github.com/decentraland/creator-hub/pull/826); 音效和模型缩略图 [#822](https://github.com/decentraland/creator-hub/pull/822), [#804](https://github.com/decentraland/creator-hub/pull/804); 空白预览调试器 [#821](https://github.com/decentraland/creator-hub/pull/821); 导入后资源会刷新 [#809](https://github.com/decentraland/creator-hub/pull/809); 通过链接器服务器部署 [#812](https://github.com/decentraland/creator-hub/pull/812).

### 0.25.3（2025年10月6日）

* **Chrome 开发者工具** 可在 Creator Hub 中用于调试场景。 [#766](https://github.com/decentraland/creator-hub/pull/766)
* **修复**: 创建新场景。 [#794](https://github.com/decentraland/creator-hub/pull/794)

### 0.24.1（2025年9月17日）

* **默认代码编辑器设置**: 选择 VS Code、Cursor 或其他已安装的编辑器。 [#751](https://github.com/decentraland/creator-hub/pull/751)
* **从磁盘删除文件** 作为删除项目时的一个选项。 [#787](https://github.com/decentraland/creator-hub/pull/787)
* **修复**: 纹理可以移除并从 URL 加载 [#772](https://github.com/decentraland/creator-hub/pull/772), [#779](https://github.com/decentraland/creator-hub/pull/779); 删除实体时画布会调整大小 [#770](https://github.com/decentraland/creator-hub/pull/770).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/releases.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
