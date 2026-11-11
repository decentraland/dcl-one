# 样式与主题

本页涵盖使用 styled-components 结合 Material UI 的样式方案为 Decentraland Web UI 制定的完整样式标准。


> [!NOTE]
> 本文档中的所有代码示例仅用于说明。它们并不代表生产组件，而是用于演示模式和标准。


## 核心原则

1. **仅使用对象语法** - 使用对象表示法以获得良好的 TypeScript 支持
2. **以主题为先** - 所有值都来自 UI2 主题
3. **不使用内联样式** - 所有样式都使用 styled 组件
4. **所有内容都加类型** - 为 props 和 theme 充分利用 TypeScript
5. **默认响应式** - 使用主题断点

***

## 对象语法标准

UI2 组件 **必须** 使用对象语法。这可确保良好的 TypeScript 支持、csstype 校验以及直接的主题集成。


> [!WARNING]
> 模板字面量语法 **不允许**。始终使用对象表示法。


### 基础示例

```tsx
// ✅ 好：对象语法
const Button = styled('button')({
  color: 'turquoise',
  padding: '8px 16px',
});

// ❌ 差：模板字面量语法
const Button = styled.button`
  color: turquoise;
  padding: 8px 16px;
`;
```

### 结合 Theme 和 Props

```tsx
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

// ✅ 好：结合 theme 和 props 的对象语法
const Button = styled('button')<ButtonProps>(({ theme, primary, size = 'medium' }) => ({
  color: primary ? theme.palette.primary.main : theme.palette.text.primary,
  backgroundColor: primary ? theme.palette.primary.main : 'transparent',
  borderRadius: theme.shape.borderRadius,
  padding: {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  }[size],
}));
```


> [!CAUTION]
> **规则**: 值必须始终来自 UI2 主题。不允许使用任意十六进制色值或像素值。


***

## 元素语法

为原生 HTML 元素设置样式时，始终使用函数调用形式 `styled('tag')`.

### 正确语法

```tsx
// ✅ 好：函数调用形式
const Container = styled('div')({
  display: 'flex',
  flexDirection: 'column',
});

const Action = styled('button')(({ theme }) => ({
  color: theme.palette.primary.main,
  padding: theme.spacing(1, 2),
}));

const Label = styled('label')(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontSize: theme.typography.caption.fontSize,
}));
```

### 错误语法

```tsx
// ❌ 差：属性形式（旧语法）
const Container = styled.div`
  display: flex;
  flex-direction: column;
`;

const Action = styled.button`
  color: ${props => props.theme.palette.primary.main};
`;
```

***

## 不使用内联样式

内联样式（`style={...}`) **绝不能** 用于 UI2 组件中。

### 为什么不用内联样式？

* 绕过 theme 类型检查
* 更难维护
* 无法复用
* 无法优化
* 没有 TypeScript 校验

### 正确做法

```tsx
// ❌ 差：内联样式
<Card 
  key={id} 
  style={{ backgroundColor: color } as React.CSSProperties}
>
  <CardHeader>
    <Title style={{ fontSize: 20 }}>{title}</Title>
  </CardHeader>
</Card>

// ✅ 好：带 props 的 styled 组件
interface CardProps {
  backgroundColor: string;
}

interface TitleProps {
  size: number;
}

const StyledCard = styled('div')<CardProps>(({ backgroundColor, theme }) => ({
  backgroundColor,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
}));

const Title = styled('h2')<TitleProps>(({ size, theme }) => ({
  fontSize: theme.typography.pxToRem(size),
  color: theme.palette.text.primary,
}));

<StyledCard key={id} backgroundColor={color}>
  <CardHeader>
    <Title size={20}>{title}</Title>
  </CardHeader>
</StyledCard>
```

***

## 断点

为简单的 UI 状态使用 `theme.breakpoints` 辅助工具，而不是硬编码像素值。

### 断点辅助函数

| 辅助函数                  | 用法                                      | 描述      |
| --------------------- | --------------------------------------- | ------- |
| `up(key)`             | `theme.breakpoints.up('md')`            | 最小宽度及以上 |
| `down(key)`           | `theme.breakpoints.down('md')`          | 最大宽度及以下 |
| `between(start, end)` | `theme.breakpoints.between('sm', 'lg')` | 两个断点之间  |
| `only(key)`           | `theme.breakpoints.only('md')`          | 仅在此断点   |

### 示例

```tsx
// ❌ 差：硬编码断点
const Panel = styled('div')`
  @media (max-width: 768px) {
    width: 100%;
  }
`;

// ✅ 好：主题断点
interface PanelProps {
  expanded: boolean;
}

const Panel = styled('div')<PanelProps>(({ theme, expanded }) => ({
  width: expanded ? '400px' : '0',
  transition: theme.transitions.create('width'),
  
  [theme.breakpoints.down('sm')]: {
    width: expanded ? '100%' : '0',
  },
}));
```

### 多个断点

```tsx
const Layout = styled('div')(({ theme }) => ({
  display: 'grid',
  gridTemplateColumns: '1fr 320px',
  gap: theme.spacing(2),
  
  // 移动端：单列
  [theme.breakpoints.down('md')]: {
    gridTemplateColumns: '1fr',
  },
  
  // 大屏桌面：更宽的侧边栏
  [theme.breakpoints.up('xl')]: {
    gridTemplateColumns: '1fr 400px',
  },
  
  // 平板范围：不同间距
  [theme.breakpoints.between('sm', 'lg')]: {
    gap: theme.spacing(3),
  },
  
  // 仅平板
  [theme.breakpoints.only('md')]: {
    padding: theme.spacing(2),
  },
}));
```

***

## 间距尺度

为简单的 UI 状态使用 `theme.spacing` 仅用于外边距、内边距和间距。

### 间距约定

* `theme.spacing(n)` 其中 `n` 是一个数字
* 基础单位通常为 8px
* `spacing(1)` = 8px， `spacing(2)` = 16px，等等。
* 允许小数： `spacing(1.5)` = 12px

```tsx
// ✅ 好：主题间距
const Box = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),           // 16px
  margin: theme.spacing(1, 0),         // 垂直 8px，水平 0
  gap: theme.spacing(1.5),             // 12px
  paddingInline: theme.spacing(3),     // 左右 24px
}));

// ❌ 差：原始像素值
const Box = styled('div')({
  padding: '16px',
  margin: '8px 0',
  gap: '12px',
});
```

### 常见间距模式

```tsx
const Card = styled('div')(({ theme }) => ({
  // 四周等间距
  padding: theme.spacing(3),
  
  // 垂直/水平不同
  padding: theme.spacing(2, 3),  // 垂直 16px，水平 24px
  
  // 四边都不同
  padding: theme.spacing(1, 2, 3, 2),  // 上、右、下、左
  
  // 逻辑属性（为 RTL 支持推荐）
  paddingBlock: theme.spacing(2),      // 上下
  paddingInline: theme.spacing(3),     // 左右
  marginBlockStart: theme.spacing(1),  // margin-top
}));
```

***

## z-index 和层叠

遵循主题的 z-index 尺度。切勿使用任意 z-index 值。

### 主题 z-index 值

```tsx
theme.zIndex.mobileStepper  // 1000
theme.zIndex.fab            // 1050
theme.zIndex.speedDial      // 1050
theme.zIndex.appBar         // 1100
theme.zIndex.drawer         // 1200
theme.zIndex.modal          // 1300
theme.zIndex.snackbar       // 1400
theme.zIndex.tooltip        // 1500
```

### 正确用法

```tsx
// ✅ 好：主题 z-index
const StickyBar = styled('div')(({ theme }) => ({
  position: 'sticky',
  top: 0,
  zIndex: theme.zIndex.appBar,
  backgroundColor: theme.palette.background.paper,
}));

const Overlay = styled('div')(({ theme }) => ({
  position: 'fixed',
  inset: 0,
  zIndex: theme.zIndex.modal,
  backgroundColor: 'rgba(0, 0, 0, 0.5)',
}));

// ❌ 差：任意 z-index
const StickyBar = styled('div')({
  position: 'sticky',
  top: 0,
  zIndex: 999,  // 别这样做
});
```

### 层叠上下文最佳实践

* 注意创建新的层叠上下文
* 避免不必要的 `position: relative` 在父元素上使用
* 记录为什么需要 z-index
* 如果需要一个新层级，先把它添加到主题中

***

## 颜色令牌

始终从以下位置获取颜色： `theme.palette` 和 `dclColors`。不要使用临时的十六进制值。

### 调色板结构

```tsx
// 文本颜色
theme.palette.text.primary
theme.palette.text.secondary
theme.palette.text.disabled

// 背景颜色
theme.palette.background.default
theme.palette.background.paper

// 主色/次色/错误色
theme.palette.primary.main
theme.palette.primary.light
theme.palette.primary.dark
theme.palette.primary.contrastText

// 操作颜色
theme.palette.action.active
theme.palette.action.hover
theme.palette.action.selected
theme.palette.action.disabled
theme.palette.action.disabledBackground

// 分隔线
theme.palette.divider
```

### Decentraland 颜色

```tsx
import { dclColors } from 'decentraland-ui2';

// 稀有度颜色
dclColors.rarity.unique
dclColors.rarity.mythic
dclColors.rarity.legendary
dclColors.rarity.epic
dclColors.rarity.rare
dclColors.rarity.uncommon
dclColors.rarity.common
```

### 示例

```tsx
// ✅ 好：主题颜色
const Chip = styled('span')(({ theme }) => ({
  color: theme.palette.text.secondary,
  backgroundColor: theme.palette.background.paper,
  borderColor: theme.palette.divider,
  
  '&:hover': {
    backgroundColor: theme.palette.action.hover,
  },
}));

const RarityBadge = styled('span')<{ rarity: string }>(({ rarity }) => ({
  backgroundColor: dclColors.rarity[rarity],
  color: '#FFFFFF', // 对比色 - 如有文档说明则可接受
  padding: '4px 8px',
  borderRadius: '4px',
}));

// ❌ 差：任意十六进制值
const Chip = styled('span')({
  color: '#666666',
  backgroundColor: '#FFFFFF',
  borderColor: '#E0E0E0',
});
```

***

## 交互状态

所有可交互控件都必须显示悬停、聚焦、 सक्रिय和禁用状态。

### 完整的交互式组件

```tsx
const InteractiveButton = styled('button')(({ theme }) => ({
  // 基础状态
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  outline: 'none',
  transition: theme.transitions.create([
    'background-color',
    'transform',
    'box-shadow',
  ]),
  
  // 悬停状态（鼠标）
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // 聚焦状态（键盘导航）
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
    boxShadow: theme.shadows[2],
  },
  
  // 激活/按下状态
  '&:active': {
    backgroundColor: theme.palette.primary.dark,
    transform: 'scale(0.98)',
  },
  
  // 禁用状态
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
    transform: 'none',
  },
}));
```

### Focus-Visible 模式

始终使用 `:focus-visible` 而不是 `:focus` 以避免在鼠标点击时显示焦点环：

```tsx
// ✅ 好：仅在键盘操作时显示焦点环
'&:focus-visible': {
  outline: `2px solid ${theme.palette.primary.main}`,
  outlineOffset: 2,
}

// ❌ 坏：每次点击都显示焦点环
'&:focus': {
  outline: `2px solid ${theme.palette.primary.main}`,
}
```

***

## 排版

请使用主题排版变体，而不是自定义字体属性。

```tsx
// ✅ 好：排版变体
const Heading = styled('h1')(({ theme }) => ({
  ...theme.typography.h1,
  marginBottom: theme.spacing(2),
}));

const Body = styled('p')(({ theme}) => ({
  ...theme.typography.body1,
  color: theme.palette.text.secondary,
}));

// 使用 MUI Typography 组件（推荐）
<Typography variant="h1">Heading</Typography>
<Typography variant="body1">正文文本</Typography>

// ❌ 坏：自定义排版
const Heading = styled('h1')({
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: 1.2,
});
```

***

## 性能优化

### 不要在渲染中创建样式组件

```tsx
// ❌ 坏：每次渲染都会创建新组件
function Component({ color }) {
  const Box = styled('div')({
    backgroundColor: color,
  });
  return <Box />;
}

// ✅ 好：创建一次，传递属性
const Box = styled('div')<{ color: string }>(({ color }) => ({
  backgroundColor: color,
}));

function Component({ color }) {
  return <Box color={color} />;
}
```

### 记忆化派生属性

```tsx
// ❌ 坏：每次渲染都会创建新对象
<StyledComponent style={{ color: isDark ? 'white' : 'black' }} />

// ✅ 好：传递原始属性
const StyledComponent = styled('div')<{ isDark: boolean }>(({ isDark, theme }) => ({
  color: isDark ? theme.palette.common.white : theme.palette.common.black,
}));

<StyledComponent isDark={isDark} />
```

***

## 命名约定

### 组件名称

* **PascalCase** 用于组件
* 具有描述性的名称

```tsx
// ✅ 好的命名
const UserCard = styled('div')({...});
const PrimaryButton = styled('button')({...});
const NavigationList = styled('ul')({...});

// ❌ 不好的命名
const card = styled('div')({...});
const btn = styled('button')({...});
const list1 = styled('ul')({...});
```

### 文件结构

```
Component.tsx        # 主组件
Component.styles.ts  # 样式组件
Component.test.tsx   # 测试
Component.stories.tsx # Storybook
```

***

## 下一步

* 回顾 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/) 用于组件创建指南
* 查看 [迁移指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/migration/) 用于从 UI1 到 UI2 的迁移
* 检查 [流程概览](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/process-overview/) 了解完整工作流


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
