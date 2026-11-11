# 의존성 관리

이 문서는 Decentraland 생태계 전반의 JavaScript/TypeScript 프로젝트에서 의존성을 관리하기 위한 권장 관행을 정의합니다. 이는 프런트엔드, 백엔드, SDK, 공유 라이브러리, 그리고 npm/yarn/pnpm 기반 패키지 모두에 적용됩니다.

> **요약**:
>
> * **버전 전략** → 사용 **정확/고정 버전** 에 `의존성` (보안) 및 `devDependencies` (일관된 개발 환경). **버전 범위** 는 오직 `peerDependencies` (유연성)에만 사용합니다.
> * **예외** → Decentraland 패키지(`@dcl/*`, `decentraland-*`)는 신뢰할 수 있는 내부 패키지이므로 버전 범위(`^`) 신뢰할 수 있는 내부 패키지이기 때문입니다.
> * **라이브러리/공유 패키지** → 사용 `peerDependencies` 공유 라이브러리(React, @dcl/schemas 등)에 대해 버전 범위(^)를 사용합니다. `의존성` 유틸리티처럼 중복되어도 안전한 경우에는 정확한 버전만 사용합니다.
> * **앱/최종 서비스** → 사용 `의존성` React, ethers 등 런타임 패키지에는 정확한 버전을 사용합니다.

## 1. 배경

많은 라이브러리는 전역 싱글턴, React Context, 클래스 동일성, 공유 캐시, 스키마 enum, 또는 공유 DB 풀에 의존합니다. 동일한 라이브러리의 여러 버전을 설치하면 다음과 같은 문제가 생길 수 있습니다:

* 복사본 간에 컨텍스트가 공유되지 않음
* 캐시 또는 풀 중복
* instanceof 검사 실패
* enum/symbol 불일치
* 번들 크기 증가
* 진단하기 어려운 런타임 버그

올바른 의존성 관리는 이러한 문제를 방지합니다.

### 비목표

이 표준은 **아니라는** 다음을 시도하지 않습니다:

* 단일 패키지 매니저를 강제함(npm, yarn, pnpm 모두 지원)
* 에서 단일 물리적 복사본을 보장함 `node_modules` (초점은 런타임/번들 중복 제거에 있음)

## 2. 정의

| 필드                     | 목적                                                    | 소비자 책임                                        | 적용 대상      |
| ---------------------- | ----------------------------------------------------- | --------------------------------------------- | ---------- |
| `의존성`                  | 모듈 내부에서 사용되는 패키지                                      | 없음 - 소비자가 제공할 것으로 기대하지 않음                     | 앱 및 라이브러리  |
| `peerDependencies`     | 런타임에서 단일 유효 버전으로 해석되어야 하는 패키지                         | 직접 설치해야 함(또는 선택 사항이면 peerDependenciesMeta 사용) | **라이브러리만** |
| `peerDependenciesMeta` | peerDependencies에 대한 메타데이터로, peer를 선택 사항으로 표시하는 데 사용됨 | 없음 - 패키지 매니저의 동작에만 영향                         | **라이브러리만** |
| `devDependencies`      | 개발 중에만 사용되는 도구                                        | 없음                                            | 앱 및 라이브러리  |

### peerDependenciesMeta에 대해

`peerDependenciesMeta` 는 귀하의 `peerDependencies`에 대한 메타데이터를 제공하는 필드입니다. 가장 일반적인 사용 사례는 peer를 **선택 사항**:

* **필수 peer** (기본값): 설치되지 않으면 패키지 매니저가 경고합니다
* **선택적 peer**: 누락되어도 패키지 매니저가 경고하지 않으며, 패키지는 그 부재를 우아하게 처리해야 합니다

이것은 특정 의존성이 있거나 없어도 동작할 수 있는 패키지에 유용합니다(예: React가 있든 없든 동작하는 유틸리티, 또는 여러 Web3 제공자를 지원하는 라이브러리).

### 앱 vs 라이브러리

**라이브러리 / 공유 패키지:**

* 다음 `peerDependencies` 런타임에서 단일 유효 버전으로 해석되어야 하는 패키지용(React, ethers, @dcl/schemas)
* 소비자(앱)가 이러한 의존성을 제공합니다
* 중복 설치와 싱글턴 충돌을 방지함

**앱 / 최종 서비스:**

* 종종 `의존성` 를 사용함
* 최종 소비자이므로 중복은 문제가 되지 않음
* `peerDependencies` 앱이 의존성으로 소비될 수도 있다면 여전히 유효함

## 3. 각 필드 사용 시점

> **빠른 참조**: 참조 [4장](#4-examples) 에서 일반적인 패키지와 권장 위치를 확인하세요.

### 라이브러리 전용인 경우 peerDependencies를 사용:

**공유 라이브러리/패키지에서**, `peerDependencies` 런타임에서 단일 유효 버전으로 해석되어야 하는 패키지에는

* React, Redux, wagmi, ethers, viem
* @dcl/schemas 및 유사한 크로스 생태계 라이브러리
* decentraland/connect
* 풀을 공유할 때의 DB 드라이버
* 싱글턴이나 컨텍스트에 의존하는 모든 라이브러리

✅ 올바름(라이브러리):

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "@dcl/schemas": "^20.0.0"
  }
}
```

❌ 잘못됨(공유 라이브러리에 dependencies를 사용하는 라이브러리):

```json
{
  "dependencies": {
    "react": "^18.0.0"
  }
}
```

### dependencies를 사용하는 경우:

* 중복해도 안전한 유틸리티(lodash-es, date-fns)
* **앱에서**: 앱이 최종 소비자인 경우 React, ethers 같은 런타임 패키지

> **중요**: 항상 **정확/고정 버전** 에서 `의존성` 보안을 위해 사용하세요.

✅ 올바름(라이브러리):

```json
{
  "dependencies": {
    "lodash-es": "4.17.21",
    "date-fns": "3.6.0"
  }
}
```

✅ 올바름(앱):

```json
{
  "dependencies": {
    "react": "18.3.1",
    "ethers": "6.13.0",
    "lodash-es": "4.17.21"
  }
}
```

### 선택적 peerDependencies(peerDependenciesMeta)

특정 의존성이 있거나 없어도 동작하는 재사용 가능한 패키지의 경우, `peerDependenciesMeta` 를 사용하여 peer를 선택 사항으로 표시합니다:

✅ 올바름:

```json
{
  "peerDependencies": {
    "react": "^18.0.0",
    "ethers": "^6.0.0"
  },
  "peerDependenciesMeta": {
    "ethers": {
      "optional": true
    }
  }
}
```

**의미:**

* `react` 가 **필수**: 소비자가 설치해야 하며, 그렇지 않으면 패키지 매니저가 경고합니다
* `ethers` 가 **선택 사항**: 소비자가 설치할 필요가 없으며, 패키지는 사용하기 전에 존재 여부를 확인해야 합니다

**사용 사례:**

* React 환경과 비-React 환경 모두에서 동작하는 패키지
* 여러 Web3 제공자(ethers, viem 등)를 지원하는 라이브러리
* 다른 라이브러리를 보강하지만 필수는 아닌 유틸리티

### devDependencies를 사용하는 경우:

* 도구(TypeScript, ESLint, 테스터, 번들러)

> **중요**: 항상 **정확/고정 버전** 에서 `devDependencies` 팀 전체에서 일관된 개발 환경을 보장하기 위해.

✅ 올바름:

```json
{
  "devDependencies": {
    "typescript": "5.4.5",
    "eslint": "8.57.0",
    "vitest": "1.6.0"
  }
}
```

## 4. 예시

### 반드시 peerDependencies여야 하는 패키지(공유 컨텍스트 / 싱글턴)

* `react`, `react-dom`, `react-redux`, `react-router-dom`
* `redux`, `@reduxjs/toolkit`
* `ethers`, `viem`, `wagmi`
* `@dcl/schemas`, `@dcl/ui-env`, `@dcl/crypto`
* `decentraland-dapps`, `decentraland-ui`, `decentraland-ui2`, `decentraland-connect`
* `pg`, `pg-pool`

### dependencies여야 하는 패키지(중복해도 안전)

* `lodash-es`, `date-fns`
* `uuid`, `nanoid`
* `zod`, `ajv`
* `ms`, `mitt`, `fp-future`

## 5. 관련 표준

* [테스트 표준](/docs/contributor/contributor-ko/contributor-guides/testing-standards/)
* [UI 표준](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md)
* [웹 UI 표준](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md)
* [WKC 아키텍처](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md)
* [API 문서화 지침](/docs/contributor/contributor-ko/contributor-guides/api-documentation/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/dependency-management.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
