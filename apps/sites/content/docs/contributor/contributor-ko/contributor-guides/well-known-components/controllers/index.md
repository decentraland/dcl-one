# 컨트롤러

컨트롤러는 애플리케이션에서 HTTP 및 WebSocket 요청을 처리할 책임이 있습니다. 컨트롤러는 외부 요청의 진입점 역할을 하며 전송 계층과 비즈니스 로직 사이의 다리 역할을 합니다.

## 목적

컨트롤러는 다음의 특정 작업만 수행해야 합니다:

1. **전송 계층 입력 검증** - 인증, 권한 부여 및 입력 형식 확인
2. **로직 구성요소 호출** - 비즈니스 로직을 적절한 로직 구성요소에 위임
3. **입력 변환** - 요청 데이터를 로직 구성요소 호출을 위한 매개변수로 변환
4. **오류 처리** - 로직 구성요소에서 발생한 오류를 잡아 적절히 응답


> [!WARNING]
> 컨트롤러는 비즈니스 로직을 포함해서는 안 됩니다. 모든 비즈니스 규칙은 로직 구성요소에서 구현되어야 합니다.


## 위치

모든 컨트롤러는 다음 경로 아래에 배치되어야 합니다: `/src/controllers` 디렉터리이며 수행하는 작업을 기준으로 이름을 지정해야 합니다.

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

## 컨트롤러 구조

### 기본 HTTP 핸들러

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
    // 1. 인증 검증
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: '인증이 필요합니다'
        }
      }
    }

    // 2. 입력 검증
    if (!id || typeof id !== 'string') {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: '잘못된 사용자 ID'
        }
      }
    }

    // 3. 로직 구성요소 호출
    const user = await userLogic.getUser(id)

    // 4. 성공 응답 반환
    return {
      status: 200,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    // 5. 오류 처리
    logger.error('사용자 가져오기 오류', error)

    if (error instanceof UserNotFoundError) {
      return {
        status: 404,
        body: {
          error: 'Not Found',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: '예기치 않은 오류가 발생했습니다'
      }
    }
  }
}
```

### POST 요청 핸들러

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
    // 1. 인증 검증
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: '인증이 필요합니다'
        }
      }
    }

    // 2. 요청 본문 파싱 및 검증
    const body = await request.json()
    
    if (!body.username || !body.email) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: '사용자 이름과 이메일은 필수입니다'
        }
      }
    }

    // 3. 로직 구성요소를 위한 입력 변환
    const userData = {
      username: body.username.toLowerCase(),
      email: body.email.toLowerCase(),
      display_name: body.display_name || body.username
    }

    // 4. 로직 구성요소 호출
    const user = await userLogic.createUser(userData)

    // 5. 성공 응답 반환
    return {
      status: 201,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    logger.error('사용자 생성 오류', error)

    if (error instanceof UserAlreadyExistsError) {
      return {
        status: 409,
        body: {
          error: 'Conflict',
          message: error.message
        }
      }
    }

    if (error instanceof InvalidEmailError) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: '예기치 않은 오류가 발생했습니다'
      }
    }
  }
}
```

### 쿼리 매개변수 핸들러

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
    // 1. 인증 검증
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: '인증이 필요합니다'
        }
      }
    }

    // 2. 쿼리 매개변수 추출 및 검증(소문자!)
    const searchParams = new URLSearchParams(url.search)
    const query = searchParams.get('query')?.toLowerCase()
    const limit = parseInt(searchParams.get('limit') || '10', 10)
    const offset = parseInt(searchParams.get('offset') || '0', 10)

    if (!query) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: '쿼리 매개변수가 필요합니다'
        }
      }
    }

    // 3. 로직 구성요소 호출
    const results = await userLogic.searchUsers({
      query,
      limit,
      offset
    })

    // 4. 성공 응답 반환
    return {
      status: 200,
      body: {
        ok: true,
        data: results
      }
    }
  } catch (error) {
    logger.error('사용자 검색 오류', error)

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: '예기치 않은 오류가 발생했습니다'
      }
    }
  }
}
```

## 입력 요구사항

컨트롤러를 통해 수신되는 모든 입력 키(JSON 본문, 쿼리 매개변수, URL 매개변수, 헤더 등) **는 반드시 소문자여야 합니다** 데이터 처리 시 대소문자 문제를 방지하기 위해서입니다.

### 여러 단어로 된 매개변수

여러 단어로 된 매개변수는 반드시 다음 형식으로 정의해야 합니다: **snake\_case**.


> [!CAUTION]
> **잘못된 명명:**
> 
> * `minPrice`
> * `Car`
> * `somethingURL`
> * `userId`



> [!TIP]
> **올바른 명명:**
> 
> * `min_price`
> * `car`
> * `something_url`
> * `user_id`


### 예시

```tsx
// 요청 본문
{
  "user_name": "john_doe",        // ✅ 올바름
  "email_address": "john@example.com",  // ✅ 올바름
  "display_name": "John Doe",     // ✅ 올바름
  "max_items": 100                // ✅ 올바름
}

// 쿼리 매개변수
?search_query=test&max_results=50&sort_by=created_at  // ✅ 올바름

// URL 매개변수
/users/:user_id/posts/:post_id   // ✅ 올바름
```

## 오류 처리

### 표준 오류 응답

컨트롤러는 일관된 오류 응답 형식을 반환해야 합니다:

```tsx
// 400 Bad Request
{
  "error": "Bad Request",
  "message": "상세 오류 메시지",
  "details": {} // 추가 세부 정보는 선택 사항
}

// 401 Unauthorized
{
  "error": "Unauthorized",
  "message": "인증이 필요합니다"
}

// 403 Forbidden
{
  "error": "Forbidden",
  "message": "이 리소스에 접근할 권한이 없습니다"
}

// 404 Not Found
{
  "error": "Not Found",
  "message": "리소스를 찾을 수 없습니다"
}

// 409 Conflict
{
  "error": "Conflict",
  "message": "리소스가 이미 존재합니다"
}

// 500 Internal Server Error
{
  "error": "Internal Server Error",
  "message": "예기치 않은 오류가 발생했습니다"
}
```

### 오류 매핑

도메인 오류를 적절한 HTTP 상태 코드에 매핑합니다:

```tsx
function mapErrorToResponse(error: Error, logger: ILogger) {
  logger.error('핸들러 오류', error)

  // 도메인별 오류
  if (error instanceof UserNotFoundError) {
    return { status: 404, body: { error: 'Not Found', message: error.message } }
  }
  
  if (error instanceof UserAlreadyExistsError) {
    return { status: 409, body: { error: 'Conflict', message: error.message } }
  }
  
  if (error instanceof UnauthorizedError) {
    return { status: 403, body: { error: 'Forbidden', message: error.message } }
  }
  
  if (error instanceof InvalidInputError) {
    return { status: 400, body: { error: 'Bad Request', message: error.message } }
  }

  // 일반 오류
  return {
    status: 500,
    body: {
      error: 'Internal Server Error',
      message: '예기치 않은 오류가 발생했습니다'
    }
  }
}
```

## 인증 및 권한 부여

### 인증 확인

```tsx
// 인증 필요
if (!verification?.auth) {
  return {
    status: 401,
    body: {
      error: 'Unauthorized',
      message: '인증이 필요합니다'
    }
  }
}

const userAddress = verification.auth.toLowerCase()
```

### 권한 부여 확인

```tsx
// 로직 구성요소를 통해 권한 확인
const canAccess = await permissionsLogic.canUserAccessResource(
  userAddress,
  resourceId
)

if (!canAccess) {
  return {
    status: 403,
    body: {
      error: 'Forbidden',
      message: '이 리소스에 접근할 권한이 없습니다'
    }
  }
}
```

## 권장 사항

### 1. 컨트롤러는 얇게 유지하기

컨트롤러는 로직 구성요소를 감싸는 얇은 래퍼여야 합니다:

```tsx
// ✅ 좋은 예: 얇은 컨트롤러
export async function handler(context) {
  const { userLogic } = context.components
  const user = await userLogic.getUser(context.params.id)
  return { status: 200, body: { data: user } }
}

// ❌ 나쁜 예: 컨트롤러에 비즈니스 로직 포함
export async function handler(context) {
  const { database } = context.components
  const user = await database.query('SELECT * FROM users...')
  const processed = processUser(user) // 비즈니스 로직!
  const validated = validateUser(processed) // 비즈니스 로직!
  return { status: 200, body: { data: validated } }
}
```

### 2. 일찍 검증하기

검증하고 빠르게 실패하세요:

```tsx
// 먼저 인증 검증
if (!verification?.auth) {
  return { status: 401, body: { error: 'Unauthorized' } }
}

// 그다음 입력 검증
if (!params.id) {
  return { status: 400, body: { error: 'Bad Request' } }
}

// 그다음 비즈니스 로직 진행
const result = await logic.doSomething(params.id)
```

### 3. 타입 안전성 사용

타입 안전성을 위해 TypeScript를 활용하세요:

```tsx
interface CreateUserRequest {
  user_name: string
  email_address: string
  display_name?: string
}

const body: CreateUserRequest = await request.json()
```

### 4. 적절히 로깅하기

중요한 이벤트와 오류를 기록하세요:

```tsx
logger.info('사용자 생성됨', { userId: user.id, userAddress })
logger.warn('속도 제한에 접근 중', { userAddress, requests })
logger.error('사용자 생성 실패', { error, userAddress })
```

## 컨트롤러 테스트

다음을 참조하세요 [Testing Services (WKC)](/docs/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc/) 통합 테스트 안내 문서입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/well-known-components/controllers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
