> 씬 미리보기에서 볼 수 있는 내용

# 씬 미리보기

다음이 있으면 [새 씬을 만들었거나](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/#create-your-first-scene) 또는 다음을 다운로드했거나 [씬 예제](/docs/creator/content-creator-ko/tutorials-and-examples/examples/) 로컬에서 미리 볼 수 있습니다.

## Creator Hub의 씬 편집기 사용

다음을 확인하세요: [Creator Hub를 설치했는지](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/).

1. 씬 프로젝트를 여세요.
2. 다음을 클릭하세요: **실행** 버튼을 오른쪽 상단 모서리에서 누르세요. 그러면 Decentraland Desktop Explorer가 새 창으로 열리고, 해당 씬만 실행됩니다. 그곳에서 씬 안을 이동하고 상호작용 가능한 항목과 상호작용할 수 있습니다.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

### 미리 보기 위치 선택

다음 옆의 화살표를 클릭하여 **실행** 버튼을 눌러 **실행 옵션** 메뉴를 여세요. 메인 패널에서 씬을 실행할 대상을 선택합니다:

* **데스크톱 클라이언트**: 미리 보기를 Decentraland Desktop Explorer에서 엽니다. 기본값입니다.
* **웹(Bevy)**: Bevy Web 클라이언트를 사용해 브라우저에서 미리 보기를 엽니다. 다음과 동일합니다: `--web` CLI 플래그.
* **모바일용 QR 코드 표시**: 씬 미리 보기를 다음에서 열 수 있는 QR 코드를 표시합니다: [Decentraland 모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/). 컴퓨터와 동일한 Wi‑Fi 네트워크에 연결된 휴대폰으로 코드를 스캔하세요. 팝업 주변의 어두운 영역을 클릭하면 닫을 수 있습니다. 자세한 내용은 [모바일에서 미리 보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) 를 참고하세요.

### 데스크톱 클라이언트 옵션

선택하세요 **데스크톱 클라이언트** 다음의 **실행 옵션** 메뉴에서 선택한 다음, 마우스를 올려 데스크톱 클라이언트에만 적용되는 설정이 있는 하위 메뉴를 여세요:

* **랜드스케이프 지형 사용**: 씬 주변의 랜드스케이프를 켜거나 끕니다. 기본적으로 활성화되어 있으며, 끄면 씬의 메모리 사용량을 줄일 수 있습니다. 씬 자체 설정에서 이미 지형이 숨겨져 있으면 회색으로 비활성화됩니다.
* **멀티 인스턴스 미리 보기**: 여러 Explorer 창을 동시에 실행할 수 있어 [멀티플레이어 씬을 로컬에서 테스트할 수 있습니다](#test-a-multiplayer-scene-locally). 이 기능을 켜면 각 창에서 로그인을 요청하므로, 각 창에 다른 플레이어로 연결할 수 있습니다. 프로젝트가 SDK 7.20.4 이상을 사용할 때만 표시됩니다.
* **MCP 서버 사용**: MCP 자동화 서버가 활성화된 상태로 Explorer를 실행하여 AI 에이전트가 실행 중인 미리 보기를 보고 제어할 수 있게 합니다. 프로젝트가 SDK 7.25.0 이상을 사용할 때만 표시됩니다. 자세한 내용은 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world) 에서 전체 워크플로를 확인하세요.
* **에셋 압축**: 로컬에서 생성한 에셋 번들로 씬을 미리 보며, 프로덕션에서 렌더링되는 방식과 동일하게 맞춥니다. [에셋 번들 변환](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#asset-bundle-conversion). 첫 실행 시 모든 에셋을 변환하며, 큰 씬에서는 몇 분이 걸릴 수 있습니다. 플랫폼과 프로젝트의 SDK 버전이 이를 지원할 때만 표시됩니다. 자세한 내용은 [최적화된 에셋으로 미리 보기](#preview-with-optimized-assets).

씬의 콘솔 출력에는 메뉴 옵션이 필요하지 않습니다. Creator Hub가 미리 보기를 실행할 때마다 자동으로 열어 줍니다. 자세한 내용은 [Creator Hub 콘솔](#the-creator-hub-console).


> [!NOTE]
> **팁:** Decentraland 모바일 앱에서 직접 씬을 미리 볼 수도 있습니다. 다음 옵션을 사용하거나 **모바일용 QR 코드 표시** Creator Hub에서 해당 옵션을 사용하거나 다음을 실행하세요: `npm run start -- --mobile` CLI에서 실행합니다. 자세한 내용은 [모바일에서 미리 보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) 를 참고하세요.


## CLI 사용

씬을 미리 보려면 씬의 루트 폴더에서 다음 명령을 실행하세요:

```bash
npm run start
```

누락된 종속성이 있으면 설치한 다음, CLI가 시스템에 로컬 웹 서버를 만들고 다음을 통해 Decentraland Desktop 클라이언트에서 씬을 실행합니다: `decentraland://` 딥링크. 데스크톱 클라이언트가 기본 미리 보기 대상입니다.

대신 브라우저 탭에서 미리 보려면 다음을 추가하세요: `-- --web` (또는 `-- --bevy-web`)를 추가해 다음 위치의 Bevy Web 클라이언트에서 씬을 여세요: `decentraland.org/bevy-web/`.

씬을 변경할 때마다 미리 보기가 자동으로 다시 로드되고 업데이트되므로, 명령을 다시 실행할 필요가 없습니다.


> [!WARNING]
> **📔 참고**: 일부 씬은 사용자 지정 로직을 수행하거나 데이터를 저장 및 조회하기 위해 외부 서버와 통신해야 합니다. 이러한 씬을 미리 볼 때는 다른 포트에서 서버도 로컬로 실행해야 할 가능성이 큽니다. 씬과 서버를 함께 실행하는 방법은 씬의 README를 확인하세요.


### 미리 보기 명령의 매개변수

다음 플래그를 `npm run start` 명령에 추가하여 동작을 변경할 수 있습니다:

* `-- --web` (별칭 `-- --bevy-web`) 다음 위치의 Bevy Web 브라우저 클라이언트에서 미리 보기를 엽니다: `decentraland.org/bevy-web/` Desktop Explorer 대신 사용됩니다. Chromium 기반 브라우저(Chrome 142+)에서는 호스팅된 페이지가 로컬 미리 보기 서버에 연결하려면 Local Network Access 권한이 필요합니다. 브라우저에서 기기의 앱에 접근하라고 요청하면 "Allow"를 클릭하세요.
* `-- --mobile` (별칭 `-- -m`) 터미널에 QR 코드를 표시하여 씬을 다음에서 열 수 있게 합니다: [Decentraland 모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) 동일한 Wi‑Fi 네트워크에 연결된 휴대폰에서 사용할 수 있습니다. 자세한 내용은 [모바일에서 미리 보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/).
* `-- --skip-build` 빌드를 건너뛰고 미리 보기 모드에서 파일만 제공합니다.
* `-- --port` (별칭 `-- -p`) 씬을 실행할 특정 포트를 지정합니다. 지정하지 않으면 사용 가능한 포트를 사용합니다.
* `-- --no-browser` (별칭 `-- -b`) 미리 보기가 새 브라우저 탭을 열지 않도록 합니다.
* `-- -w` 또는 `-- --no-watch` 파일 시스템 변경을 감시하지 않아 씬 코드가 바뀔 때마다 핫 리로드가 발생하지 않도록 합니다.
* `-- --ci` 원격 Unix 서버에서 parcel 미리 보기 도구를 실행합니다.
* `-- --multi-instance` 여러 Explorer 인스턴스를 동시에 실행할 수 있게 합니다.
* `-- --asset-bundles` 최적화된 에셋 번들로 미리 봅니다. Desktop Explorer가 미리 보기 중에 씬의 에셋을 자체적으로 에셋 번들로 변환하여, 프로덕션에서 씬이 렌더링되는 방식과 맞춥니다. 다음과 동일합니다: **에셋 압축** Creator Hub의 옵션입니다. 데스크톱 클라이언트에서만 사용할 수 있으며(Bevy Web 클라이언트에서는 사용할 수 없음), 자세한 내용은 [최적화된 에셋으로 미리 보기](#preview-with-optimized-assets).
* `-- --no-client` 모든 자동 실행(데스크톱 Explorer 딥링크, 브라우저 열기, 모바일 QR)을 억제합니다. 파일 감시기는 Explorer가 자체적으로 연결되면 여전히 데스크톱 Explorer에 알립니다. 외부 도구가 Explorer 프로세스를 관리할 때 유용합니다.
* `-- --mcp` Explorer에서 MCP 서버를 활성화합니다(딥링크 매개변수로 전달됨).
* `-- --mcp-port` Explorer의 MCP 서버 포트입니다(딥링크 매개변수로 전달됨). 예: `npm run start -- --mcp --mcp-port 3001`.


> [!WARNING]
> **📔 참고**: 매개변수는 두 개의 대시 묶음으로 추가해야 합니다. 예: `npm run start -- --web3`.


## 최적화된 에셋으로 미리 보기

씬을 게시하면 Decentraland 서버가 씬의 모든 3D 모델을 에셋 번들로 변환합니다. 이는 로드와 렌더링이 훨씬 가벼운 최적화 형식입니다. 자세한 내용은 [에셋 번들 변환](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#asset-bundle-conversion). 기본적으로 로컬 미리 보기는 이 단계를 건너뛰고 대신 원본 모델을 불러옵니다.

씬을 미리 볼 때 동일한 변환을 로컬에서 실행하려면 최적화된 에셋을 사용하세요. 여기에는 두 가지 장점이 있습니다:

* 미리 보기가 더 빠르게 로드되고 더 부드럽게 실행됩니다. 특히 무거운 3D 모델이 있는 씬에서 효과적입니다.
* 게시 후 플레이어가 보게 될 모습과 정확히 같은 모델을 볼 수 있으므로, 게시 전에 에셋 압축 문제를 발견할 수 있습니다.

이 옵션은 로컬 미리 보기에만 영향을 줍니다. 게시된 씬은 이 옵션 사용 여부와 관계없이 항상 서버에서 변환됩니다.

활성화하려면:

* **Creator Hub에서**: 열기 **실행 옵션** 다음 옆의 **실행** 버튼을 클릭하고 마우스를 올린 다음 **데스크톱 클라이언트**, 그리고 다음을 선택하세요: **에셋 압축**.
* **CLI에서**: 실행하세요 `npm run start -- --asset-bundles`.

![](/docs/_remote/b1d24b74a3fe9e049ed203e0.webp)

이 옵션을 사용하면 Decentraland explorer가 씬의 3D 모델을 불러오기 전에 사용자의 컴퓨터에서 에셋 번들로 변환합니다. 대부분의 씬에서는 변환이 거의 즉시 이루어지지만, 매우 무거운 에셋이 있는 씬은 첫 미리 보기에 더 오래 걸릴 수 있습니다. 변환된 모델은 캐시에 저장되므로 이후 미리 보기에서는 새로 추가되거나 수정된 에셋만 변환하면 됩니다.


> [!NOTE]
> **💡 팁**: 어떤 이유로든 에셋 변환에 실패하면, 일반 미리 보기처럼 원본 3D 모델을 불러오는 방식으로 되돌아갑니다.


## Decentraland에 씬 업로드

씬이 만족스럽다면 업로드하여 Decentraland에 게시할 수 있습니다. 이를 위해서는 LAND, Decentraland NAME, ETH ENS 이름을 소유하고 있거나, 그런 권한을 가진 사람에게서 권한을 받아야 합니다. 자세한 내용은 [게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/) 에서 방법을 확인하세요.

## 씬 크기 미리 보기

미리 보기에 표시되는 씬 크기는 씬의 구성에 기반합니다.

Scene Editor의 씬 메뉴 두 번째 탭에서 이를 수정할 수 있습니다.

![](/docs/_remote/5bcc0f6dfa70093b896862ae.webp)

드롭다운을 사용한 다음 클릭하세요: **레이아웃 적용** 을 눌러 씬의 크기를 변경하세요. 각 개별 parcel을 클릭하여 레이아웃에서 제외할 수도 있습니다.

![](/docs/_remote/c2905f95052e96703e78829e.webp)

다음도 편집할 수 있습니다: *scene.json* 파일에서 "parcels" 필드에 여러 parcel을 나열할 수 있습니다. 자세한 내용은 [명령줄로 parcel 설정](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-metadata/#scene-parcels) 을 참고하세요.


> [!NOTE]
> **💡 팁**: 미리 보기 실행 중에는 parcel 좌표가 실제 씬에서 사용할 좌표와 정확히 일치할 필요는 없으며, 서로 인접해 있고 같은 형태로 배열되어 있기만 하면 됩니다. 나중에 씬을 [배포할 때](#upload-a-scene-to-decentraland).


## 씬 콘솔 보기

다음을 클릭하여 콘솔을 여세요: ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 오른쪽 상단 모서리의 아이콘. 여기에서 오류 메시지와 함께 씬이 다음을 통해 콘솔에 출력하는 텍스트도 볼 수 있습니다: `console.log()`.

다음을 눌러서도 열 수 있습니다: **\`** 키보드의 키입니다. 또한 Shift + **\`** 를 눌러 콘솔을 더 넓게 열 수 있습니다. 더 많은 텍스트를 봐야 할 때 유용합니다.

## Creator Hub 콘솔

Creator Hub는 편집기를 벗어나지 않고도 동일한 출력을 표시하며, 기본적으로 그렇게 동작합니다. 미리 보기를 실행하면 **CONSOLE** 탭이 다음 옆에 나타납니다: **FILE SYSTEM** 및 **ASSET PACKS** 가 편집기 하단 패널에 표시됩니다.

콘솔이 열리지 않게 하려면 다음 선택을 해제하세요: **디버그 콘솔 열기** 아래의 **실행 옵션** 다음에서 **EDITOR** Creator Hub 설정 탭.

콘솔을 방해되지 않게 옮기려면 헤더의 팝아웃 아이콘을 클릭하세요. 아이콘에는 다음 문구가 표시됩니다: **콘솔을 별도 창으로 열기**. 콘솔은 다음 제목의 별도 창으로 열립니다: **콘솔**이며, 두 번째 모니터로 옮겨 편집하는 동안 계속 볼 수 있습니다.

별도 창에는 **콘솔 지우기** 버튼과 **편집기 안으로 다시 도킹** 버튼이 있어 콘솔을 하단 패널로 되돌릴 수 있습니다. 한편 **CONSOLE** 편집기 탭에는 다음이 표시됩니다: **콘솔이 별도 창에서 열렸습니다** 그리고 다음 버튼을 제공합니다: **여기로 다시 도킹** 버튼도 동일한 작업을 합니다.


> [!NOTE]
> **💡 팁**: 콘솔은 분리된 상태에서도 출력을 계속 수집하므로, 다시 도킹해도 아무것도 잃지 않습니다. 미리 보기가 실행 중이 아니면 다음이 표시됩니다: **씬을 실행하여 디버그 출력 보기**.


## 멀티플레이어 씬을 로컬에서 테스트하기

씬 미리 보기를 실행하여 두 개 이상의 서로 다른 explorer 창에서 열면, 각 열린 창은 별도의 플레이어로 인식되며 모의 통신 서버가 이 플레이어들을 동기화 상태로 유지합니다.

한 창에서 씬과 상호작용한 다음 다른 창으로 전환하면 그 상호작용의 결과도 거기에서 보입니다.

Creator Hub에서 다음을 켭니다: **멀티 인스턴스 미리 보기** 다음의 **데스크톱 클라이언트** 의 하위 메뉴 **실행 옵션**, 그런 다음 다음을 클릭하세요: **실행** 버튼을 두 번째로 눌러 두 번째 Decentraland explorer 창을 여세요. 두 창 모두 서로 다른 주소로 연결해야 합니다. 씬이 다시 로드되어도 같은 세션이 열린 상태로 유지됩니다.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

대안으로, 다음을 브라우저 URL에 입력하여 두 번째 Decentraland explorer 창을 열 수 있습니다:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### 고급: 원격 에셋 번들로 빠르게 반복하기


> [!NOTE]
> **💡 팁**: 대부분의 씬에서 에셋 번들로 미리 보는 가장 간단한 방법은 다음을 활성화하는 것입니다: [최적화된 에셋](#preview-with-optimized-assets), 이를 통해 모델을 로컬에서 변환하고 항상 최신 버전을 반영합니다. 아래 모드는 서버에 이미 게시된 번들을 재사용하는 대안입니다.


많은 3D 모델이 있는 무거운 씬에서는 다음을 재사용하여 씬 로드와 다시 로드를 더 빠르게 할 수 있습니다: [에셋 번들](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#asset-bundle-conversion) Decentraland 서버에 이미 게시된 것들을 사용하면, 최적화되지 않은 원본 3D 모델을 불러오는 대신 사용할 수 있습니다. 특히 코드만 변경하며 반복 작업할 때 유용합니다.

이 모드를 활성화하려면 다음 인수로 Decentraland Desktop 클라이언트를 실행하세요:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab <ab-source>
```

다음은 `<ab-source>` 인수이며, 씬이 이미 게시된 위치에 따라 달라집니다:

* **Genesis City에서**: `--lsd-remote-ab-server Genesis`
* **World에서**: `--lsd-remote-ab-world <world-name>.dcl.eth`

예를 들어, 이미 World에 배포된 씬의 로컬 복사본을 미리 보려면:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab --lsd-remote-ab-world myworld.dcl.eth
```

두 경우 모두 `--realm http://127.0.0.1:8000/` 는 클라이언트를 로컬 미리 보기 서버로 지정합니다( `npm run start` 먼저 실행해야 합니다), 그리고 `--local-scene true` 는 클라이언트가 그곳에서 씬 코드를 불러오도록 합니다.


> [!WARNING]
> **📔 중요**: 이 모드를 사용할 때는 다음을 권장합니다: **모든** 아트가 이미 게시되어 있고, 해당 에셋 번들이 콘텐츠 서버에서 완전히 처리된 상태여야 합니다. 새 에셋을 추가했다면 최적화된 로딩을 이용하지 못하고, 일반 미리 보기처럼 원시 gltf 파일로 로드됩니다. 하지만 이미 게시된 에셋을 같은 파일 이름으로 로컬에서 수정했다면, 그 에셋의 이전 게시 버전이 표시됩니다.
> 
> 그런 경우에는 먼저 씬을 다시 배포하고, 에셋 번들이 생성될 때까지 몇 분 기다린 다음(참고: [에셋 번들 변환](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#asset-bundle-conversion)), 그다음 코드만 반복 작업할 때 이 모드를 다시 사용하세요.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/preview-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
