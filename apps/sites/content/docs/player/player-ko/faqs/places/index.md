> 장소

# 장소

<details>

<summary>Places란 무엇인가요?</summary>

[Places](https://decentraland.org/places/) Genesis City와 Worlds의 특정 관심 지점을 보여줍니다.

</details>

<details>

<summary>내 Place의 제목, 설명 또는 이미지를 어떻게 수정하나요?</summary>

Place의 제목, 설명 및 이미지는 씬 메타데이터에서 가져옵니다. Creator 문서에서 씬 메타데이터 편집에 대해 자세히 알아보세요.

</details>

<details>

<summary>내 Place는 어떻게 업데이트되나요?</summary>

씬을 다시 업로드하면 Places 웹사이트가 모든 메타데이터를 자동으로 업데이트합니다.

</details>

<details>

<summary>Place를 크게 변경하면 어떻게 되나요?</summary>

각 Place에는 고유 ID가 할당됩니다. 특정 조건이 충족되면 Place가 "새로운" 것으로 간주되어 새 ID가 할당될 수 있습니다. 이 변경으로 인해 Place에 누적된 즐겨찾기와 좋아요가 모두 사라집니다. 이러한 조건에 대한 자세한 내용은 다음을 참조하세요 [**ADR: Place 식별자**](https://adr.decentraland.org/adr/ADR-186).

</details>

<details>

<summary>좋아요 비율은 어떻게 계산되나요?</summary>

남용을 방지하기 위해 좋아요 비율은 최소 100의 투표 권한을 가진 사용자의 좋아요와 싫어요를 바탕으로 계산됩니다. 계정이 있는 사람이라면 누구나 좋아요 또는 싫어요를 할 수 있지만, 좋아요 비율 계산에는 자격을 갖춘 투표만 사용됩니다.

Place에 제목, 설명 및 썸네일이 있는지 확인하여 좋은 평가를 받을 가능성을 높이세요.

</details>

<details>

<summary>내 Place가 관심 지점(Point of Interest)이 되려면 어떻게 하나요?</summary>

관심 지점은 Decentraland의 주목할 만한 위치입니다. 이러한 "POI"는 가상 세계 UI의 여러 영역에서 강조되며, 특히 Decentraland를 처음 접하는 사람들에게 탐색하기 좋은 장소로 홍보됩니다.

다음에서 제안을 만들 수 있습니다 [**DAO**](https://decentraland.org/dao/en/) Place를 관심 지점으로 추천하려면.

</details>

<details>

<summary>Worlds 탭에는 어떤 Worlds가 표시되나요?</summary>

다음에 정의된 바와 같이 *Worlds 1.0 - 단기 계획* [**DAO 제안**](https://decentraland.org/governance/proposal/?id=e712bb50-e822-11ed-b8f1-75dbe089d333/), Foundation의 World Content Server에 배포된 Worlds는 LAND를 보유하고 있거나 활성 LAND 임대 계약을 가진 NAME 소유자에게만 검색 가능하도록 허용합니다.

</details>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-ko/faqs/places.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
