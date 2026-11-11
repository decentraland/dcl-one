# 自定义组件

我们区分两种类型的自定义组件，它们各自有不同的流程和期望。

## 组件类型

### A) 项目特定自定义组件

为特定项目或界面构建的组件，不打算在其他项目中复用。

**示例：**

* 一个 `Box` 在一个项目中使用的特殊布局
* 一个 `Card` 为某个项目的界面定制布局的变体
* 项目特定的数据可视化
* 一次性布局组件

**何时使用：**

* 组件解决某个项目特有的问题
* 其他项目不太可能需要
* 过于具体，无法泛化

### B) UI2 候选组件

旨在跨多个项目和产品复用的组件。

**示例：**

* `Navbar` - 全站导航
* `UserMenu` - 用户账户菜单
* 标准化 `Modal` 对话框
* 从 UI1 迁移过来的组件

**何时使用：**

* 组件将在多个项目中使用
* 代表一种常见的 Decentraland 模式
* 替代或扩展 UI1 组件

***

## 项目特定组件

### 要求

#### 以 MUI 为基础

**必须** 尽可能扩展现有的 MUI 组件：

```tsx
// ✅ 好：扩展 MUI Card
import { Card as MuiCard } from '@mui/material';
import { styled } from '@mui/material/styles';

const ProjectCard = styled(MuiCard)(({ theme }) => ({
  padding: theme.spacing(3),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

// ❌ 差：从零构建
const ProjectCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: '4px',
  boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  // 重复实现 Card 功能
}));
```

**不要分叉或重复 MUI 已经覆盖的模式：**

* 为简单的 UI 状态使用 `Card` 不要创建带阴影的自定义盒子
* 为简单的 UI 状态使用 `Button` 不要创建样式化的锚点
* 为简单的 UI 状态使用 `TextField` 不要创建自定义输入框
* 扩展 `Dialog` 不要创建自定义模态框

#### 仅使用主题值

**必须** 使用 UI2 主题中的值：

```tsx
// ✅ 好：所有值都来自主题
const StyledBox = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  border: `1px solid ${theme.palette.divider}`,
}));

// ❌ 差：临时硬编码值
const StyledBox = styled('div')({
  color: '#333333',
  backgroundColor: '#FFFFFF',
  padding: '16px',
  borderRadius: '8px',
  border: '1px solid #E0E0E0',
});
```

**不允许使用任意值：**

* 颜色：使用 `theme.palette` 或 `dclColors`
* 间距：使用 `theme.spacing(n)`
* 边框圆角：使用 `theme.shape.borderRadius`
* 排版：使用 `theme.typography` 变体
* 断点：使用 `theme.breakpoints` 辅助函数

#### 状态与可访问性

**必须** 定义并实现所有交互状态：

```tsx
const ActionButton = styled('button')(({ theme }) => ({
  // 基础/空闲状态
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  transition: theme.transitions.create(['background-color', 'transform']),
  
  // 悬停状态
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // 聚焦状态（键盘导航）
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
  },
  
  // 激活/按下状态
  '&:active': {
    transform: 'scale(0.98)',
  },
  
  // 禁用状态
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
  },
}));
```

**必须** 实现基础可访问性：\*\*

* **键盘导航** - 可通过键盘聚焦并操作
* **焦点指示** - 可见的焦点状态
* **ARIA 标签** - 在文本不可见时使用
* **语义化 HTML** - 使用适当的元素
* **颜色对比度** - 符合 WCAG AA 标准

### 示例：项目特定组件

```tsx
// src/components/LandCard/LandCard.tsx
import { Card, CardContent, CardActions, Typography, Button } from '@mui/material';
import { styled } from '@mui/material/styles';
import type { Parcel } from '@/types';

interface LandCardProps {
  parcel: Parcel;
  onTransfer: (id: string) => void;
  onView: (id: string) => void;
}

const StyledCard = styled(Card)(({ theme }) => ({
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  transition: theme.transitions.create('transform'),
  
  '&:hover': {
    transform: 'translateY(-4px)',
  },
}));

const CoordinatesText = styled(Typography)(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontFamily: theme.typography.fontFamilyMono,
}));

export function LandCard({ parcel, onTransfer, onView }: LandCardProps) {
  return (
    <StyledCard>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {parcel.name || `地块 ${parcel.x},${parcel.y}`}
        </Typography>
        <CoordinatesText variant="body2">
          ({parcel.x}, {parcel.y})
        </CoordinatesText>
        <Typography variant="body2" color="text.secondary">
          拥有者：{parcel.owner}
        </Typography>
      </CardContent>
      <CardActions>
        <Button size="small" onClick={() => onView(parcel.id)}>
          查看
        </Button>
        <Button size="small" onClick={() => onTransfer(parcel.id)}>
          转移
        </Button>
      </CardActions>
    </StyledCard>
  );
}
```

***

## UI2 候选组件

将在项目之间共享的组件需要更高标准和更完整的文档。

### 要求

#### 主题一致性

**必须** 完全依赖 UI2 主题值：

```tsx
// ✅ 好：完整集成主题
const NavbarContainer = styled('nav')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderBottom: `1px solid ${theme.palette.divider}`,
  padding: theme.spacing(0, 2),
  height: 64,
  display: 'flex',
  alignItems: 'center',
  gap: theme.spacing(2),
  
  [theme.breakpoints.down('md')]: {
    padding: theme.spacing(0, 1),
  },
}));
```

#### Storybook 覆盖

**必须** 添加完整的 Storybook stories：

**必需覆盖：**

1. **所有属性和变体**
   * 每种属性组合
   * 所有尺寸变体
   * 所有颜色变体
2. **所有状态**
   * 空闲/默认
   * 加载中
   * 错误
   * 禁用
   * 悬停（通过 pseudo states 插件）
   * 聚焦（通过 pseudo states 插件）
3. **交互**
   * 点击处理器
   * 表单提交
   * 键盘导航
4. **配色方案**
   * 浅色模式
   * 深色模式
5. **响应式行为**
   * 关键断点（xs、md、lg）
   * 记录每个断点下的行为

**示例 Storybook 文件：**

```tsx
// Navbar.stories.tsx
import type { Meta, StoryObj } from '@storybook/react';
import { Navbar } from './Navbar';

const meta: Meta<typeof Navbar> = {
  title: '组件/Navbar',
  component: Navbar,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'compact'],
    },
    showUserMenu: {
      control: 'boolean',
    },
  },
};

export default meta;
type Story = StoryObj<typeof Navbar>;

export const Default: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
};

export const Compact: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
};

export const WithoutUserMenu: Story = {
  args: {
    variant: 'default',
    showUserMenu: false,
  },
};

export const Loading: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
    isLoading: true,
  },
};

// 测试不同视口
export const Mobile: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
};

export const Tablet: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
  },
};

// 测试配色方案
export const DarkMode: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    backgrounds: {
      default: 'dark',
    },
  },
};
```

### 组件结构

UI2 候选组件应遵循以下结构：

```
src/components/Navbar/
├── Navbar.tsx           # 主组件
├── Navbar.styles.ts     # 样式化组件
├── Navbar.stories.tsx   # Storybook stories
├── Navbar.test.tsx      # 单元测试
├── types.ts             # TypeScript 类型
├── index.ts             # 公共导出
└── README.md            # 组件文档
```

### 文档要求

**必须** 在组件 README 中包含：

1. **目的** - 解决了什么问题？
2. **用法** - 如何使用该组件
3. **属性** - 所有属性及其类型和说明
4. **示例** - 常见用例
5. **可访问性** - 键盘支持、ARIA 标签
6. **主题** - 使用了哪些主题值
7. **迁移说明** - 如果替换的是 UI1 组件

**示例 README：**

````markdown
# 导航栏

带用户菜单和响应式行为的全站导航组件。

## 使用

\```tsx
import { Navbar } from 'decentraland-ui2';

function App() {
  return (
    <Navbar
      variant="default"
      showUserMenu={true}
      onLogoClick={() => navigate('/')}
      onLoginClick={handleLogin}
    />
  );
}
\```

## 属性

| 属性         | 类型                   | 默认值   | 说明                        |
| ------------ | ---------------------- | --------- | ----------------------------- |
| variant      | 'default' \| 'compact' | 'default' | 导航变体                    |
| showUserMenu | boolean                | true      | 登录时显示用户菜单          |
| onLogoClick  | () => void             | -         | Logo 点击处理器             |
| onLoginClick | () => void             | -         | 登录按钮点击处理器          |

## 可访问性

* 键盘导航：使用 Tab 在菜单项之间切换
* ARIA：正确的地标和标签
* 屏幕阅读器：会播报菜单状态

## 主题

使用以下主题值：

* `theme.palette.background.paper`
* `theme.palette.divider`
* `theme.spacing`
* `theme.breakpoints`

````

### 测试要求

**必须** 包含以下测试：

* 属性渲染
* 用户交互
* 可访问性特性
* 响应式行为
* 错误状态

```tsx
// Navbar.test.tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Navbar } from './Navbar';

describe('Navbar', () => {
  it('应渲染徽标', () => {
    render(<Navbar />);
    expect(screen.getByRole('banner')).toBeInTheDocument();
  });

  it('在点击徽标时应调用 onLogoClick', async () => {
    const onLogoClick = jest.fn();
    render(<Navbar onLogoClick={onLogoClick} />);
    
    await userEvent.click(screen.getByRole('link', { name: /decentraland/i }));
    expect(onLogoClick).toHaveBeenCalled();
  });

  it('应可通过键盘导航', async () => {
    render(<Navbar />);
    const firstLink = screen.getAllByRole('link')[0];
    
    firstLink.focus();
    expect(firstLink).toHaveFocus();
  });
});
```

***

## 决策矩阵

使用此表决定要创建哪种类型的组件：

| 问题                | 项目特定 | UI2 候选    |
| ----------------- | ---- | --------- |
| 其他项目会使用它吗？        | 否    | 是         |
| UI1 有对应项吗？        | 不适用  | 可能        |
| 需要 Storybook 文档吗？ | 否    | **是**     |
| 需要全面测试吗？          | 基础   | **全面**    |
| 需要设计评审吗？          | 项目级  | **UI2 级** |
| 可以使用项目特定模式吗？      | 是    | **否**     |
| 必须在所有主题下工作吗？      | 否    | **是**     |

***

## 审批流程

### 项目特定组件

1. 由项目维护者进行代码审查
2. 验证主题合规性
3. 在项目上下文中测试
4. 批准后合并

### UI2 候选组件

1. 设计评审与批准
2. 技术设计评审
3. 实现
4. Storybook 故事
5. 全面测试
6. 可访问性评审
7. 代码评审
8. 向 UI2 仓库提交 PR
9. 版本化并发布
10. 更新依赖项目

***

## 最佳实践

### 组合优于定制

```tsx
// ✅ 好：组合 MUI 组件
function FeatureCard({ title, children }) {
  return (
    <Card>
      <CardContent>
        <Typography variant="h6">{title}</Typography>
        {children}
      </CardContent>
    </Card>
  );
}

// ❌ 差：重新实现 Card 功能
function FeatureCard({ title, children }) {
  return (
    <div className="custom-card">
      <div className="custom-card-content">
        <h3>{title}</h3>
        {children}
      </div>
    </div>
  );
}
```

### 渐进式增强

从简单开始，并根据需要添加功能：

1. 包含核心功能的基础版本
2. 添加响应式行为
3. 添加无障碍功能
4. 添加高级交互
5. 优化性能

### 文档优先

在编写代码之前：

1. 编写组件 README
2. 定义 props 接口
3. 列出所需状态
4. 规划 Storybook 故事
5. 然后实现

***

## 下一步

* 回顾 [样式与主题](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming/) 用于实现细节
* 查看 [迁移指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/migration/) 用于从 UI1 到 UI2 的迁移
* 检查 [流程概览](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/process-overview/) 用于完整工作流程


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
