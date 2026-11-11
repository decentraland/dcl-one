> 버그가 검토되고 수정될 수 있도록 신고하는 방법

# 버그 신고

Decentraland SDK와 Creator Hub의 Scene Editor에서 버그나 문제가 발생하면, GitHub 저장소에 신고해 주세요 [여기](https://github.com/decentraland/sdk/issues/new/choose).

## 버그를 신고하기 전에

버그 보고서를 제출하기 전에 다음을 확인해 주세요:

* 문제가 최신 버전에서 이미 수정되었을 수 있으므로, SDK의 최신 버전을 사용하고 있는지 확인하세요.
* 사용하려는 기능에 대한 문서를 확인하여, 해당 기능이 예상한 대로 동작하도록 설계되었는지, 그리고 이것이 단순한 오해가 아닌지 확인하세요.
* 문제가 씬을 구성한 방식 때문에 발생한 것인지 확인하세요. 다음을 참조하세요 [미리보기에서 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-preview/) 및 [프로덕션에서 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-prod/) 에서 팁을 확인하세요.
* 중복을 피하세요. 다음을 확인해 [알려진 버그 목록](https://github.com/orgs/decentraland/projects/20/views/13) 을 확인해 중복 등록을 피하세요. 기존 이슈에 추가할 정보가 있다면 새 이슈를 만들지 말고 해당 이슈에 댓글을 남겨 주세요.

## 버그 신고하기

SDK 관련 버그를 신고하려면 GitHub 계정이 필요합니다. 다음 단계를 따르세요:

1. 다음 링크를 방문하세요: [새 이슈 페이지](https://github.com/decentraland/sdk/issues/new/choose)
2. 클릭 **시작하기** 옆의 **버그 보고서**. ![](/docs/_remote/b51c88e272f5593a42575857.webp)
3. 템플릿의 모든 항목을 작성해 주세요. 템플릿에는 각 섹션에 대한 안내가 포함되어 있습니다. 제공할 수 있는 세부 정보가 많을수록, 개발자가 문제를 더 빠르게 파악하고 해결할 수 있습니다.
4. 클릭 **새 이슈 제출**.

![](/docs/_remote/dc32a849c6f1a59a64d4565b.webp)

개발자는 설명이 더 필요하거나 추가 정보가 필요할 경우 티켓에 후속 질문을 할 수 있습니다. 버그 보고서의 업데이트는 이메일 알림으로 받게 됩니다.


> [!WARNING]
> **📔 참고**: SDK와 관련 없는 문제는 지원팀에 문의해 주세요. 문의하려면 이 페이지를 방문하세요 [intercom.decentraland.org](https://intercom.decentraland.org/) 또는 다음 주소로 이메일을 보내 주세요 <hello@decentraland.org>. 또한 다음에서도 저희를 찾을 수 있습니다 [Discord](https://decentraland.org/discord/). 팀은 월요일부터 일요일까지 UTC 기준 오후 12시부터 오후 9시까지 도움을 드릴 수 있으며, 가능한 한 빨리 다시 연락드리도록 최선을 다할 것입니다. 위 시간대의 Discord에서는 60분 이내에, 이메일은 24시간 이내에 답변을 기대하실 수 있습니다.
> 
> DAO 지원팀에도 다음에서 연락할 수 있습니다 [DAO Discord](https://discord.gg/bxHtcMxUs4).


**버그 보고서 추가 팁:**

* 버그마다 별도의 이슈를 만드세요. 여러 문제가 있다면 각 문제마다 새 이슈를 하나씩 작성해 주세요.
* 장황한 설명은 피하세요. 필요한 경우에만 관련 명령줄 출력, 콘솔 출력 또는 씬 코드를 포함하세요.
* 가능하면 이미지나 동영상을 첨부하세요. 이슈 본문 영역에 쉽게 드래그 앤 드롭할 수 있으며, GitHub가 업로드와 링크 연결을 처리해 줍니다.

## Decentraland Playground 사용하기

문제를 보여 주는 코드 스니펫을 제공하면 매우 유용합니다. 재현 가능성이 매우 중요합니다. 버그를 재현할 수 없다면 원인 진단과 수정이 어렵습니다.

이슈 본문에 코드 스니펫을 붙여넣을 수 있지만, 코드 스니펫을 제공하는 가장 좋은 방법은 다음을 사용하는 것입니다 [Decentraland Playground](https://playground.decentraland.org/). 다음 단계를 따르세요:

1. 왼쪽에 코드 스니펫을 작성하고, 오른쪽 패널에서 씬을 확인하세요.
2. 다음을 클릭하세요: **공유** 버튼을 눌러 링크를 이슈 보고서에 붙여넣으세요.

![](/docs/_remote/14f730a1e519a0ed272964d1.webp)

플레이그라운드를 사용하면 이슈 재현이 간단해지고, 운영 체제, SDK 버전, Node 버전 등에 관계없이 링크를 여는 누구나 동일한 결과를 볼 수 있습니다. 또한 영향을 받는 조건을 빠르게 반복적으로 확인하고 문제의 원인을 정확히 찾는 데 도움이 됩니다.

문제를 재현할 수 있을 만큼만 간결하게 예시를 작성하세요. 더 단순한 예시는 문제의 출처에 대한 모호함을 줄여 줍니다.


> [!WARNING]
> **📔 참고**: Playground에서는 씬에 에셋을 가져올 수 없습니다. 따라서 3D 모델이나 사운드 파일을 가져와야 문제를 재현할 수 있다면, 그곳에서는 표시할 수 없습니다.


## 로그 가져오기

버그를 신고할 때 Decentraland 탐색기의 로그를 확인하는 것이 종종 유용합니다.

로그를 가져오려면 채팅을 열고 다음을 입력하세요 `/logs`. 그러면 Decentraland 탐색기의 로그가 포함된 새 창이 열립니다. 다음 파일을 첨부하세요 `Player.log` 파일을 이슈 보고서에 첨부하세요.

## 문서 편집

문서 웹사이트의 내용에서 문제를 발견하면 변경 요청을 제출할 수 있습니다. 페이지 맨 아래로 스크롤한 다음 **이 페이지 편집** 버튼을 클릭하세요. GitHub가 변경 사항으로 풀 리퀘스트를 만드는 과정을 안내해 줍니다. 재단 팀이 변경 사항을 검토하고 유용하다고 판단되면 게시할 것입니다.

## 기능 요청 및 제안

문제를 신고하기보다 새로운 기능이나 기능 추가에 대한 제안이 있다면, GitHub 이슈를 만드는 대신 [Decentraland Discord 서버](https://dcl.gg/discord) 에 참여하여 [Creator Hub 채널](https://discord.com/channels/417796904760639509/1288888172318560326) 에 메시지를 올려 주세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/debugging/report-bug.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
