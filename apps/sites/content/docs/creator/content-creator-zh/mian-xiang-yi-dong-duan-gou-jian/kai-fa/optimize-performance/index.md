> Decentraland 场景的移动端限制和性能目标，以及如何在 Creator Hub 中预览和衡量它们。

# 性能

移动设备的资源限制比桌面端或网页端更严格。了解移动场景限制以及如何预览它们，能帮助你发布一个运行流畅、覆盖最广受众的场景。

## 移动场景限制

下表显示在移动设备上运行时强制执行的限制。达到一个 **软限制** 会在性能面板中触发警告；达到一个 **硬限制** 会阻止场景加载。

| 指标         | 软限制       | 硬限制       |
| ---------- | --------- | --------- |
| 三角形        | 1,000,000 | 1,200,000 |
| 实体         | 4,800     | 6,000     |
| 网格（主体）     | 2,400     | 3,000     |
| 几何体        | 1,000     | 2,000     |
| 材质         | 400       | 500       |
| 纹理         | 400       | 500       |
| 碰撞体        | 1,200     | 1,500     |
| VFX 发射器    | 8         | 10        |
| 实时粒子       | 25,000    | 50,000    |
| 光源         | 8         | 16        |
| 阴影灯光       | 2         | 4         |
| 内容大小       | 120 MB    | 150 MB    |
| 外部内容       | 40 MB     | 50 MB     |
| 内存（进程 RSS） | 1,638 MB  | 2,048 MB  |
| 绘制调用       | 1,000     | 2,000     |
| 性能（越高越好）   | 90%       | 85%       |

## 灯光和粒子

灯光行数统计 **创建的** 场景内容，而不是引擎最终实际运行的内容。每个图形配置文件只会启用一定数量的最近光源预算（极低为 0，高为 8），并且可能完全禁用阴影——因此，一个制作了 20 盏灯的场景仍然会承担这些场景开销，即使一次最多只有 8 盏会发光。按上限制作，不要依赖运行时预算。

粒子在运行时还有额外上限：每个发射器 5,000 个粒子，每个场景 50,000 个。任何在这些上限之上制作的内容都会被静默截断，因此超过上限只会浪费场景预算。

## 预览移动端性能

你可以直接从 **Creator Hub** 中查看你的场景移动端性能，而无需部署到物理设备。

1. 在 **Creator Hub**.
2. 选择 预览 > 显示移动端二维码
3. 打开 **场景限制预览** ，从手机右上角图标中打开（带红色统计信息的显示器图标）。

![Creator Hub 中的移动场景限制面板](/docs/_remote/86e4b4fb53b120e0cdcd5a75.webp)


> [!WARNING]
> **在高画质下将性能保持在 90% 以上**
> 
> 该 **性能** 值是当前启用图形配置文件所设定 FPS 预算的百分比。得分为 **100%** 表示场景已达到完整的 FPS 目标。
> 
> 始终在中端设备上测试，例如 **Samsung Galaxy A54** 并争取 **在高图形配置文件下性能得分高于 90%**。这能为大多数移动玩家提供流畅体验。参见 [硬件要求](/docs/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/hardware-requirements/) 了解参考设备。
> 
> 你可以从 **游戏内菜单 → 设置 → 图形 → 将动态图形设为关闭 → 切换配置文件**.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/kai-fa/optimize-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
