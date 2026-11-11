# 어댑터

어댑터는 다음을 처리하는 책임을 지는 구성 요소입니다 **I/O 프로세스** 및 외부 통합을 처리합니다. 이들은 비즈니스 로직과 데이터베이스, API, 파일 시스템, 캐시 저장소와 같은 외부 시스템 사이의 다리 역할을 합니다.

## 목적

어댑터는 외부 의존성의 구현 세부 사항을 추상화하여 다음을 가능하게 합니다:

* **상호 교체 가능성** - 구현을 쉽게 교체할 수 있음(예: PostgreSQL → MongoDB)
* **테스트 가능성** - 테스트에서 외부 의존성을 모의(Mock)할 수 있음
* **격리** - 비즈니스 로직을 인프라 관련 사항과 독립적으로 유지

## 위치

모든 어댑터 구성 요소는 반드시 다음 아래에 배치되어야 합니다 `src/adapters` 프로젝트의 디렉터리 안에.

```
src/
└── adapters/
    ├── database/
    ├── storage/
    ├── cache/
    └── external-api/
```

## 공유 인터페이스

어댑터는 상호 교체 가능성을 염두에 두고 설계해야 합니다. 다음에 공유 인터페이스를 정의하세요 `src/types.ts` 그래서 여러 어댑터가 동일한 계약을 구현할 수 있습니다.

### 예시: 저장소 인터페이스

```tsx
// src/types.ts
export interface IStorage {
  set: (key: string, value: any) => Promise<void>
  get: (key: string) => Promise<any>
  delete: (key: string) => Promise<void>
}
```

## 메모리 저장소 어댑터

다음은 테스트나 개발에 유용한 간단한 메모리 내 저장소 어댑터 구현입니다.

### 디렉터리 구조

```
src/adapters/memory-storage/
├── component.ts
├── types.ts
├── errors.ts
└── index.ts
```

### 구현: `component.ts`

```tsx
import { IStorage } from '../../types'

export function createMemoryStorageAdapter(): IStorage {
  // 내부 상태 - 이 컴포넌트 인스턴스 범위에 한정됨
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

## Redis 저장소 어댑터

다음은 라이프사이클 관리를 보여주는 프로덕션용 Redis 어댑터입니다.

### 구현: `component.ts`

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

  // 라이프사이클 메서드: 컴포넌트가 시작될 때 호출됨
  async function start(): Promise<void> {
    try {
      await client.connect()
      logger.info('Redis 클라이언트가 성공적으로 연결되었습니다')
    } catch (error) {
      logger.error('Redis에 연결하지 못함', error)
      throw new RedisConnectionError('Redis에 연결할 수 없습니다')
    }
  }

  // 라이프사이클 메서드: 컴포넌트가 중지될 때 호출됨
  async function stop(): Promise<void> {
    try {
      await client.quit()
      logger.info('Redis 클라이언트 연결이 종료되었습니다')
    } catch (error) {
      logger.error('Redis 클라이언트 연결 종료 중 오류 발생', error)
    }
  }

  async function set(key: string, value: any): Promise<void> {
    const serialized = JSON.stringify(value)
    await client.set(key, serialized)
    logger.debug(`키 설정: ${key}`)
  }

  async function get(key: string): Promise<any> {
    const value = await client.get(key)
    logger.debug(`키 조회: ${key}`)
    return value ? JSON.parse(value) : null
  }

  async function delete(key: string): Promise<void> {
    await client.del(key)
    logger.debug(`키 삭제됨: ${key}`)
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

### 오류 처리: `errors.ts`

```tsx
export class RedisConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'RedisConnectionError'
  }
}
```

## 데이터베이스 어댑터 예시

다음은 PostgreSQL 데이터베이스 어댑터 예시입니다.

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
    // 연결 테스트
    const client = await pool.connect()
    logger.info('데이터베이스 연결 풀 초기화 완료')
    client.release()
  }

  async function stop(): Promise<void> {
    await pool.end()
    logger.info('데이터베이스 연결 풀이 닫혔습니다')
  }

  async function query<T = any>(
    sql: string,
    params?: any[]
  ): Promise<QueryResult<T>> {
    logger.debug('쿼리 실행 중', { sql, params })
    return pool.query<T>(sql, params)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    query
  }
}
```

## 라이프사이클 메서드

WKC는 자동으로 호출되는 특별한 라이프사이클 메서드를 제공합니다:

* `[Lifecycle.ComponentStarted]` 또는 `[START_COMPONENT]` - 서비스 시작 시 호출됨
* `[Lifecycle.ComponentStopped]` 또는 `[STOP_COMPONENT]` - 서비스 종료 시 호출됨

### 라이프사이클 메서드를 사용할 때

어댑터가 다음을 수행해야 할 때 라이프사이클 메서드를 사용하세요:

* 연결 설정(데이터베이스, 캐시, 메시지 큐)
* 풀 또는 클라이언트 초기화
* 상태 점검 수행
* 종료 시 리소스 정리
* 연결을 우아하게 종료

## 권장 사항

### 1. 오류 처리

항상 연결 오류를 처리하고 의미 있는 사용자 정의 오류를 발생시키세요:

```tsx
try {
  await client.connect()
} catch (error) {
  logger.error('연결 실패', error)
  throw new ConnectionError('외부 서비스에 연결하지 못했습니다')
}
```

### 2. 설정

어댑터 설정을 관리할 때 config 컴포넌트를 사용하세요:

```tsx
const {
  host: config.requireString('DB_HOST'),
  port: config.getNumber('DB_PORT', 5432),
  timeout: config.getNumber('DB_TIMEOUT', 30000)
}
```

### 3. 로깅

디버깅과 모니터링을 위해 중요한 작업을 기록하세요:

```tsx
logger.info('작업 완료', { recordId, duration })
logger.warn('느린 쿼리 감지됨', { query, duration })
logger.error('작업 실패', { error, context })
```

### 4. 타입 안정성

반환 값은 항상 올바르게 타입 지정하세요:

```tsx
async function getUser(id: string): Promise<User | null> {
  const result = await query<User>('SELECT * FROM users WHERE id = $1', [id])
  return result.rows[0] || null
}
```

### 5. 리소스 관리

stop 라이프사이클 메서드에서 항상 리소스를 정리하세요:

```tsx
async function stop(): Promise<void> {
  await pool.end()
  await client.disconnect()
  logger.info('리소스 정리 완료')
}
```

## 어댑터 테스트

다음을 참조하세요 [Testing Services (WKC)](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc/) 어댑터 테스트에 대한 자세한 안내는 문서를 참조하세요.

### 간단한 예시

```tsx
describe('메모리 저장소 어댑터를 생성할 때', () => {
  let storage: IStorage

  beforeEach(() => {
    storage = createMemoryStorageAdapter()
  })

  describe('값을 설정할 때', () => {
    it('값을 저장하고 검색해야 함', async () => {
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
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/well-known-components/adapters.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
