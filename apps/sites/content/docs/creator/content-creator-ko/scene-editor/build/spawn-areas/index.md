> 스폰 영역을 사용하여 스폰 위치와 스폰 카메라 위치를 시각적으로 설정합니다.

# 스폰 영역

제작자는 **스폰 위치** 및 **스폰 카메라 타깃** 를 크리에이터 허브에서 **스폰 영역** 엔티티를 수정하여 설정할 수 있습니다.

이를 통해 제작자는 장면의 다른 오브젝트처럼 스폰 영역과 스폰될 때 아바타가 바라볼 방향을 시각적으로 설정할 수 있습니다.

스폰 영역은 플레이어가 장면에 직접 접속할 때 나타나는 위치를 정의합니다. 좌표를 입력하거나 텔레포트할 때 나타날 수 있습니다. 장면에 나무나 계단처럼 플레이어가 바로 그 위에 스폰되면 이동을 막을 수 있는 오브젝트가 있거나, 지형이 높은 곳에 있을 수도 있습니다. 플레이어가 움직일 수 없는 곳 위에 스폰되면 좋지 않은 경험이 됩니다. 그래서 임시 위치에 하나 또는 여러 개의 스폰 위치를 설정할 수 있는 옵션이 있습니다.


> [!WARNING]
> **📔 참고**: 모든 스폰 지점은 장면을 구성하는 파셀 안에 있어야 합니다. 이 파셀의 범위 밖에 플레이어를 스폰할 수는 없습니다.


<img src="/docs/_remote/04f3a7c93b45dca9a38de100.webp" alt="" width="200">

스폰 영역은 함께 작동하는 두 가지 서로 다른 엔티티로 구성됩니다:

* **스폰** 엔티티:  **플레이어** 엔티티 안에서 찾을 수 있습니다. 플레이어의 실제 스폰 지점을 정의합니다.

<img src="/docs/_remote/da6fb5ebe275854d0b596645.webp" alt="" width="200">

* **카메라 타깃** 엔티티:  **스폰 영역** 엔티티. 플레이어가 스폰될 원하는 방향으로 이동하세요.

<img src="/docs/_remote/13b486809fcb8431fd6ecb7c.webp" alt="" width="200">

## 스폰 영역 변경하기

**스폰 영역** 와 **카메라 타깃** 는 장면의 다른 오브젝트처럼 씬 에디터에서 직접 이동할 수 있으며, 수동으로 값도 수정할 수 있습니다(다른 엔티티의 트랜스폼처럼).

### 매개변수

스폰 영역을 선택하면 제작자는 컴포넌트 섹션에서 다음 값을 보고 설정할 수 있습니다:

<img src="/docs/_remote/81d5fae5f7131c97a7c0709a.webp" alt="" width="600">

* **위치**: 아바타의 스폰 위치입니다. 이는 장면 원점을 기준으로 한 장면 내부의 좌표이며, 트랜스폼 컴포넌트에서 사용하는 것과 비슷합니다.
* **스폰 카메라 타깃**: 장면에 스폰된 후 카메라(및 아바타)가 바라볼 방향입니다. 플레이어가 장면에 들어왔을 때 첫인상을 더 잘 제어하려면 이를 사용하세요.
* **랜덤화된 영역**: 스폰 영역에 미터 단위로 무작위성을 추가하며, **무작위 적용 안 함** 토글로 켜고 끌 수 있습니다. 활성화하면 씬 에디터에 영역이 표시되어 플레이어가 무작위로 스폰될 수 있는 범위를 보여줍니다. 무작위 오프셋은 X축과 Z축 모두에 적용되며, 스폰 시 모든 플레이어가 서로 겹쳐 나타나는 것을 방지합니다. 특히 혼잡한 장면에서는 보기 좋지 않습니다.

<img src="/docs/_remote/87b06caf636a947bd765acb5.webp" alt="" width="600">

## 여러 스폰 영역

<img src="/docs/_remote/80f2cf42b9eb7797e76dccd8.webp" alt="" width="600">

제작자는 여러 개의 **스폰 영역** 를 정의할 수 있습니다. 새 스폰 영역을 만들려면 다음 단계를 따르세요:

1. 스폰 영역 아무거나 선택한 다음, **spawn1** 이 기본값입니다.
2. 모든 스폰 영역은 오른쪽 패널에 표시됩니다. 다음을 클릭하여 새 영역을 추가할 수 있습니다: **+ 새 스폰 영역 추가**&#x20;

   <img src="/docs/_remote/2d3066e023e44c9ba5f38990.webp" alt="아이콘" width="200">

   .

<img src="/docs/_remote/24547b03ea234f33b90a044e.webp" alt="" width="600">

3. 원하는 스폰 영역의 **주 스폰**&#x20;

   <img src="/docs/_remote/745070068b65895eedbc404a.webp" alt="아이콘" width="100">

   &#x20;을 활성화하세요. 여러 스폰 영역이 **주 스폰**로 설정되어 있으면, 플레이어는 그중 하나에 무작위로 나타납니다.
4. 장면을 미리 봅니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/build/spawn-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
