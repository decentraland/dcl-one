# E2E 테스트


> [!NOTE]
> 이 섹션은 현재 작업 진행 중(WIP)입니다.


이 섹션에서는 개발자들이 e2e 테스트와 관련된 코드베이스를 어떻게 테스트할지 설명합니다.

## 테스트 스택

모든 UI 테스트는 다음을 사용하여 반드시 수행해야 합니다 [Cypress](https://www.cypress.io/) 주요 테스트 프레임워크로서 [Synpress](https://github.com/Synthetixio/synpress) 지갑 지원을 위해. 테스트 코드는 반드시 다음을 사용하여 작성해야 합니다: [TypeScript](https://www.typescriptlang.org/) 그리고 다음을 사용하여 실행되어야 합니다 [ts-jest](https://github.com/kulshekhar/ts-jest) 하여 타입 검사 지원을 갖춰야 합니다.

## 디렉터리 구조

새 테스트는 반드시 다음 위치에 있어야 합니다: `tests/e2e` 디렉터리를 루트 경로에 두어야 합니다. 파일 이름은 테스트할 흐름에 맞게 지정되며, 즉 e2e 테스트 파일이 다음을 테스트한다면 `컬렉션 발행` 흐름이면, 파일 이름은 반드시 다음이어야 합니다: `collection-publication.spec.cy.ts`.

## 실행 빈도

GitHub Actions를 통해 예약 실행됩니다. 빈도는 추후 결정됩니다.

## 무엇을 테스트할 것인가

* dApp에서 가장 관련성 높은 사용자 상호작용(아이템 발행, NFT 구매 등)

## 테스트 방법

모든 테스트는 반드시 다음의 반구조화 스타일을 따라야 합니다: **Given-When-Then** (GWT).

다음 예시:

1. **Given** 사용자가 방문한다 `https://example.cypress.io`
2. **When** 다음 레이블이 붙은 링크를 클릭한다 `유형`
3. 그리고 다음에 "<fake@email.com>"을 입력한다 `[data-testid="action-email"]` 입력란
4. **그다음** URL에 다음이 포함되어야 한다 `/commands/actions`
5. 그리고 `[data-testid="action-email"]` 입력란의 값은 "<fake@email.com>"이어야 한다

다음과 같이 작성해야 합니다:

```jsx
describe('사용자가 https://example.cypress.io를 방문한다', () => {
  beforeEach(() => {
    cy.visit('https://example.cypress.io')
  })

  describe('type이라고 레이블된 링크를 클릭할 때', () => {
    beforeEach(() => {
      cy.contains('type').click()
    })

    describe('그리고 action-email 입력란에 이메일을 입력할 때', () => {
      beforeEach(() => {
        // 입력란을 가져와 입력합니다
        cy.get('.action-email').type('fake@email.com')
      })

      it('그런 다음 URL에 /commands/actions가 포함되어야 합니다', () => {
        // 새 URL이어야 하며
        // '/commands/actions'를 포함합니다
        cy.url().should('include', '/commands/actions')
      })

      it('그리고 action-email 입력란의 값에는 입력한 이메일이 포함되어야 합니다', () => {
        // 값이 업데이트되었는지 확인합니다
        cy.get('.action-email').should('have.value', 'fake@email.com')
      })
    })
  })
})
```

## 계약 테스트

스마트 계약을 테스트하려면 Solidity Standards 문서를 참조하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards/testing-e2e.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
