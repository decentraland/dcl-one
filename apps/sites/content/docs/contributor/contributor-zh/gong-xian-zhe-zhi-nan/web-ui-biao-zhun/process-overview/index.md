# 流程概览

我们的开发流程确保设计出来的内容就是最终实现的内容。工作流程分为三个明确阶段，每个步骤都有清晰的交付物和职责。

## 工作流程阶段

```mermaid
flowchart LR
    UX[UX 调研] --> Figma[Figma 设计] --> dApp[dApp 实现]
    
    样式 UX fill:#e3f2fd
    样式 Figma fill:#fff3e0
    样式 dApp fill:#e8f5e9
```

每个阶段都建立在前一阶段之上，确保从概念到代码的一致性。

***

## UX 调研

UX 阶段为后续的一切奠定基础。该阶段清晰的需求可以避免后期代价高昂的改动。

### 必需交付物

#### 目标与范围

**必须** 定义：

* **业务目标** - 我们要解决什么问题？
* **目标用户** - 这是为谁设计的？
* **成功指标** - 我们如何衡量成功？
* **约束条件** - 技术、时间或资源限制

**示例**:

```
目标：让用户能够高效浏览并筛选他们的土地地块
目标对象：拥有 10 个以上地块的土地所有者
成功标准：80% 的用户能在 10 秒内找到特定地块
约束：必须支持移动端；不允许更改后端
```

#### 交互流程

**必须** 梳理端到端的用户旅程：

* **所有使用场景** - 主要和次要用户路径
* **边界情况** - 空状态、首次使用用户、错误
* **替代路径** - 实现目标的不同方式
* **视觉辅助** - 如有帮助，可使用线框图或流程图

**示例**:

```
主流程：
1. 用户点击“我的土地”
2. 系统加载地块
3. 用户应用筛选条件
4. 结果立即更新

边界情况：
- 用户没有地块 → 显示带 CTA 的空状态
- 筛选后没有结果 → 显示“无结果”提示
- 加载时间超过 2 秒 → 显示骨架加载器
```

#### UI 状态

**应当** 为所有组件状态进行说明：

* **空闲** - 默认状态
* **加载中** - 数据获取或处理中
* **错误** - 操作失败
* **空** - 无可用数据
* **成功** - 操作成功
* **禁用** - 不可用操作

**应当** 概述错误恢复：

* 发生错误时会怎样？
* 用户如何重试或解决？
* 用户需要哪些信息？

#### 分析追踪

**必须** 在此阶段就应就追踪方案达成一致：

* **事件** - 要追踪的操作
* **属性** - 要采集的数据
* **用户属性** - 相关用户属性
* **转化目标** - 关键成功指标

**示例**:

```
事件：
- parcel_filter_applied { filter_type, filter_value }
- parcel_selected { parcel_id, source }
- parcel_transfer_initiated { parcel_id }

属性：
- user_parcel_count
- filter_used（布尔值）
- time_to_result（毫秒）
```

### 推荐交付物

#### 无障碍意图

**应当** 描述：

* **键盘流程** - Tab 顺序和快捷键
* **焦点顺序** - 合乎逻辑的焦点推进
* **屏幕阅读器标签** - ARIA 标签和描述
* **替代内容** - 图片、图标、图表的文本说明

**示例**:

```
键盘导航：
- Tab：在筛选器之间导航
- Enter：应用所选筛选器
- Escape：关闭筛选下拉菜单
- 方向键：浏览筛选选项

屏幕阅读器：
- “按以下条件筛选：所有者地址。可输入搜索或从列表中选择”
- “找到 3 个符合你筛选条件的地块”
```

#### 无障碍可读性

**应当** 确保：

* **颜色对比度** - 符合 WCAG AA 最低比率
  * 普通文本：4.5:1
  * 大号文本（18pt+）：3:1
  * UI 组件：3:1
* **文本大小** - 在默认字号下可读
* **交互目标** - 最小 44×44px 触控目标

#### UI 交互元素

**应当** 为所有交互元素定义状态：

* **启用** - 默认交互状态
* **禁用** - 非交互状态
* **悬停** - 鼠标悬停（桌面端）
* **按下/激活** - 交互过程中
* **焦点** - 键盘焦点状态

**应当** 说明用户反馈：

* 交互时的视觉变化
* 加载指示器
* 成功/错误消息
* 触觉反馈（移动端）

***

## Figma 设计

设计师必须从我们的 [Figma 组件库](https://www.figma.com/design/tsyaDSedcsVZ8iM9N0McT2/DCL-UI2)开始，它与 Material UI 组件保持一致，并使用 UI2 中定义的 Decentraland 主题。


> [!WARNING]
> 不应使用特殊颜色、尺寸或样式。可能存在例外，但必须在规范中说明理由，并在评审中获批。


### 必需标准

#### 使用 Decentraland Figma 组件库

**必须** 当存在对应组件时，使用 MUI 组件：

* 优先检查 MUI 组件库
* 使用 Decentraland 变体和样式
* 不要为已有组件创建自定义版本

**如果需要的 UI 元素不存在：**

1. 检查是否可以由现有组件组合而成
2. 如果不行，请提出自定义组件（见 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/))
3. 在规范中记录理由
4. 在设计评审中获得批准

#### 颜色

**必须** 将 Figma 颜色样式与 UI2 主题名称对应：

```tsx
import { dclColors } from 'decentraland-ui2';

// Figma “Rarity / Unique” → 代码
dclColors.rarity.unique

// Figma “Primary / Main” → 代码
theme.palette.primary.main

// Figma “Text / Secondary” → 代码
theme.palette.text.secondary
```

**事实来源**: [colors.ts](https://github.com/decentraland/ui2/blob/master/src/theme/colors.ts)

#### 排版

**必须** 使用已定义的变体：

可用变体：

* `h1`, `h2`, `h3`, `h4`, `h5`, `h6` - 标题
* `subtitle1`, `subtitle2` - 副标题
* `body1`, `body2` - 正文
* `button` - 按钮文本
* `caption` - 说明文字
* `overline` - 上划线文本

**参考**:

* [排版来源](https://github.com/decentraland/ui2/blob/master/src/theme/typography.ts)
* [MUI Typography](https://mui.com/material-ui/react-typography/)
* [Material Design 字体系统](https://m2.material.io/design/typography/the-type-system.html#type-scale)

**示例映射**:

```
Figma “H1” → <Typography variant="h1">
Figma “Body 1” → <Typography variant="body1">
Figma “Caption” → <Typography variant="caption">
```

#### 断点

**必须** 为以下断点设计：

| 名称   | 宽度     | 典型设备  |
| ---- | ------ | ----- |
| `xs` | 768px  | 移动端   |
| `sm` | 991px  | 平板    |
| `md` | 1024px | 小型桌面端 |
| `lg` | 1280px | 桌面端   |
| `xl` | 1500px | 大型桌面端 |

**来源**: [index.ts](https://github.com/decentraland/ui2/blob/master/src/theme/index.ts)

**最佳实践**:

* 采用移动优先设计（从 `xs`)
* 在 Figma 框架中显示关键断点
* 记录响应式行为
* 在视口边界进行测试（767px、768px 等）

#### 圆角与调色板

**必须** 使用主题定义的值：

* 不要引入新的圆角值
* 不要创建新的调色板角色
* 例外情况需要在规范中说明理由并获批

**主题值**:

```tsx
theme.shape.borderRadius // 默认圆角
theme.palette.primary    // 主要颜色
theme.palette.secondary  // 次要颜色
theme.palette.error      // 错误颜色
theme.palette.text       // 文本颜色
theme.palette.background // 背景颜色
theme.palette.divider    // 分隔线颜色
```

#### 配色方案

**如果屏幕支持多种配色方案：**

**必须** 提供：

* 哪个方案生效（见 [colorSchemes.ts](https://github.com/decentraland/ui2/blob/master/src/theme/colorSchemes.ts))
* 浅色模式预览
* 深色模式预览
* 关于在 Storybook 中切换方案的文档

**示例**:

```
支持的方案：浅色、深色
默认：系统偏好
切换：设置菜单 → 外观
Storybook：在工具栏中使用“Theme”控制项
```

### 组件选择

**应当** 复用已准备好的组件：

1. **先检查 MUI** - MUI 是否有这个组件？
2. **检查 UI2** - 是否有 Decentraland 变体？
3. **如有可能则组合** - 能否将现有组件组合起来？
4. **最后才使用自定义** - 遵循 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/) process

***

## dApp 实现

开发者使用 UI2 组件实现设计，并遵循我们的 [样式与主题](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming/) 规范。

### 实现检查清单

* [ ] 若有可用的 UI2 组件，先从它开始
* [ ] 如果没有对应的 UI2 组件，则使用带有 Decentraland 主题的 MUI 组件
* [ ] 遵循 [样式与主题](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming/) 规范
* [ ] 实现 UX 规范中的所有状态
* [ ] 按说明添加分析追踪
* [ ] 在所有断点上测试
* [ ] 验证键盘导航
* [ ] 检查颜色对比度
* [ ] 使用屏幕阅读器测试
* [ ] 处理加载和错误状态
* [ ] 如果是可复用组件，则加入 Storybook

### 组件实现流程

```
1. 在 UI2 中查找组件
   ↓
2. 找到了？→ 使用它
   ↓
3. 没找到？→ 检查 MUI
   ↓
4. 在 MUI 中找到了？→ 使用 Decentraland 主题
   ↓
5. 需要定制？→ 查看自定义组件指南
   ↓
6. 按照样式规范实现
   ↓
7. 加入 Storybook（如果可复用）
```

### 质量门槛

在标记实现完成前：

1. **视觉一致** - 在关键断点上与 Figma 像素级一致
2. **主题合规** - 所有值都来自 UI2 主题
3. **状态已实现** - 所有 UX 规定的状态都可正常工作
4. **可访问性** - 键盘导航、焦点状态、ARIA 标签
5. **分析** - 事件按规定触发
6. **响应式** - 在所有断点上都能正常工作
7. **性能** - 没有不必要的重新渲染
8. **测试** - 组件测试通过
9. **Storybook** - 已添加故事（如果可复用）
10. **代码评审** - 已由维护者批准

***

## 交接最佳实践

### 从 UX 到设计

* 清晰的需求文档
* 带注释的用户流程
* 复杂交互的状态图
* 分析事件规范

### 从设计到开发

* 已启用开发者模式的 Figma 文件
* 组件规范
* 响应式行为说明
* 颜色和排版 Token 映射
* 无障碍注释
* UX 需求链接

### 实现过程中

* 设计师与开发者之间定期同步
* 对技术限制的早期反馈
* 在关键里程碑进行迭代评审
* 合并前的最终评审

***

## 下一步

* 了解 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/) 用于创建新组件
* 回顾 [样式与主题](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming/) 用于实现细节
* 查看 [迁移指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/migration/) 如果使用 UI1 组件


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/process-overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
