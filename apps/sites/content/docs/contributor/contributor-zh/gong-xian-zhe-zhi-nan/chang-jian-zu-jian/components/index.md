# 组件

组件是独立封装的软件单元，被设计为黑盒，接收其他组件（或不接收任何组件）来完成其任务。主要目标是解耦代码模块并最大化可测试性。

## 目录结构

所有组件必须定义在一个专用目录中，且至少包含以下四个文件：

* `component.ts` - 包含组件的实现代码
* `types.ts` - 包含组件的接口和导出的类型
* `errors.ts` - 包含所有导出的错误类
* `index.ts` - 导出组件的公共 API

### 示例结构

```
src/
└── components/
    └── my-component/
        ├── component.ts
        ├── types.ts
        ├── errors.ts
        └── index.ts
```

## 类型文件

组件遵循一个定义其公共 API 的接口。该接口可以在可互换的组件之间共享，或用于了解组件暴露的方法。

所有拥有自身接口的组件必须在一个 `types.ts` 组件目录内的文件中定义它。

组件暴露的其他类型也必须放在此文件中。

### 示例： `types.ts`

```tsx
export type Something = {
  aValue: boolean
  anotherValue: string
}

export interface IMyComponent {
  myFunction: () => boolean
  getSomething: () => Something
}
```

### 共享类型

在多个组件之间共享的类型，例如通用接口，必须放在一个 `types.ts` 项目源代码根目录中的文件里（`/src/types.ts`).

## 组件文件

组件文件包含组件创建函数。该函数遵循特定的命名和结构约定。

### 命名约定

组件创建函数必须命名为： `create` + `组件名称` + `组件`

**示例：** `createMyNewComponent`

### 函数签名

创建函数必须：

1. 接收一个 components 对象作为第一个参数，包含所有依赖项
2. 可选地接收额外的配置参数
3. 返回一个包含暴露（公共）方法的对象

### 组件结构

在组件创建函数开始处：

1. 从 components 对象中提取依赖项
2. 初始化公共变量（例如日志记录器、配置）
3. 定义内部辅助函数
4. 定义公共方法
5. 返回公共 API

### 示例： `component.ts`

```tsx
import { IMyComponent, Something } from './types'
import { WrongStringError } from './errors'

export function createMyNewComponent(
  components: Pick<AppComponents, 'logs'>
): IMyComponent {
  const { logs } = components
  const logger = logs.getLogger('my-component')

  // 内部方法 - 未暴露
  function computeString(fstString: string, sndString: string): string {
    if (fstString.length === 0) {
      throw new WrongStringError()
    }
    
    return fstString + ' ' + sndString
  }

  // 公共方法 - 在返回对象中暴露
  function myFunction(): boolean {
    return true
  }

  // 公共方法 - 在返回对象中暴露
  function getSomething(): Something {
    logger.info('获取某些内容')
    
    return {
      aValue: true,
      anotherValue: computeString('aString', 'anotherString')
    }
  }

  // 返回公共 API
  return {
    myFunction,
    getSomething
  }
}
```

## 错误文件

错误文件包含组件可以抛出的自定义错误类。这些错误可在其他组件或控制器中使用，以正确识别并处理特定错误情况。

### 示例： `errors.ts`

```tsx
export class WrongStringError extends Error {
  constructor(message?: string) {
    super(message || '错误的字符串')
    this.name = 'WrongStringError'
  }
}

export class ComponentNotInitializedError extends Error {
  constructor() {
    super('组件尚未初始化')
    this.name = 'ComponentNotInitializedError'
  }
}
```

### 错误处理最佳实践

* 针对不同的错误情况创建专门的错误类
* 包含有意义的错误消息
* 将 `name` 将 property 设置为与类名一致，以便更易调试
* 记录每个公共方法可能抛出的错误

## 索引文件

索引文件作为组件的公共入口点，只导出应用程序其他部分应可访问的内容。

### 示例： `index.ts`

```tsx
export { createMyNewComponent } from './component'
export type { IMyComponent, Something } from './types'
export { WrongStringError } from './errors'
```

## 最佳实践

1. **单一职责** - 每个组件都应有一个明确的目的
2. **显式依赖** - 所有依赖项都应通过 components 参数注入
3. **不可变性** - 尽可能优先使用不可变数据结构
4. **错误处理** - 针对不同错误情况使用自定义错误类
5. **日志记录** - 使用日志记录器进行调试和监控
6. **类型安全** - 充分利用 TypeScript 的类型系统
7. **文档** - 为复杂方法和业务逻辑编写文档

## 组件生命周期

某些组件可能需要执行初始化或清理操作。WKC 提供生命周期方法：

* `[START_COMPONENT]` - 在组件启动时调用
* `[STOP_COMPONENT]` - 在组件停止时调用

请参阅 [适配器](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/chang-jian-zu-jian/adapters/) 使用生命周期方法的组件示例部分。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/chang-jian-zu-jian/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
