> Decentraland 제작 워크플로를 빠르게 해주는 도구, 애드온, 에셋 라이브러리, 예제 프로젝트.

# 유용한 자료

이 페이지에는 Decentraland용 경험을 만들 때 워크플로를 빠르게 해줄 도구, 애드온, 에셋 라이브러리, 예제 프로젝트가 모여 있습니다.

## Decentraland SDK 스킬

AI 코딩 어시스턴트에 Decentraland 전용 지침을 제공해 경험을 빠르게 만들고, 반복 개선하고, 배포하세요. Decentraland SDK Skills는 Claude Code, Cursor, Codex, Cline, Windsurf 같은 도구와 함께 사용할 수 있으며, 최신 SDK 변경 사항에 맞춰 계속 업데이트됩니다.

🔗 [github.com/decentraland/sdk-skills](https://github.com/decentraland/sdk-skills)

참조 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) AI 워크플로에서 SDK Skills를 사용하는 팁은

## Decentraland SDK7 테스트 씬

특정 SDK 기능의 동작 예제를 보려면 Decentraland SDK7 테스트 씬을 사용하세요. 이 저장소에는 많은 독립형 테스트 씬이 포함되어 있으며, 라이브 테스트 월드를 통해 자신의 경험에 비슷한 시스템을 적용하기 전에 먼저 시험해 볼 수 있습니다.

🔗 [github.com/decentraland/sdk7-test-scenes](https://github.com/decentraland/sdk7-test-scenes/)

## 멀티플레이어 서버 리더보드

Decentraland의 멀티플레이어 서버를 사용해 복원력 있는 리더보드를 만드는 방법을 확인해 보세요. 서버가 게임 상태를 소유하고, 각 점수를 검증하며, 플레이어별 총점을 유지하고, 모든 Decentraland 클라이언트가 렌더링할 수 있는 순위형 리더보드를 브로드캐스트합니다.

🔗 [멀티플레이어 서버 리더보드 테스트 씬](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard)

참조 [권한형 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 권한형 서버에서 빌드하는 방법을 알아보려면

## Blender용 Decentraland 도구

Decentraland 설정이 내장된 이 Blender 애드온으로 워크플로를 빠르게 하세요. 이 애드온은 파셀 그리드, 착용 아이템 제한, 텍스처 최적화, LOD 생성, GLB 내보내기를 처리합니다.

🔧 [extensions.blender.org/add-ons/decentraland-tools](https://extensions.blender.org/add-ons/decentraland-tools/)

## 3D 아트 에셋

좋은 경험에는 그에 어울리는 훌륭한 3D 아트가 필요합니다. 직접 3D 모델을 만들고 싶다면 [3D 모델링 섹션](/docs/creator/content-creator-ko/3d/3d-models/). 하지만 코딩이나 게임 디자인에 더 집중하고 싶다면, 직접 에셋을 만들 필요는 없습니다!

Decentraland 씬에서 사용할 수 있는 3D 모델 소스 몇 가지를 소개합니다:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://cgtrader.com)

생성형 AI 도구를 사용해 직접 3D 모델을 만들 수도 있습니다. 다음을 확인해 보세요:

* [Blender MCP](https://github.com/ahujasid/blender-mcp) + [Claude Desktop](https://claude.ai/download): AI 어시스턴트를 직접 연결하세요 [Blender](https://www.blender.org/), 원하는 내용을 설명하기만 하면 3D 모델을 만들고 편집할 수 있습니다. 모두 무료 도구입니다.
* [Meshy](https://www.meshy.ai/)
* [Luma AI](https://lumalabs.ai/genie)
* [Tripo3D](https://www.tripo3d.ai/app)
* [Rodin](https://hyper3d.ai/rodin)


> [!WARNING]
> **📔 참고**: 모델은 지원되는 `.gltf` 또는 `.glb` 형식이어야 하며, 삼각형 수, 텍스처, 머티리얼이 다음 기준을 준수해야 합니다 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/). 타사 사이트에서 모델을 가져오는 경우, 다운로드하는 콘텐츠의 라이선스 제한에 주의하세요.


## Blender MCP 서버

Blender MCP를 사용하면 Claude Code 같은 AI 에이전트를 Blender에 연결할 수 있습니다. 간단한 텍스트 프롬프트만으로 GLTF/GLB 모델, 머티리얼, 텍스처를 만들고 편집하는 데 도움이 됩니다.

🔗 [blender.org/lab/mcp-server](https://www.blender.org/lab/mcp-server/)


> [!WARNING]
> **📔 참고**: Blender는 민감한 파일이 없는 안전한 환경에서 MCP를 사용할 것을 권장합니다.


## 오픈 소스 3D 에셋

소품, 환경, 구조물을 포함한 고품질 3D 에셋 선별 컬렉션으로, 완전히 무료이며 오픈 소스입니다. GLB 및 glTF 표준을 기반으로 합니다.

🔗 [opensource3dassets.com](https://www.opensource3dassets.com/en)

## 제네시스 플라자 에셋

자신의 경험에서 Genesis Plaza의 에셋이나 텍스처를 재사용하세요. .blend 파일을 다운로드하거나, Stom의 Blender Asset Library를 사용해 저장소에서 드래그 앤 드롭할 수 있습니다. 나무와 덤불 같은 일치하는 절차적 조경 에셋도 다운로드할 수 있습니다.

* 📂 [.blend 파일 다운로드](https://drive.google.com/file/d/1beZHX1D8zR9iMCEmcPQU6etxpZXohZ5P/view?usp=sharing)
* 🐈‍⬛ Stom의 저장소는 다음에서 확인하세요 [github.com/stom66/dcl-genesis-plaza-assets](https://github.com/stom66/dcl-genesis-plaza-assets)
* ℹ️ [저장소 사용 방법에 대한 동영상을 시청하세요](https://files.catbox.moe/eo5hhb.mp4)
* 🌺 Decentraland의 절차적 에셋에 맞는 [이 나무, 덤불, 바위와 함께](https://drive.google.com/drive/folders/1Hxoog-ov_igbNTU3H7EwOX634EVCbhYQ?usp=drive_link)

## Decentraland 씬 최적화 도구

씬을 더욱 최적화하려면 이 유용한 도구로 GLB 파일에서 텍스처를 추출, 중복 제거, 압축해 보세요.

🔗 [github.com/decentraland/SceneOptimizer](https://github.com/decentraland/SceneOptimizer)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/useful-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
