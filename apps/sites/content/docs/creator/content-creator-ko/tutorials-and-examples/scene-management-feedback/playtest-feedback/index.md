> 플레이 중 또는 피드백 버튼, 키오스크, 영역을 통해 플레이어 평점과 댓글을 수집하고, CSV 결과를 씬 스토리지에 저장합니다.

# 씬 피드백

<figure><img src="/docs/_remote/a0baa00d823b7604d1c33550.webp" alt="Coin Hunt participation invitation with Skip and Give feedback buttons" width="400"><figcaption><p>데모는 플레이어가 참여할지 여부를 선택하도록 초대합니다.</p></figcaption></figure>

플레이어가 무엇을 혼란스럽게, 답답하게 또는 즐겁게 느끼는지, 직접 플레이하는 모습을 볼 수 없을 때도 알아보세요. 이 재사용 가능한 모듈에는 플레이 중과 플레이어가 피드백을 주기로 선택했을 때 피드백을 수집하는 방법을 보여주는 코인 찾기 장면이 포함되어 있습니다.

* [공유 테스트 빌드 실행(Zone)](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1)
* [설치 및 사용 가이드](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md)

## 프로젝트 정보

**관리 주체:** Decentraland

[소스 보기](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)

**라이선스:** 이 카탈로그에서 식별되지 않았습니다. 재사용하기 전에 적용 가능한 조건은 상위 저장소와 유지 관리자에게 문의하세요.

## AI 에이전트로 추가하기

1. 확인하세요 [Decentraland SDK 스킬](https://github.com/decentraland/sdk-skills#updating-skills) 설치되어 있고 최신 상태인지.
2. AI 에이전트에게 이렇게 말하세요: "이 장면에서 플레이어들의 피드백을 수집하고 싶습니다."

에이전트는 Scene Feedback 스킬을 사용해 두 단계에서 도움을 줄 수 있습니다:

* **플레이테스트 전:** 질문, 평점 척도, 타이밍을 게임에 맞게 조정한 다음, 계획을 검토한 뒤 모듈을 통합하세요.
* **플레이테스트 후:** CSV 파일과 함께 이 스킬을 다시 사용해 반복되는 문제와 선호를 파악하고, 근거를 검토한 뒤 다음에 무엇을 조사할지 결정하세요. 또한 다음을 요청할 수도 있습니다. [사람이 읽을 수 있는 HTML 보고서](#example-ai-generated-feedback-report) 그룹화된 답변과 평점 요약을 포함한.

직접 통합하시겠습니까? 다음을 따르세요: [설치 가이드](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#add-it-to-your-scene).

## 모듈이 장면에 맞춰지는 방식

플레이어를 언제 초대할지, 어떤 질문을 할지, 그리고 그 질문이 게임에서 언제 나타날지를 직접 선택합니다.  [Zone의 코인 찾기 데모](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1) 는 가능한 한 가지 설정을 보여줍니다:

1. **첫 번째 코인을 수집하세요.** 초대 메시지가 플레이어에게 피드백을 줄지 묻습니다: "안녕하세요, 우리는 Coin Hunt 팀입니다!" 수락하면 다음 질문이 이어집니다: "코인을 찾는 것은 얼마나 쉽거나 어려웠나요?" 플레이어는 평점을 선택하고 댓글을 추가할 수 있습니다.
2. **첫 번째 라운드(코인 5개)를 완료하세요.** 참여에 동의한 플레이어는 두 개의 질문을 연속으로 받습니다. 다음 목표가 얼마나 명확한지, 그리고 계속 플레이하고 싶은지에 대한 질문입니다.
3. **언제든지 피드백을 제공하세요.** 모든 방문자는 다음을 사용할 수 있습니다: **피드백 남기기** 버튼, 클릭 가능한 키오스크 또는 표시된 영역을 사용해 환영 메시지와 이어지는 질문 세트를 열 수 있습니다. 게임플레이 질문을 거절한 후에도 가능합니다.

플레이어가 참여하는 방식을 선택하세요:

* **참여를 선택한 플레이어:** 화면 프롬프트로 플레이어를 초대하거나, 버튼·키오스크·표시된 영역을 통해 피드백 양식을 열 수 있게 하세요.
* **선정된 테스터:** 게임 로직을 사용해 지갑 목록 또는 다른 규칙으로 테스터를 선택한 뒤, 다음으로 등록하세요: `feedback.enroll`. 다음을 참조하세요: [선정된 테스터 예시](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md#4-selected-testers-without-an-intro).
* **또는 누구에게나 물어보세요:** 사전 초대나 테스터 목록 없이 플레이 중 방문자에게 질문을 표시합니다. 플레이어는 여전히 거절할 수 있습니다. 다음을 참조하세요: [참여 설정](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#asking-participants-during-play-dynamic-approach).

<figure><img src="/docs/_remote/c600bed5d83d04aa544fb1e0.webp" alt="Example feedback form with five rating choices and an optional comment field" width="400"><figcaption><p>라벨이 있는 평점과 선택적 댓글이 포함된 질문.</p></figcaption></figure>

## 주요 기능

* **유연한 질문.** 미리 준비된 질문과 평점 척도를 선택하고, 이를 게임에 맞게 조정하거나 직접 만드세요. 라벨이 있는 평점, 선택적 댓글 또는 텍스트만 있는 답변을 사용할 수 있습니다.
* **참여 보상.** 게임에 맞는 보상으로 참여를 유도하세요. 데모에서는 한 양식의 모든 질문에 답하면 방문 전용 Playtest Contributor 이름표가 부여되어 당신에게만 보이고, +5 코인 점수와 다음 다섯 번의 획득에 대한 2배 점수를 받습니다. 방문당 한 번만 적용됩니다. 어떤 평점이든 인정되며 댓글은 선택 사항입니다.
* **장면 Storage의 CSV 결과.** 멀티플레이어 서버는 각 응답을 질문 및 게임플레이 순간과 함께 저장합니다. 스프레드시트에서 검토하려면 파일을 다운로드하세요.

## 이를 장면에 맞게 조정하세요

다음을 사용하세요 [통합 레시피](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md) 질문을 여러분의 게임 이벤트에 연결하기 위해, 지금 참여자를 초대하고 나중의 게임플레이 후에 질문하는 방식도 포함됩니다. 플레이테스트 계획 및 플레이어 찾기는 다음을 참조하세요: [플레이테스트](/docs/creator/sdk7/design-experience/playtests/).

## AI가 생성한 피드백 보고서 예시

**보고서는 Scene Feedback 모듈이 아니라 AI 에이전트가 생성합니다.** 내보낸 CSV 파일을 에이전트에게 제공하고, Scene Feedback SDK 스킬을 사용해 이를 분석한 뒤 HTML 보고서를 생성하세요.

이 예시는 가상의 장면에서 나온 합성 응답을 사용합니다. 에이전트는 여러분의 플레이테스트 질문과 데이터에 맞게 분석과 보고서를 조정할 수 있습니다.

<figure><img src="/docs/_remote/ad88dfb78877894462538f58.webp" alt="Synthetic racing-scene report showing a main finding, response counts and a rating distribution" width="600"><figcaption><p>가상의 장면에서 나온 합성 응답을 사용한 예시 보고서.</p></figcaption></figure>

{% file src="/files/c23f046fd8fb709abba50c62d5e7e6e22a655799" %}
예시 보고서를 다운로드하고 ZIP을 압축 해제한 뒤 브라우저에서 HTML 파일을 여세요.
{% endfile %}


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/tutorials-and-examples/scene-management-feedback/playtest-feedback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
