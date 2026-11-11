# 迁移指南

本指南介绍了将组件从 UI1（`decentraland-ui`）迁移到 UI2（`decentraland-ui2`).

## 何时迁移

迁移可以基于三个合理原因开始：

### 1. 技术改进

你可能希望迁移某个组件，原因包括：

* 更好的主题支持
* 改进的 TypeScript 类型
* 与其他 UI2 组件保持一致
* 性能优化
* 可访问性改进

### 2. 更新组件

* 需要更新一个 UI1 组件
* 目前还没有对应的 UI2 版本
* **要求**: 先在 UI2 中创建它，然后再使用它

### 3. 项目需求

* 该组件将用于一个新项目或现有项目
* 有足够时间可以正确完成迁移
* 项目资源允许进行彻底迁移


> [!WARNING]
> **不要** “仅仅因为这样”就迁移组件。每次迁移都应有明确的业务或技术理由。


***

## 迁移流程

### 步骤 1：规划

在开始迁移之前：

1. **识别依赖项**
   * 它使用了哪些其他组件？
   * 哪些项目目前在使用它？
   * 是否有任何计划中的破坏性变更？
2. **审查当前使用情况**
   * 有多少项目使用这个组件？
   * 最常用的是哪些 props？
   * 是否存在已知问题？
3. **定义范围**
   * 这会是 1:1 迁移吗？
   * 是否有计划中的改进？
   * 时间线是什么？

### 步骤 2：创建 UI2 组件

遵循 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/) UI2 候选组件指南。

**要求：**

* 对 styled-components 使用对象语法
* 只使用主题值（不要使用任意值）
* 添加完整的 Storybook stories
* 编写完整测试
* 记录所有 props 和行为

**示例结构：**

```
ui2/src/components/Button/
├── Button.tsx
├── Button.styles.ts
├── Button.stories.tsx
├── Button.test.tsx
├── types.ts
├── index.ts
└── README.md
```

### 步骤 3：保持兼容性

UI2 组件 **必须** 应暴露与 UI1 版本相同的 props 和行为。

#### 相同的 Props

```tsx
// UI1 Button
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}

// UI2 Button - 必须支持相同的 props
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  // 可以添加新的可选 props
  variant?: 'text' | 'outlined' | 'contained';
}
```

#### 向后兼容的更改

如果你需要更改或新增 props：

1. **首先**，将新的 props 作为可选项添加到 UI1 中
2. **然后**，迁移 UI1 的使用方以使用新的 props
3. **最后**，使用新的 API 创建 UI2 组件

```tsx
// 第 1 步：向 UI1 添加可选 prop
interface ButtonProps {
  primary?: boolean;
  // 新的可选 prop
  variant?: 'primary' | 'secondary';
}

// 第 2 步：更新 UI1 的实现
export function Button({ primary, variant = primary ? 'primary' : 'secondary' }: ButtonProps) {
  // 内部改为使用 variant，而不是 primary
}

// 第 3 步：使用新的 API 创建 UI2
interface ButtonProps {
  // variant 现在成为主要 prop
  variant?: 'primary' | 'secondary';
  // 为兼容性保留 primary，并标记为已废弃
  /** @deprecated 请改用 variant */
  primary?: boolean;
}
```

### 步骤 4：弃用 UI1 组件

为 UI1 组件添加弃用提示：

````tsx
/**
 * @deprecated 此组件已迁移到 UI2。
 * 请改为从 'decentraland-ui2' 导入：
 * 
 * ```tsx
 * import { Button } from 'decentraland-ui2';
 * ```
 * 
 * 查看迁移指南：https://docs.decentraland.org/contributor-guides/web-ui-standards/migration
 */
export function Button(props: ButtonProps) {
  // ... 现有实现
}
````

### 步骤 5：逐步采用

不要强制立即迁移。允许逐步采用：

1. **发布** UI2 组件
2. **记录** 迁移路径
3. **更新** 新项目以使用 UI2
4. **迁移** 现有项目，视情况逐步进行
5. **规划** 最终移除 UI1（提前通知）

***

## 迁移示例

### 示例 1：简单组件

迁移一个基础的 `Card` 组件：

#### UI1 版本

```tsx
// decentraland-ui/src/components/Card/Card.tsx
import React from 'react';
import './Card.css';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

export function Card({ className, children }: CardProps) {
  return (
    <div className={`dcl-card ${className || ''}`}>
      {children}
    </div>
  );
}
```

#### UI2 版本

```tsx
// decentraland-ui2/src/components/Card/Card.tsx
import { styled } from '@mui/material/styles';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

const StyledCard = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
  boxShadow: theme.shadows[1],
  
  [theme.breakpoints.down('sm')]: {
    padding: theme.spacing(1),
  },
}));

export function Card({ className, children }: CardProps) {
  return (
    <StyledCard className={className}>
      {children}
    </StyledCard>
  );
}
```

### 示例 2：带变体的组件

迁移一个 `Button` ，带有变体：

#### UI1 版本

```tsx
// UI1
import './Button.css';

interface ButtonProps {
  primary?: boolean;
  secondary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

export function Button({ primary, secondary, size = 'medium', ...props }: ButtonProps) {
  const classes = [
    'dcl-button',
    primary && 'primary',
    secondary && 'secondary',
    `size-${size}`,
  ].filter(Boolean).join(' ');
  
  return <button className={classes} {...props} />;
}
```

#### UI2 版本

```tsx
// UI2
import { styled } from '@mui/material/styles';

interface ButtonProps {
  /** @deprecated 请改用 variant="contained" */
  primary?: boolean;
  /** @deprecated 请改用 variant="outlined" */
  secondary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
  size?: 'small' | 'medium' | 'large';
}

const StyledButton = styled('button')<ButtonProps>(({ theme, variant = 'contained', size = 'medium' }) => {
  const sizes = {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  };
  
  const variants = {
    text: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
    },
    outlined: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
      border: `1px solid ${theme.palette.primary.main}`,
    },
    contained: {
      backgroundColor: theme.palette.primary.main,
      color: theme.palette.primary.contrastText,
    },
  };
  
  return {
    padding: sizes[size],
    borderRadius: theme.shape.borderRadius,
    border: 'none',
    cursor: 'pointer',
    ...variants[variant],
    
    '&:hover': {
      opacity: 0.9,
    },
    
    '&:disabled': {
      opacity: 0.5,
      cursor: 'not-allowed',
    },
  };
});

export function Button({ 
  primary, 
  secondary, 
  variant, 
  ...props 
}: ButtonProps) {
  // 处理已废弃的 props
  const actualVariant = variant || 
    (primary ? 'contained' : secondary ? 'outlined' : 'text');
  
  return <StyledButton variant={actualVariant} {...props} />;
}
```

***

## 迁移清单

每次组件迁移都使用此清单：

### 规划阶段

* [ ] 识别所有使用该组件的项目
* [ ] 记录当前的 props 和行为
* [ ] 定义迁移范围和时间线
* [ ] 获得相关方批准

### 实施阶段

* [ ] 按照标准创建 UI2 组件
* [ ] 保持 props 兼容性
* [ ] 使用对象语法进行样式编写
* [ ] 只使用主题值
* [ ] 实现所有状态（空闲、悬停、聚焦、禁用、错误）
* [ ] 添加完整的 Storybook stories
* [ ] 编写单元测试
* [ ] 记录可访问性特性

### 弃用阶段

* [ ] 为 UI1 组件添加弃用提示
* [ ] 更新 UI1 文档
* [ ] 为使用方创建迁移指南
* [ ] 发布 UI2 组件

### 采用阶段

* [ ] 更新新项目以使用 UI2
* [ ] 为现有项目创建迁移 PR
* [ ] 监控问题
* [ ] 收集反馈
* [ ] 规划 UI1 移除时间线

***

## 常见迁移模式

### 从 CSS 到 Styled Components

```tsx
// UI1：CSS 文件
.dcl-card {
  background: #fff;
  padding: 16px;
  border-radius: 8px;
}

// UI2：样式化组件
const Card = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
}));
```

### 从 Class Name 到 Props

```tsx
// UI1：基于类的变体
<Button className={primary ? 'primary' : 'secondary'} />

// UI2：基于 prop 的变体
<Button variant={primary ? 'contained' : 'outlined'} />
```

### 固定值到主题

```tsx
// UI1：固定值
const styles = {
  color: '#333',
  fontSize: '14px',
  padding: '8px 16px',
};

// UI2：主题值
const Component = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  fontSize: theme.typography.body2.fontSize,
  padding: theme.spacing(1, 2),
}));
```

***

## 破坏性变更

有时需要进行破坏性变更。请谨慎处理：

### 何时可以接受破坏性变更

* 安全修复
* 严重错误
* 重大版本更新
* 移除已废弃功能（提前通知）

### 如何处理破坏性变更

1. **尽早宣布** - 提前充分沟通变更
2. **提供迁移路径** - 记录如何更新
3. **版本升级** - 遵循语义化版本控制
4. **弃用期** - 留出迁移时间
5. **Codemods** - 如有可能，提供自动化迁移工具

### 示例：移除已废弃的 Props

```tsx
// 版本 1.0：引入新 API，弃用旧 API
interface ButtonProps {
  /** @deprecated 请改用 variant="contained" */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// 版本 1.5：提示即将移除
interface ButtonProps {
  /** @deprecated 将在 2.0 中移除。请改用 variant */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// 版本 2.0：移除已废弃的 prop
interface ButtonProps {
  variant?: 'text' | 'outlined' | 'contained';
}
```

***

## 测试迁移

确保迁移后的组件能正确工作：

### 视觉回归测试

从视觉上比较 UI1 和 UI2 组件：

```tsx
// 用于比较的 Storybook story
export const ComparisonStory: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '2rem' }}>
      <div>
        <h3>UI1</h3>
        <UI1Button primary>Click Me</UI1Button>
      </div>
      <div>
        <h3>UI2</h3>
        <UI2Button variant="contained">Click Me</UI2Button>
      </div>
    </div>
  ),
};
```

### 行为测试

确保 props 的行为一致：

```tsx
describe('Button migration', () => {
  it('should handle primary prop (deprecated) the same as variant="contained"', () => {
    const { container: ui1 } = render(<UI1Button primary>Test</UI1Button>);
    const { container: ui2 } = render(<UI2Button primary>Test</UI2Button>);
    
    // 比较渲染输出
    expect(ui1.textContent).toBe(ui2.textContent);
  });
});
```

***

## 文档更新

迁移后，更新文档：

### 更新 UI1 组件文档

```markdown
# Button（UI1 - 已废弃）

> ⚠️ **此组件已迁移到 UI2。**
> 新版本请参见 [UI2 Button 文档](../ui2/button)。

此组件已废弃，并将在未来版本中移除。
请迁移到 UI2。

## 迁移指南

详情请参见 [迁移指南](./migration)。
```

### 创建 UI2 组件文档

```markdown
# Button（UI2）

具有完整主题支持的现代按钮组件。

## 从 UI1 迁移

如果你正在从 UI1 迁移：

- `primary` prop → `variant="contained"`
- `secondary` prop → `variant="outlined"`
- CSS 类 → styled-components

详情请参见完整的 [迁移指南](./migration)。
```

***

## 下一步

* 回顾 [自定义组件](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/custom-components/) 用于创建 UI2 组件
* 查看 [样式与主题](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/styling-and-theming/) 用于样式标准
* 检查 [流程概览](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/process-overview/) 了解完整工作流


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/web-ui-biao-zhun/migration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
