# 새 엔티티 유형을 만드는 방법

#### 새 엔티티를 생성해야 하는 경우는 언제인가요?

먼저, 기존 엔티티로 필요한 사용 사례를 해결할 수 있는지 확인하여 솔루션을 과도하게 설계하지 않도록 하세요. <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13> 에 정의된 기존 엔티티 유형 중 어느 것도 요구사항을 충족하지 못한다면, 새 엔티티 유형을 만들어야 할 수 있습니다.

#### 첫 번째 답변:

* 새 엔티티는 NFT 컬렉션인가요?
* 소유권은 어떻게 검증되나요?
* 그 크기는 어느 정도인가요?
* 생성될 엔티티 수는 얼마인가요?

#### 단계

먼저 <https://github.com/decentraland/adr> 에 ADR을 생성하여 스키마와 새 엔티티의 검증을 시작할 날짜를 정의하세요.

**저장소**

**@dcl/schemas**

1. enum에 새 엔티티 유형을 생성하세요: <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L19>
2. <https://github.com/decentraland/common-schemas/tree/main/src/platform> 에 폴더와 파일을 생성하여 type과 JSON 스키마를 만드세요. 예: <https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts#L20>

**@dcl/content-validator**

ADR에 정의된 검증을 구현하되, 해당 ADR의 날짜를 고려하세요. <https://github.com/decentraland/content-validator/blob/main/src/validations/timestamps.ts>. 예: <https://github.com/decentraland/content-validator/blob/main/src/validations/items/emotes.ts#L8>

1. 새 엔티티 유형의 소유권을 확인하는 코드를 <https://github.com/decentraland/content-validator/tree/main/src/validations/access-checker> 에 추가하세요.
2. 새 유형에 대한 validationForType을 구현하세요. 예: <https://github.com/decentraland/content-validator/blob/main/src/validations/profile.ts#L112>
3. 여기서는 엔티티당 최대 크기를 <https://github.com/decentraland/content-validator/blob/main/src/validations/ADR51.ts> 에서 정의해야 합니다.

**@dcl/urn-resolver**

1. 새 엔티티가 컬렉션에 해당한다면, 추가할 것은 없습니다.
2. 그렇지 않다면, 새 엔티티에 해당하는 새로운 URN을 생성하고 <https://github.com/decentraland/urn-resolver> 에 정의하세요. 또한 <https://github.com/decentraland/urn-resolver/blob/main/src/resolvers.ts#L23> 에 리졸버를 추가하세요.

**Catalyst: 콘텐츠 서버**

1. 데이터베이스 변경은 필요하지 않습니다.
2. 위에 상세히 설명된 수정된 모든 라이브러리를 업데이트하세요.

**Catalyst: Lambdas 서버**

1. 위에 상세히 설명된 수정된 모든 라이브러리를 업데이트하세요.
2. 프로필 정제(sanitization)를 업데이트해야 하는지 확인하세요 (`/lambdas/profiles` 엔드포인트)
3. erc 컬렉션 엔드포인트를 업데이트해야 하는지 확인하세요 (`/lambdas/collections` 엔드포인트)
4. 해당 엔티티를 표시하는 엔드포인트를 만드세요. 예: `/lambdas/wearables-by-owner`


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/tutorials/how-to-create-new-entity-type.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
