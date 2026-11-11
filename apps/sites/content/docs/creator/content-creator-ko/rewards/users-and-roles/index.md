> 월렛 주소에 역할과 권한을 할당하여 Decentraland Rewards 캠페인을 운영할 수 있는 사람을 관리합니다.

# 사용자 및 역할

Rewards 서비스 내의 캠페인에서 어떤 작업을 수행하려면, 필요한 권한이 있는 지갑으로 로그인되어 있어야 합니다.

캠페인을 생성하면 자동으로 소유자로 지정되고 모든 권한이 부여됩니다. 소유자는 캠페인에 다른 사용자를 추가하고 특정 권한이 있는 역할을 부여할 수 있습니다. 그러나 각 캠페인에는 소유자 한 명—즉 생성자—만 있을 수 있으며, 자금을 관리할 권한이 있는 유일한 사용자입니다.

캠페인에 사용자를 추가하려면.

1. 다음을 클릭하세요: **사용자 역할** 탭에서 캠페인에 접근 권한이 있는 사용자 목록을 볼 수 있습니다.
2. 이 목록의 끝에서 새 사용자를 추가하는 양식을 찾을 수 있습니다. 참고: 이 양식은 사용자를 추가할 수 있는 필요한 권한이 있는 경우에만 표시됩니다.
3. 사용자의 지갑 주소(또는 ENS 이름)를 입력하고 역할을 선택하여 양식을 작성하세요.
4. 다음을 클릭하세요: **사용자 추가** 추가를 완료하는 버튼.

![](/docs/_remote/f926b41052805cdf281f1c90.webp)

## 역할별 권한

각 역할에 허용되는 작업은 다음과 같습니다:

| 권한                      |  뷰어 | 개발자 | 협업자 | 소유자 |
| ----------------------- | :-: | :-: | :-: | :-: |
| 캠페인 이름, 네트워크 및 최대 가스 보기 |  ✅  |  ✅  |  ✅  |  ✅  |
| 캠페인 이름 및 최대 가스 편집       |  ❌  |  ❌  |  ✅  |  ✅  |
| 캠페인 활성화 및 비활성화          |  ❌  |  ❌  |  ✅  |  ✅  |
| 공급량 보기                  |  ✅  |  ✅  |  ✅  |  ✅  |
| 공급량 추가/제거               |  ❌  |  ✅  |  ✅  |  ✅  |
| 디스펜서 보기                 |  ✅  |  ✅  |  ✅  |  ✅  |
| 디스펜서 키 보기               |  ❌  |  ✅  |  ✅  |  ✅  |
| 디스펜서 추가/제거              |  ❌  |  ✅  |  ✅  |  ✅  |
| 캠페인 거래 보기               |  ✅  |  ✅  |  ✅  |  ✅  |
| 사용자 보기                  |  ✅  |  ✅  |  ✅  |  ✅  |
| 사용자 추가/제거               |  ❌  |  ❌  |  ✅  |  ✅  |
| 소유자 사용자 제거              |  ❌  |  ❌  |  ❌  |  ✅  |
| 자금 관리                   |  ❌  |  ❌  |  ❌  |  ✅  |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/rewards/users-and-roles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
