# 逻辑组件


> [!NOTE]
> 本节目前正在开发中。请稍后查看关于逻辑组件的完整文档。


逻辑组件是包含以下内容的软件部分： **业务逻辑** 您的应用程序。它们按领域或功能进行语义组织，并作为控制器与适配器之间的编排层。

## 目的

逻辑组件：

* 实现业务规则和领域逻辑
* 协调多个适配器之间的操作
* 封装复杂工作流
* 保持与传输层（HTTP、WebSocket）相关问题无关
* 无需 I/O 依赖即可进行充分的单元测试

## 位置

逻辑组件应放置在 `src/logic` 或 `src/components` 目录下，并按领域组织：

```
src/
└── logic/
    ├── users/
    ├── content/
    ├── permissions/
    └── notifications/
```

## 特性

### 1. 适配器消费

逻辑组件是适配器的主要使用者。它们使用适配器与外部资源交互，同时专注于业务规则。

```tsx
export function createUserLogic(
  components: Pick<AppComponents, 'database' | 'cache' | 'logs'>
): IUserLogic {
  const { database, cache, logs } = components
  const logger = logs.getLogger('user-logic')

  async function getUserProfile(userId: string): Promise<UserProfile> {
    // 先检查缓存
    const cached = await cache.get(`user:${userId}`)
    if (cached) {
      return cached
    }

    // 从数据库获取
    const user = await database.query('SELECT * FROM users WHERE id = $1', [userId])
    
    // 应用业务逻辑
    const profile = transformUserToProfile(user)
    
    // 缓存结果
    await cache.set(`user:${userId}`, profile, { ttl: 3600 })
    
    return profile
  }

  return {
    getUserProfile
  }
}
```

### 2. 业务规则

逻辑组件强制执行业务规则和验证：

```tsx
async function createUser(userData: CreateUserInput): Promise<User> {
  // 业务规则：用户名必须唯一
  const existing = await database.query(
    'SELECT id FROM users WHERE username = $1',
    [userData.username]
  )
  
  if (existing.rows.length > 0) {
    throw new UserAlreadyExistsError(userData.username)
  }

  // 业务规则：验证用户数据
  if (!isValidEmail(userData.email)) {
    throw new InvalidEmailError(userData.email)
  }

  // 创建用户
  const user = await database.query(
    'INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *',
    [userData.username, userData.email]
  )

  logger.info('用户已创建', { userId: user.id })
  
  return user
}
```

### 3. 工作流编排

逻辑组件协调复杂的多步骤操作：

```tsx
async function publishContent(
  userId: string,
  content: ContentInput
): Promise<PublishedContent> {
  // 第 1 步：验证权限
  const canPublish = await permissions.canUserPublish(userId)
  if (!canPublish) {
    throw new UnauthorizedError('用户无法发布内容')
  }

  // 第 2 步：处理内容
  const processed = await processContent(content)

  // 第 3 步：存储内容
  const stored = await storage.save(processed)

  // 第 4 步：更新索引
  await searchIndex.index(stored)

  // 第 5 步：通知订阅者
  await notifications.notifySubscribers(userId, stored)

  logger.info('内容已发布', { contentId: stored.id, userId })

  return stored
}
```

## 最佳实践

### 1. 单一职责

每个逻辑组件应专注于一个领域或限界上下文：

```tsx
// 好：专注于用户领域
createUserLogic()
createContentLogic()
createPermissionsLogic()

// 避免：职责混杂
createUserAndContentLogic()
```

### 2. 依赖注入

始终通过 components 参数注入依赖：

```tsx
export function createOrderLogic(
  components: Pick<AppComponents, 'database' | 'payments' | 'inventory' | 'logs'>
): IOrderLogic {
  // 使用注入的依赖
}
```

### 3. 错误处理

抛出有意义的领域错误，以便控制器能够捕获并处理：

```tsx
// 定义领域特定错误
export class InsufficientInventoryError extends Error {
  constructor(public readonly productId: string, public readonly requested: number) {
    super(`产品 ${productId} 库存不足。请求数量：${requested}`)
    this.name = 'InsufficientInventoryError'
  }
}
```

### 4. 纯业务逻辑

让逻辑组件不受传输层相关问题影响：

```tsx
// 好：纯业务逻辑
async function calculateOrderTotal(items: OrderItem[]): Promise<number> {
  return items.reduce((total, item) => total + item.price * item.quantity, 0)
}

// 避免：HTTP/传输层相关问题
async function calculateOrderTotal(req: Request, res: Response): Promise<void> {
  // 不要在逻辑组件中这样做
}
```

## 测试逻辑组件

逻辑组件应进行充分的单元测试。详见 [Testing Services (WKC)](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc/) 文档了解详情。

```tsx
describe('when creating a user', () => {
  let userLogic: IUserLogic
  let mockDatabase: jest.Mocked<IDatabase>

  beforeEach(() => {
    mockDatabase = createMockDatabase()
    userLogic = createUserLogic({ database: mockDatabase, logs: mockLogs })
  })

  describe('and the username already exists', () => {
    beforeEach(() => {
      mockDatabase.query.mockResolvedValueOnce({ rows: [{ id: '123' }] })
    })

    it('should throw UserAlreadyExistsError', async () => {
      await expect(
        userLogic.createUser({ username: 'existing', email: 'test@test.com' })
      ).rejects.toThrow(UserAlreadyExistsError)
    })
  })
})
```

## 即将推出

本节将扩展为：

* 领域驱动逻辑组件的详细示例
* 处理复杂工作流的模式
* 组织大型领域逻辑的指南
* 与事件系统集成
* 缓存策略
* 事务管理模式


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/chang-jian-zu-jian/logic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
