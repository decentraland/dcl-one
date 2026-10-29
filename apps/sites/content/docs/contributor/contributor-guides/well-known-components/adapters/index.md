# Adapters

Adapters are components responsible for handling **I/O processes** and external integrations. They serve as the bridge between your business logic and external systems such as databases, APIs, file systems, or cache stores.

## Purpose

Adapters abstract away the implementation details of external dependencies, allowing:

* **Interchangeability** - Easily swap implementations (e.g., PostgreSQL → MongoDB)
* **Testability** - Mock external dependencies in tests
* **Isolation** - Keep business logic independent of infrastructure concerns

## Location

All adapter components MUST be placed under the `src/adapters` directory in the project.

```
src/
└── adapters/
    ├── database/
    ├── storage/
    ├── cache/
    └── external-api/
```

## Shared Interfaces

Adapters SHOULD be built with interchangeability in mind. Define shared interfaces in `src/types.ts` so that multiple adapters can implement the same contract.

### Example: Storage Interface

```tsx
// src/types.ts
export interface IStorage {
  set: (key: string, value: any) => Promise<void>
  get: (key: string) => Promise<any>
  delete: (key: string) => Promise<void>
}
```

## Memory Storage Adapter

Here's a simple in-memory storage adapter implementation, useful for testing or development.

### Directory Structure

```
src/adapters/memory-storage/
├── component.ts
├── types.ts
├── errors.ts
└── index.ts
```

### Implementation: `component.ts`

```tsx
import { IStorage } from '../../types'

export function createMemoryStorageAdapter(): IStorage {
  // Internal state - scoped to this component instance
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

## Redis Storage Adapter

Here's a production-ready Redis adapter that demonstrates lifecycle management.

### Implementation: `component.ts`

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

  // Lifecycle method: called when the component starts
  async function start(): Promise<void> {
    try {
      await client.connect()
      logger.info('Redis client connected successfully')
    } catch (error) {
      logger.error('Failed to connect to Redis', error)
      throw new RedisConnectionError('Could not connect to Redis')
    }
  }

  // Lifecycle method: called when the component stops
  async function stop(): Promise<void> {
    try {
      await client.quit()
      logger.info('Redis client disconnected')
    } catch (error) {
      logger.error('Error disconnecting Redis client', error)
    }
  }

  async function set(key: string, value: any): Promise<void> {
    const serialized = JSON.stringify(value)
    await client.set(key, serialized)
    logger.debug(`Set key: ${key}`)
  }

  async function get(key: string): Promise<any> {
    const value = await client.get(key)
    logger.debug(`Get key: ${key}`)
    return value ? JSON.parse(value) : null
  }

  async function delete(key: string): Promise<void> {
    await client.del(key)
    logger.debug(`Deleted key: ${key}`)
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

### Error Handling: `errors.ts`

```tsx
export class RedisConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'RedisConnectionError'
  }
}
```

## Database Adapter Example

Here's an example of a PostgreSQL database adapter.

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
    // Test the connection
    const client = await pool.connect()
    logger.info('Database connection pool initialized')
    client.release()
  }

  async function stop(): Promise<void> {
    await pool.end()
    logger.info('Database connection pool closed')
  }

  async function query<T = any>(
    sql: string,
    params?: any[]
  ): Promise<QueryResult<T>> {
    logger.debug('Executing query', { sql, params })
    return pool.query<T>(sql, params)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    query
  }
}
```

## Lifecycle Methods

WKC provides special lifecycle methods that are automatically called:

* `[Lifecycle.ComponentStarted]` or `[START_COMPONENT]` - Called when the service starts
* `[Lifecycle.ComponentStopped]` or `[STOP_COMPONENT]` - Called when the service shuts down

### When to Use Lifecycle Methods

Use lifecycle methods when your adapter needs to:

* Establish connections (database, cache, message queue)
* Initialize pools or clients
* Perform health checks
* Clean up resources on shutdown
* Close connections gracefully

## Best Practices

### 1. Error Handling

Always handle connection errors and throw meaningful custom errors:

```tsx
try {
  await client.connect()
} catch (error) {
  logger.error('Connection failed', error)
  throw new ConnectionError('Failed to connect to external service')
}
```

### 2. Configuration

Use the config component to manage adapter settings:

```tsx
const {
  host: config.requireString('DB_HOST'),
  port: config.getNumber('DB_PORT', 5432),
  timeout: config.getNumber('DB_TIMEOUT', 30000)
}
```

### 3. Logging

Log important operations for debugging and monitoring:

```tsx
logger.info('Operation completed', { recordId, duration })
logger.warn('Slow query detected', { query, duration })
logger.error('Operation failed', { error, context })
```

### 4. Type Safety

Always type your return values properly:

```tsx
async function getUser(id: string): Promise<User | null> {
  const result = await query<User>('SELECT * FROM users WHERE id = $1', [id])
  return result.rows[0] || null
}
```

### 5. Resource Management

Always clean up resources in the stop lifecycle method:

```tsx
async function stop(): Promise<void> {
  await pool.end()
  await client.disconnect()
  logger.info('Resources cleaned up')
}
```

## Testing Adapters

See the [Testing Services (WKC)](/docs/contributor/contributor-guides/testing-standards/testing-services-wkc/) documentation for detailed guidance on testing adapters.

### Quick Example

```tsx
describe('when creating a memory storage adapter', () => {
  let storage: IStorage

  beforeEach(() => {
    storage = createMemoryStorageAdapter()
  })

  describe('when setting a value', () => {
    it('should store and retrieve the value', async () => {
      await storage.set('key', 'value')
      const result = await storage.get('key')
      expect(result).toBe('value')
    })
  })
})
```
