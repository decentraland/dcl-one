> 플레이어의 위치에 반응

# 트리거 영역

플레이어가 어떤 영역 안으로 들어가거나 나갈 때 무언가가 일어나게 하려면, 트리거 영역을 사용하세요 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/).

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

장면을 편집하는 동안 보이는 주황색 큐브는 Scene Editor에서만 보이며, 장면 미리보기를 실행하면 보이지 않게 됩니다. 필요한 영역을 정확히 덮도록 주황색 큐브를 쉽게 조정하고 크기를 변경할 수 있습니다.

장면에 끌어다 놓고 스케일 기즈모로 크기를 조절하세요. 보이는 상자나 구는 편집기에서만 그려지며, 플레이어는 절대 볼 수 없습니다.

플레이어 아바타의 일부가 이 주황색 큐브와 겹치면, 할당된 이벤트가 호출됩니다. 트리거 영역은 다른 플레이어의 아바타가 아니라 로컬 머신의 플레이어에게만 반응합니다. 각 플레이어는 자신의 장면 인스턴스에서 트리거를 발생시킵니다.


> [!NOTE]
> **💡 팁**: 장면의 트리거 영역이 다른 콘텐츠 편집을 방해하기 시작하면, 언제든지 잠그거나/또는 숨길 수 있다는 점을 기억하세요 [엔티티 트리](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#the-entity-tree).
> 
> <img src="/docs/_remote/c1baca1d0d660705a098e478.webp" alt="" data-size="original">


여러 트리거 영역은 서로 겹칠 수 있으며, 서로에게 영향을 주지 않습니다.

무엇이 일어나야 하는지 설명하면 [AI 도우미](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/) 그것에 대한 짧은 스크립트를 작성합니다.

### 설정

항목의 **스크립트** 구성 요소에는 두 개의 필드가 있습니다:

| 필드         | 기능               | 옵션                                                                           | 기본값      |
| ---------- | ---------------- | ---------------------------------------------------------------------------- | -------- |
| **활성화 대상** | 영역을 작동시키는 것.     | `내 플레이어`, `모든 플레이어`, `모든 콜라이더`, `클릭 가능한`, `사용자 지정 1`, `사용자 지정 2`, `사용자 지정 3` | `내 플레이어` |
| **모양**     | 볼륨이 상자인지 공인지 여부. | `박스`, `구`                                                                    | `박스`     |

`내 플레이어` 장면을 실행하는 플레이어의 아바타에만 반응합니다. `모든 플레이어` 다른 사람의 아바타를 포함해 어떤 아바타에도 반응합니다. 나머지 옵션은 다른 [충돌 레이어](/docs/creator/content-creator-ko/sdk7/3d/colliders/), 따라서 영역이 물리 오브젝트, 클릭 가능한 오브젝트 또는 사용자 지정 레이어에 반응하도록 할 수 있습니다.

전환 **모양** 편집기의 자리표시자를 상자와 구 사이에서 바꿔 주므로, 편집할 때 보이는 것이 곧 실제로 얻는 볼륨입니다.

### 무엇이 일어나야 하는지 설명하세요

영역은 두 개의 이벤트를 발생시킵니다: **진입** 및 **퇴장**. 다음의 **반응** Script 구성 요소의 섹션에는 각각에 대한 버튼이 있습니다:

* **플레이어가 들어오면…**
* **플레이어가 나가면…**

하나를 클릭하면 Creator Hub가 AI Assistant의 메시지 상자에 문장의 시작 부분을 작성하고, 예를 들어 항목 이름을 붙입니다 `플레이어가 "트리거 영역"에 들어오면,` . 프롬프트는 엔티티 트리에서 보이는 항목의 이름을 사용합니다. 일어나길 원하는 내용으로 문장을 완성해 보내세요. 어시스턴트가 별도의 스크립트를 작성해 감지기와 함께 동일한 엔티티에 첨부합니다.

버튼 옆의 세 개 칩(**소리 재생**, **메시지 표시**, **점수 획득**)은 문장을 완성된 형태로 채워 주므로, 그대로 보낼 수 있습니다.


> [!NOTE]
> **💡 팁**: 반응 기능을 사용하려면 [AI 도우미](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/) 를 켜야 합니다. 켜져 있지 않다면 Creator Hub는 채팅을 여는 대신 어디에서 활성화할 수 있는지 알려줍니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/trigger-area.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
