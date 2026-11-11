# 서명된 로그인

Signed Login 전송은 아키텍처에서 Archipelago를 대체하거나 감싸면서, 클라이언트를 섬에 할당하는 방식을 사용자 정의하고자 하는 서버에서 사용할 수 있습니다.

실시간 백엔드에 직접 연결하는 대신, Signed Login은 요청을 [서명된 fetch](/docs/contributor/contributor-ko/authentication/signed-fetch/) 연결 문자열을 얻기 위해 사용합니다. 이 중간 단계는 서버가 원하는 어떤 할당 전략이든 사용할 수 있게 해줍니다.

Signed Login URI는 다음과 같은 형식을 가집니다:

```
signed-login:https://comms.example.com/auth&param=value
```

다음 뒤의 부분은 `signed-login:` 접두사 뒤에는 유효한 URL이면 무엇이든 올 수 있습니다.

### 사용법

Signed Login 요청은 다음을 사용하여 구성됩니다 [서명된 fetch](/docs/contributor/contributor-ko/authentication/signed-fetch/) 메커니즘.

응답은 성공 시 상태 `200` 를 가지며, JSON 본문에는 *적어도* 다음 필드가 포함됩니다:

| 필드             | 유형       | 값                                       |
| -------------- | -------- | --------------------------------------- |
| `fixedAdapter` | `string` | 할당된 전송 URI(예: `livekit:` 또는 `ws-room:`) |

예를 들면:

```
{ 
  fixedAdapter: "ws-room:wss://comms.example.com/rooms/17"
}
```

실패하는 경우, 응답은 적절한 상태 코드를 가질 수 있으며 JSON 본문에는 *적어도* 다음 필드가 포함됩니다:

| 필드        | 유형       | 값                           |
| --------- | -------- | --------------------------- |
| `message` | `string` | (선택 사항) 실패에 대한 설명 또는 오류 코드. |

JSON 본문에 추가 필드가 있는 응답도 유효하며, 오류의 내용은 `message` 미지정 상태로 남습니다. 구현체는 이 유연성을 활용해 `signed-login:` 자신의 애플리케이션에 적절하다고 보는 방식으로 통합할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/communications/transport-types/signed-login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
