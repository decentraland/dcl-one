# 로직 컴포넌트


> [!NOTE]
> 이 섹션은 현재 개발 중입니다. 로직 컴포넌트에 대한 종합 문서는 곧 다시 확인해 주세요.


로직 컴포넌트는 다음을 포함하는 소프트웨어의 일부입니다 **비즈니스 로직** 애플리케이션의 로직 컴포넌트입니다. 이들은 도메인 또는 기능별로 의미상 구성되며, 컨트롤러와 어댑터 사이의 오케스트레이션 계층 역할을 합니다.

## 목적

로직 컴포넌트:

* 비즈니스 규칙과 도메인 로직을 구현합니다
* 여러 어댑터에 걸친 작업을 오케스트레이션합니다
* 복잡한 워크플로를 캡슐화합니다
* 전송 계층(HTTP, WebSocket) 관련 사항과는 독립적으로 유지됩니다
* I/O 의존성 없이 철저하게 단위 테스트할 수 있습니다

## 위치

로직 컴포넌트는 다음 아래에 배치해야 합니다 `src/logic` 또는 `src/components` 디렉터리, 도메인별로 구성:

```
src/
└── logic/
    ├── users/
    ├── content/
    ├── permissions/
    └── notifications/
```

## 특성

### 1. 어댑터 소비

로직 컴포넌트는 어댑터의 주요 소비자입니다. 이들은 비즈니스 규칙에 집중하면서 외부 리소스와 상호작용하기 위해 어댑터를 사용합니다.

```tsx
export function createUserLogic(
  components: Pick<AppComponents, 'database' | 'cache' | 'logs'>
): IUserLogic {
  const { database, cache, logs } = components
  const logger = logs.getLogger('user-logic')

  async function getUserProfile(userId: string): Promise<UserProfile> {
    // 캐시를 먼저 확인
    const cached = await cache.get(`user:${userId}`)
    if (cached) {
      return cached
    }

    // 데이터베이스에서 가져오기
    const user = await database.query('SELECT * FROM users WHERE id = $1', [userId])
    
    // 비즈니스 로직 적용
    const profile = transformUserToProfile(user)
    
    // 결과를 캐시
    await cache.set(`user:${userId}`, profile, { ttl: 3600 })
    
    return profile
  }

  return {
    getUserProfile
  }
}
```

### 2. 비즈니스 규칙

로직 컴포넌트는 비즈니스 규칙과 검증을 적용합니다:

```tsx
async function createUser(userData: CreateUserInput): Promise<User> {
  // 비즈니스 규칙: 사용자 이름은 고유해야 합니다
  const existing = await database.query(
    'SELECT id FROM users WHERE username = $1',
    [userData.username]
  )
  
  if (existing.rows.length > 0) {
    throw new UserAlreadyExistsError(userData.username)
  }

  // 비즈니스 규칙: 사용자 데이터 검증
  if (!isValidEmail(userData.email)) {
    throw new InvalidEmailError(userData.email)
  }

  // 사용자 생성
  const user = await database.query(
    'INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *',
    [userData.username, userData.email]
  )

  logger.info('사용자 생성됨', { userId: user.id })
  
  return user
}
```

### 3. 워크플로 오케스트레이션

로직 컴포넌트는 복잡한 다단계 작업을 조정합니다:

```tsx
async function publishContent(
  userId: string,
  content: ContentInput
): Promise<PublishedContent> {
  // 1단계: 권한 검증
  const canPublish = await permissions.canUserPublish(userId)
  if (!canPublish) {
    throw new UnauthorizedError('사용자는 콘텐츠를 게시할 수 없습니다')
  }

  // 2단계: 콘텐츠 처리
  const processed = await processContent(content)

  // 3단계: 콘텐츠 저장
  const stored = await storage.save(processed)

  // 4단계: 인덱스 업데이트
  await searchIndex.index(stored)

  // 5단계: 구독자에게 알림
  await notifications.notifySubscribers(userId, stored)

  logger.info('콘텐츠 게시됨', { contentId: stored.id, userId })

  return stored
}
```

## 권장 사항

### 1. 단일 책임

각 로직 컴포넌트는 하나의 도메인 또는 바운디드 컨텍스트에 집중해야 합니다:

```tsx
// 좋음: 사용자 도메인에 집중
createUserLogic()
createContentLogic()
createPermissionsLogic()

// 피해야 함: 책임이 혼재됨
createUserAndContentLogic()
```

### 2. 의존성 주입

항상 components 매개변수를 통해 의존성을 주입하세요:

```tsx
export function createOrderLogic(
  components: Pick<AppComponents, 'database' | 'payments' | 'inventory' | 'logs'>
): IOrderLogic {
  // 주입된 의존성 사용
}
```

### 3. 오류 처리

컨트롤러가 포착하고 처리할 수 있는 의미 있는 도메인 오류를 발생시키세요:

```tsx
// 도메인별 오류 정의
export class InsufficientInventoryError extends Error {
  constructor(public readonly productId: string, public readonly requested: number) {
    super(`제품 ${productId}의 재고가 부족합니다. 요청 수량: ${requested}`)
    this.name = 'InsufficientInventoryError'
  }
}
```

### 4. 순수 비즈니스 로직

전송 계층 관련 사항이 로직 컴포넌트에 들어오지 않도록 하세요:

```tsx
// 좋음: 순수 비즈니스 로직
async function calculateOrderTotal(items: OrderItem[]): Promise<number> {
  return items.reduce((total, item) => total + item.price * item.quantity, 0)
}

// 피해야 함: HTTP/전송 계층 관련 사항
async function calculateOrderTotal(req: Request, res: Response): Promise<void> {
  // 로직 컴포넌트에서는 이런 식으로 하지 마세요
}
```

## 로직 컴포넌트 테스트

로직 컴포넌트는 철저하게 단위 테스트해야 합니다. 자세한 내용은 [Testing Services (WKC)](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc/) 문서를 참조하세요.

```tsx
describe('사용자 생성 시', () => {
  let userLogic: IUserLogic
  let mockDatabase: jest.Mocked<IDatabase>

  beforeEach(() => {
    mockDatabase = createMockDatabase()
    userLogic = createUserLogic({ database: mockDatabase, logs: mockLogs })
  })

  describe('이미 사용자 이름이 존재할 때', () => {
    beforeEach(() => {
      mockDatabase.query.mockResolvedValueOnce({ rows: [{ id: '123' }] })
    })

    it('UserAlreadyExistsError를 던져야 합니다', async () => {
      await expect(
        userLogic.createUser({ username: 'existing', email: 'test@test.com' })
      ).rejects.toThrow(UserAlreadyExistsError)
    })
  })
})
```

## 곧 제공 예정

이 섹션은 다음 내용으로 확장될 예정입니다:

* 도메인 주도 로직 컴포넌트의 자세한 예시
* 복잡한 워크플로 처리 패턴
* 대규모 도메인 로직을 구성하는 가이드라인
* 이벤트 시스템과의 통합
* 캐싱 전략
* 트랜잭션 관리 패턴


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/well-known-components/logic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
