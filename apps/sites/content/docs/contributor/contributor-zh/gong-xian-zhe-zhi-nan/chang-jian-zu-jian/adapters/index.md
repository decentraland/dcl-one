# 适配器

适配器是负责处理 **I/O 过程** 以及外部集成。它们充当你的业务逻辑与外部系统之间的桥梁，例如数据库、API、文件系统或缓存存储。

## 目的

适配器抽象掉外部依赖的实现细节，从而允许：

* **可互换性** - 轻松切换实现（例如，PostgreSQL → MongoDB）
* **可测试性** - 在测试中模拟外部依赖
* **隔离性** - 让业务逻辑与基础设施关注点保持独立

## 位置

所有适配器组件必须放置在 `src/adapters` 目录下的项目中。

```
src/
└── adapters/
    ├── database/
    ├── storage/
    ├── cache/
    └── external-api/
```

## 共享接口

适配器应当以可互换性为设计目标。在以下位置定义共享接口： `src/types.ts` 这样多个适配器就可以实现同一契约。

### 示例：存储接口

```tsx
// src/types.ts
export interface IStorage {
  set: (key: string, value: any) => Promise<void>
  get: (key: string) => Promise<any>
  delete: (key: string) => Promise<void>
}
```

## 内存存储适配器

下面是一个简单的内存存储适配器实现，适用于测试或开发。

### 目录结构

```
src/adapters/memory-storage/
├── component.ts
├── types.ts
├── errors.ts
└── index.ts
```

### 实现： `component.ts`

```tsx
import { IStorage } from '../../types'

export function createMemoryStorageAdapter(): IStorage {
  // 内部状态——限定于此组件实例
  const memory: Record<string, any> = {}

  async function set(key: string, value: any): Promise<void> {
    memory[key] = value
  }

  async function get(key: string): Promise<any> {
    return memory[key]
  }

  async function delete(key: string): Promise<void> {
    delete memory[key]
  }

  return {
    set,
    get,
    delete
  }
}
```

## Redis 存储适配器

下面是一个可用于生产环境的 Redis 适配器，演示了生命周期管理。

### 实现： `component.ts`

```tsx
import { createClient, RedisClientType } from 'redis'
import { IStorage } from '../../types'
import { RedisConnectionError } from './errors'

export function createRedisStorageAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IStorage {
  const { config, logs } = components
  const logger = logs.getLogger('redis-storage')
  
  const hostUrl = config.requireString('REDIS_URL')
  const client: RedisClientType = createClient({ url: hostUrl })

  // 生命周期方法：组件启动时调用
  async function start(): Promise<void> {
    try {
      await client.connect()
      logger.info('Redis 客户端连接成功')
    } catch (error) {
      logger.error('连接 Redis 失败', error)
      throw new RedisConnectionError('无法连接到 Redis')
    }
  }

  // 生命周期方法：组件停止时调用
  async function stop(): Promise<void> {
    try {
      await client.quit()
      logger.info('Redis 客户端已断开连接')
    } catch (error) {
      logger.error('断开 Redis 客户端连接时出错', error)
    }
  }

  async function set(key: string, value: any): Promise<void> {
    const serialized = JSON.stringify(value)
    await client.set(key, serialized)
    logger.debug(`已设置键：${key}`)
  }

  async function get(key: string): Promise<any> {
    const value = await client.get(key)
    logger.debug(`已获取键：${key}`)
    return value ? JSON.parse(value) : null
  }

  async function delete(key: string): Promise<void> {
    await client.del(key)
    logger.debug(`已删除键：${key}`)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    set,
    get,
    delete
  }
}
```

### 错误处理： `errors.ts`

```tsx
export class RedisConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'RedisConnectionError'
  }
}
```

## 数据库适配器示例

下面是一个 PostgreSQL 数据库适配器示例。

```tsx
import { Pool, QueryResult } from 'pg'
import { IDatabase } from '../../types'

export function createPostgresAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IDatabase {
  const { config, logs } = components
  const logger = logs.getLogger('postgres-adapter')

  const pool = new Pool({
    connectionString: config.requireString('DATABASE_URL'),
    max: config.getNumber('DATABASE_POOL_SIZE', 20)
  })

  async function start(): Promise<void> {
    // 测试连接
    const client = await pool.connect()
    logger.info('数据库连接池已初始化')
    client.release()
  }

  async function stop(): Promise<void> {
    await pool.end()
    logger.info('数据库连接池已关闭')
  }

  async function query<T = any>(
    sql: string,
    params?: any[]
  ): Promise<QueryResult<T>> {
    logger.debug('正在执行查询', { sql, params })
    return pool.query<T>(sql, params)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    查询
  }
}
```

## 生命周期方法

WKC 提供了一些会自动调用的特殊生命周期方法：

* `[Lifecycle.ComponentStarted]` 或 `[START_COMPONENT]` - 在服务启动时调用
* `[Lifecycle.ComponentStopped]` 或 `[STOP_COMPONENT]` - 在服务关闭时调用

### 何时使用生命周期方法

当你的适配器需要以下操作时，请使用生命周期方法：

* 建立连接（数据库、缓存、消息队列）
* 初始化连接池或客户端
* 执行健康检查
* 在关闭时清理资源
* 优雅地关闭连接

## 最佳实践

### 1. 错误处理

始终处理连接错误并抛出有意义的自定义错误：

```tsx
try {
  await client.connect()
} catch (error) {
  logger.error('连接失败', error)
  throw new ConnectionError('连接外部服务失败')
}
```

### 2. 配置

使用 config 组件来管理适配器设置：

```tsx
const {
  host: config.requireString('DB_HOST'),
  port: config.getNumber('DB_PORT', 5432),
  timeout: config.getNumber('DB_TIMEOUT', 30000)
}
```

### 3. 日志

记录重要操作以便调试和监控：

```tsx
logger.info('操作完成', { recordId, duration })
logger.warn('检测到慢查询', { query, duration })
logger.error('操作失败', { error, context })
```

### 4. 类型安全

始终正确地为返回值添加类型：

```tsx
async function getUser(id: string): Promise<User | null> {
  const result = await query<User>('SELECT * FROM users WHERE id = $1', [id])
  return result.rows[0] || null
}
```

### 5. 资源管理

始终在 stop 生命周期方法中清理资源：

```tsx
async function stop(): Promise<void> {
  await pool.end()
  await client.disconnect()
  logger.info('资源已清理')
}
```

## 测试适配器

请参阅 [Testing Services (WKC)](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc/) 文档，了解有关测试适配器的详细指导。

### 快速示例

```tsx
describe('创建内存存储适配器时', () => {
  let storage: IStorage

  beforeEach(() => {
    storage = createMemoryStorageAdapter()
  })

  describe('设置一个值时', () => {
    it('应该存储并检索该值', async () => {
      await storage.set('key', 'value')
      const result = await storage.get('key')
      expect(result).toBe('value')
    })
  })
})
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/chang-jian-zu-jian/adapters.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
