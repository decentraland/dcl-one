# 컴포넌트

컴포넌트는 다른 컴포넌트(또는 없음)를 받아 자신의 작업을 수행하도록 설계된 블랙박스 형태의 자가 포함 소프트웨어 조각입니다. 주요 목표는 코드 섹션 간의 결합도를 낮추고 테스트 용이성을 극대화하는 것입니다.

## 디렉터리 구조

모든 컴포넌트는 최소한 다음 네 파일을 포함하는 전용 디렉터리 안에 정의되어야 합니다:

* `component.ts` - 컴포넌트의 구현 코드를 포함합니다
* `types.ts` - 컴포넌트의 인터페이스와 노출된 타입을 포함합니다
* `errors.ts` - 내보내는 모든 오류 클래스를 포함합니다
* `index.ts` - 컴포넌트의 공개 API를 내보냅니다

### 예시 구조

```
src/
└── components/
    └── my-component/
        ├── component.ts
        ├── types.ts
        ├── errors.ts
        └── index.ts
```

## 타입 파일

컴포넌트는 공개 API를 정의하는 인터페이스를 따릅니다. 이 인터페이스는 서로 교체 가능한 컴포넌트 간에 공유되거나, 컴포넌트가 노출하는 메서드를 이해하는 데 사용될 수 있습니다.

자체 인터페이스를 가진 모든 컴포넌트는 반드시 `types.ts` 파일에 컴포넌트 디렉터리 안에서 정의해야 합니다.

컴포넌트가 노출하는 다른 타입들도 이 파일에 배치되어야 합니다.

### 예시: `types.ts`

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

### 공유 타입

공통 인터페이스와 같이 여러 컴포넌트 간에 공유되는 타입은 `types.ts` 파일에 프로젝트 소스 코드의 루트 디렉터리(`/src/types.ts`).

## 컴포넌트 파일

컴포넌트 파일에는 컴포넌트 생성자 함수가 들어 있습니다. 이 함수는 특정한 명명 및 구조 규칙을 따릅니다.

### 명명 규칙

컴포넌트 생성자 함수의 이름은 반드시 다음과 같아야 합니다: `create` + `ComponentName` + `Component`

**예시:** `createMyNewComponent`

### 함수 시그니처

생성자 함수는 반드시 다음을 수행해야 합니다:

1. 모든 의존성을 포함하는 components 객체를 첫 번째 매개변수로 받습니다
2. 선택적으로 추가 구성 매개변수를 받을 수 있습니다
3. 노출된(공개) 메서드를 포함하는 객체를 반환합니다

### 컴포넌트 구조

컴포넌트 생성자 함수의 시작 부분에서:

1. components 객체에서 의존성을 추출합니다
2. 공통 변수(예: 로거, 설정)를 초기화합니다
3. 내부 헬퍼 함수를 정의합니다
4. 공개 메서드를 정의합니다
5. 공개 API를 반환합니다

### 예시: `component.ts`

```tsx
import { IMyComponent, Something } from './types'
import { WrongStringError } from './errors'

export function createMyNewComponent(
  components: Pick<AppComponents, 'logs'>
): IMyComponent {
  const { logs } = components
  const logger = logs.getLogger('my-component')

  // 내부 메서드 - 노출되지 않음
  function computeString(fstString: string, sndString: string): string {
    if (fstString.length === 0) {
      throw new WrongStringError()
    }
    
    return fstString + ' ' + sndString
  }

  // 공개 메서드 - 반환 객체에 노출됨
  function myFunction(): boolean {
    return true
  }

  // 공개 메서드 - 반환 객체에 노출됨
  function getSomething(): Something {
    logger.info('Something을 가져오는 중')
    
    return {
      aValue: true,
      anotherValue: computeString('aString', 'anotherString')
    }
  }

  // 공개 API 반환
  return {
    myFunction,
    getSomething
  }
}
```

## 오류 파일

오류 파일에는 컴포넌트가 발생시킬 수 있는 사용자 정의 오류 클래스가 들어 있습니다. 이러한 오류는 다른 컴포넌트나 컨트롤러에서 특정 오류 조건을 올바르게 식별하고 처리하는 데 사용할 수 있습니다.

### 예시: `errors.ts`

```tsx
export class WrongStringError extends Error {
  constructor(message?: string) {
    super(message || '잘못된 문자열')
    this.name = 'WrongStringError'
  }
}

export class ComponentNotInitializedError extends Error {
  constructor() {
    super('컴포넌트가 초기화되지 않았습니다')
    this.name = 'ComponentNotInitializedError'
  }
}
```

### 오류 처리 모범 사례

* 서로 다른 오류 조건에 대해 구체적인 오류 클래스를 만드세요
* 의미 있는 오류 메시지를 포함하세요
* 다음을 `name` 디버깅을 쉽게 하기 위해 클래스 이름과 일치하도록 property를 설정하세요
* 각 공개 메서드가 던질 수 있는 오류를 문서화하세요

## 인덱스 파일

인덱스 파일은 컴포넌트의 공개 진입점 역할을 하며, 애플리케이션의 다른 부분에서 접근할 수 있어야 하는 것만 내보냅니다.

### 예시: `index.ts`

```tsx
export { createMyNewComponent } from './component'
export type { IMyComponent, Something } from './types'
export { WrongStringError } from './errors'
```

## 권장 사항

1. **단일 책임** - 각 컴포넌트는 하나의 명확한 목적을 가져야 합니다
2. **명시적 의존성** - 모든 의존성은 components 매개변수를 통해 주입되어야 합니다
3. **불변성** - 가능한 경우 불변 데이터 구조를 선호하세요
4. **오류 처리** - 서로 다른 오류 조건에 대해 사용자 정의 오류 클래스를 사용하세요
5. **로깅** - 디버깅과 모니터링에 로거를 사용하세요
6. **타입 안전성** - TypeScript의 타입 시스템을 충분히 활용하세요
7. **문서화** - 복잡한 메서드와 비즈니스 로직을 문서화하세요

## 컴포넌트 생명주기

일부 컴포넌트는 설정 또는 정리 작업이 필요할 수 있습니다. WKC는 생명주기 메서드를 제공합니다:

* `[START_COMPONENT]` - 컴포넌트가 시작될 때 호출됩니다
* `[STOP_COMPONENT]` - 컴포넌트가 중지될 때 호출됩니다

다음을 참조하세요 [어댑터](/docs/contributor/contributor-ko/contributor-guides/well-known-components/adapters/) 생명주기 메서드를 사용하는 컴포넌트 예시를 위한 섹션입니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/well-known-components/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
