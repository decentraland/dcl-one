# 控制器

控制器负责处理应用中的 HTTP 和 WebSocket 请求。它们作为外部请求的入口点，并充当传输层与业务逻辑之间的桥梁。

## 目的

控制器只能执行这些特定任务：

1. **验证传输层输入** - 验证身份验证、授权和输入格式
2. **调用逻辑组件** - 将业务逻辑委托给适当的逻辑组件
3. **转换输入** - 将请求数据转换为逻辑组件调用所需的参数
4. **处理错误** - 捕获并妥善响应来自逻辑组件的错误


> [!WARNING]
> 控制器不应包含业务逻辑。所有业务规则都必须在逻辑组件中实现。


## 位置

所有控制器都必须放在 `/src/controllers` 目录下，并以它们执行的操作来命名。

```
src/
└── controllers/
    ├── users/
    │   ├── get-user.ts
    │   ├── create-user.ts
    │   └── update-user.ts
    ├── content/
    │   ├── publish-content.ts
    │   └── delete-content.ts
    └── health.ts
```

## 控制器结构

### 基本 HTTP 处理程序

```tsx
import { HandlerContextWithPath } from '@dcl/platform-server-commons'

export async function getUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/:id'>
) {
  const { 
    components: { userLogic, logs },
    params: { id },
    verification 
  } = context

  const logger = logs.getLogger('get-user-handler')

  try {
    // 1. 验证身份验证
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: '未授权',
          message: '需要身份验证'
        }
      }
    }

    // 2. 验证输入
    if (!id || typeof id !== 'string') {
      return {
        status: 400,
        body: {
          error: '错误请求',
          message: '无效的用户 ID'
        }
      }
    }

    // 3. 调用逻辑组件
    const user = await userLogic.getUser(id)

    // 4. 返回成功响应
    return {
      status: 200,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    // 5. 处理错误
    logger.error('获取用户时出错', error)

    if (error instanceof UserNotFoundError) {
      return {
        status: 404,
        body: {
          error: '未找到',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: '内部服务器错误',
        message: '发生了意外错误'
      }
    }
  }
}
```

### POST 请求处理程序

```tsx
export async function createUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users'>
) {
  const {
    components: { userLogic, logs },
    request,
    verification
  } = context

  const logger = logs.getLogger('create-user-handler')

  try {
    // 1. 验证身份验证
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: '未授权',
          message: '需要身份验证'
        }
      }
    }

    // 2. 解析并验证请求体
    const body = await request.json()
    
    if (!body.username || !body.email) {
      return {
        status: 400,
        body: {
          error: '错误请求',
          message: '需要用户名和电子邮件'
        }
      }
    }

    // 3. 为逻辑组件转换输入
    const userData = {
      username: body.username.toLowerCase(),
      email: body.email.toLowerCase(),
      display_name: body.display_name || body.username
    }

    // 4. 调用逻辑组件
    const user = await userLogic.createUser(userData)

    // 5. 返回成功响应
    return {
      status: 201,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    logger.error('创建用户时出错', error)

    if (error instanceof UserAlreadyExistsError) {
      return {
        status: 409,
        body: {
          error: '冲突',
          message: error.message
        }
      }
    }

    if (error instanceof InvalidEmailError) {
      return {
        status: 400,
        body: {
          error: '错误请求',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: '内部服务器错误',
        message: '发生了意外错误'
      }
    }
  }
}
```

### 查询参数处理程序

```tsx
export async function searchUsersHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/search'>
) {
  const {
    components: { userLogic, logs },
    url,
    verification
  } = context

  const logger = logs.getLogger('search-users-handler')

  try {
    // 1. 验证身份验证
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: '未授权',
          message: '需要身份验证'
        }
      }
    }

    // 2. 提取并验证查询参数（小写！）
    const searchParams = new URLSearchParams(url.search)
    const query = searchParams.get('query')?.toLowerCase()
    const limit = parseInt(searchParams.get('limit') || '10', 10)
    const offset = parseInt(searchParams.get('offset') || '0', 10)

    if (!query) {
      return {
        status: 400,
        body: {
          error: '错误请求',
          message: '查询参数是必需的'
        }
      }
    }

    // 3. 调用逻辑组件
    const results = await userLogic.searchUsers({
      query,
      limit,
      offset
    })

    // 4. 返回成功响应
    return {
      status: 200,
      body: {
        ok: true,
        data: results
      }
    }
  } catch (error) {
    logger.error('搜索用户时出错', error)

    return {
      status: 500,
      body: {
        error: '内部服务器错误',
        message: '发生了意外错误'
      }
    }
  }
}
```

## 输入要求

通过控制器接收的所有输入键（JSON body、查询参数、URL 参数、请求头等） **都必须使用小写** 以防止在处理数据时出现大小写问题。

### 多词参数

多词参数必须使用 **snake\_case**.


> [!CAUTION]
> **错误命名：**
> 
> * `minPrice`
> * `Car`
> * `somethingURL`
> * `userId`



> [!TIP]
> **正确命名：**
> 
> * `min_price`
> * `car`
> * `something_url`
> * `user_id`


### 示例

```tsx
// 请求体
{
  "user_name": "john_doe",        // ✅ 正确
  "email_address": "john@example.com",  // ✅ 正确
  "display_name": "John Doe",     // ✅ 正确
  "max_items": 100                // ✅ 正确
}

// 查询参数
?search_query=test&max_results=50&sort_by=created_at  // ✅ 正确

// URL 参数
/users/:user_id/posts/:post_id   // ✅ 正确
```

## 错误处理

### 标准错误响应

控制器应返回一致的错误响应格式：

```tsx
// 400 错误请求
{
  "error": "错误请求",
  "message": "详细错误信息",
  "details": {} // 可选的附加详情
}

// 401 未授权
{
  "error": "未授权",
  "message": "需要身份验证"
}

// 403 禁止访问
{
  "error": "禁止访问",
  "message": "你没有访问此资源的权限"
}

// 404 未找到
{
  "error": "未找到",
  "message": "未找到资源"
}

// 409 冲突
{
  "error": "冲突",
  "message": "资源已存在"
}

// 500 内部服务器错误
{
  "error": "内部服务器错误",
  "message": "发生了意外错误"
}
```

### 错误映射

将领域错误映射为适当的 HTTP 状态码：

```tsx
function mapErrorToResponse(error: Error, logger: ILogger) {
  logger.error('处理程序错误', error)

  // 领域特定错误
  if (error instanceof UserNotFoundError) {
    return { status: 404, body: { error: '未找到', message: error.message } }
  }
  
  if (error instanceof UserAlreadyExistsError) {
    return { status: 409, body: { error: '冲突', message: error.message } }
  }
  
  if (error instanceof UnauthorizedError) {
    return { status: 403, body: { error: '禁止访问', message: error.message } }
  }
  
  if (error instanceof InvalidInputError) {
    return { status: 400, body: { error: '错误请求', message: error.message } }
  }

  // 通用错误
  return {
    status: 500,
    body: {
      error: '内部服务器错误',
      message: '发生了意外错误'
    }
  }
}
```

## 身份验证和授权

### 身份验证检查

```tsx
// 要求身份验证
if (!verification?.auth) {
  return {
    status: 401,
    body: {
      error: '未授权',
      message: '需要身份验证'
    }
  }
}

const userAddress = verification.auth.toLowerCase()
```

### 授权检查

```tsx
// 通过逻辑组件检查权限
const canAccess = await permissionsLogic.canUserAccessResource(
  userAddress,
  resourceId
)

if (!canAccess) {
  return {
    status: 403,
    body: {
      error: '禁止访问',
      message: '你没有访问此资源的权限'
    }
  }
}
```

## 最佳实践

### 1. 保持控制器精简

控制器应是逻辑组件的精简包装器：

```tsx
// ✅ 好的：精简控制器
export async function handler(context) {
  const { userLogic } = context.components
  const user = await userLogic.getUser(context.params.id)
  return { status: 200, body: { data: user } }
}

// ❌ 不好：控制器中的业务逻辑
export async function handler(context) {
  const { database } = context.components
  const user = await database.query('SELECT * FROM users...')
  const processed = processUser(user) // 业务逻辑！
  const validated = validateUser(processed) // 业务逻辑！
  return { status: 200, body: { data: validated } }
}
```

### 2. 尽早验证

先验证并快速失败：

```tsx
// 先验证身份验证
if (!verification?.auth) {
  return { status: 401, body: { error: '未授权' } }
}

// 然后验证输入
if (!params.id) {
  return { status: 400, body: { error: '错误请求' } }
}

// 然后继续执行业务逻辑
const result = await logic.doSomething(params.id)
```

### 3. 使用类型安全

利用 TypeScript 实现类型安全：

```tsx
interface CreateUserRequest {
  user_name: string
  email_address: string
  display_name?: string
}

const body: CreateUserRequest = await request.json()
```

### 4. 适当记录日志

记录重要事件和错误：

```tsx
logger.info('用户已创建', { userId: user.id, userAddress })
logger.warn('接近速率限制', { userAddress, requests })
logger.error('创建用户失败', { error, userAddress })
```

## 测试控制器

请参阅 [Testing Services (WKC)](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc/) 用于集成测试指导的文档。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/chang-jian-zu-jian/controllers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
