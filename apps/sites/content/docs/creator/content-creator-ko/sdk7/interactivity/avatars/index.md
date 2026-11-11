# 아바타

- [플레이어 이동](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/move-player/): 플레이어를 순간이동시키거나, 씬 내의 새 위치로 걸어가게 합니다
- [아바타 애니메이션](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-animations/): 플레이어의 아바타가 기본 또는 사용자 지정 애니메이션을 재생하도록 하고, 이모트가 끝나는 시점을 감지합니다
- [이동 설정 및 제한](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/locomotion/): 플레이어의 속도와 점프 높이를 변경하거나, 이동 방식을 제한합니다
- [아바타 수정 영역](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/modifier-areas/): 씬의 특정 지역 안에 있는 플레이어에 대해 아바타를 숨기거나, 이름표를 숨기거나, 패스포트를 비활성화합니다
- [아바타 이름표](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/avatar-nametags/): 아바타 이름표 위에 사용자 지정 라벨을 표시하여 순위, 역할 또는 팀을 보여줍니다.
- [NPC 아바타](/docs/creator/content-creator-ko/sdk7/interactivity/avatars/npc-avatars/): NPC 아바타 표시 및 제어


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/interactivity/avatars.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
