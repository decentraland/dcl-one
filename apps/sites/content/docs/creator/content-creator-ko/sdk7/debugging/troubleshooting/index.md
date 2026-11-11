> 일반적인 문제에 대한 해결책

# 문제 해결

## AI 도우미로 디버그하기

아래의 문제들을 살펴보기 전에, 문제를 Cursor의 채팅, GitHub Copilot, Claude Code 같은 AI 도우미에게 맡겨 보세요. 콘솔의 오류 메시지를 붙여 넣거나, 무엇이 예상대로 동작하지 않는지 설명하면, 보통 씬 코드에서 문제를 찾아서 고쳐 줍니다.

좋은 결과를 얻으려면, AI에 Decentraland SDK 스킬이 설치되어 있는지 확인하세요. 이 스킬은 SDK의 패턴과 제약을 알려 주므로, 일반적이거나 오래된 정보에 근거해 추측하지 않게 됩니다. 씬 프로젝트에서 다음 명령을 실행해 설치하세요:

```bash
npx skills add decentraland/sdk-skills
```

참조 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) AI 도우미 설정 및 프롬프트에 대한 자세한 내용은

또한 AI가 씬을 디버그하도록 할 수도 있습니다 *실행 중인 동안*에. Decentraland 데스크톱 클라이언트는 에이전트가 자체적으로 스크린샷을 찍고, 씬의 콘솔 출력을 읽고, 플레이어를 이동시키고, 오브젝트를 클릭할 수 있게 하는 MCP 서버를 노출할 수 있습니다. 그래서 직접 버그를 재현하고 오류를 붙여 넣는 대신, 에이전트가 스스로 재현하고, 무엇이 일어나는지 보고, 수정될 때까지 반복합니다. 씬을 다음으로 실행하고 `npm run start -- --mcp` 에 연결하세요.

참조 [AI가 당신의 씬을 세계 안에서 보게 하세요](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/#let-the-ai-see-your-scene-in-world) 전체 설정은 여기에서 확인하고, `unity-explorer-mcp` 스킬을 설치하면 에이전트가 작업 흐름을 알 수 있습니다:

```bash
npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
```

## 미리보기 실행 시 문제

#### 문제: 어떤 씬 미리보기도 실행할 수 없고, 오류 메시지에 다음이 언급됩니다 **권한 거부** 또는 **EACCES**

운영 체제에서 프로젝트를 실행하려는 폴더의 권한을 수정할 수 없습니다. 씬을 실행할 때 일부 종속성을 설치해야 하지만, 허용되지 않습니다. Windows/Mac/Linux 사용자 계정이 해당 폴더의 파일을 수정할 수 있도록 폴더 권한을 설정해야 합니다.

유용한 자료:

* [docs.npmjs](https://docs.npmjs.com/resolving-eacces-permissions-errors-when-installing-packages-globally)
* [letscodepare](https://letscodepare.com/blog/npm-resolving-eacces-permissions-denied)

#### 문제: 특정 씬 미리보기를 실행할 수 없고, 오류에 다음이 표시됩니다 **오류: 프로젝트를 빌드하는 중 오류 발생**

공유받은 씬을 실행 중이라면, 이 씬이 다음을 포함한 상태로 공유되지 않았는지 확인하세요 `node_modules` 또는 `bin` 폴더 또는 `package-lock.json` 파일. 이러한 파일에는 운영 체제와 기기에 특화된 버전을 사용하는 종속성이 들어 있으며, 씬을 처음 실행할 때 생성되어야 합니다. 이러한 폴더와 파일을 수동으로 삭제한 다음, 다시 실행하세요 `npm run start` .

#### 문제: 실행 `npm run start` 은 실행되지만 오류 메시지는 없고, 브라우저 창도 열리지 않으며 미리보기를 열 수 있는 출력 URL도 없습니다

Node 버전이 최신인지 확인하세요. 20 이상이어야 합니다.

#### 문제: 실행 `npm run start` 브라우저 탭은 열리지만 로딩 화면이 끝까지 로드되지 않거나, 빨간색 오류 배너에 "critical error"라고 표시됩니다.

* 프로젝트에 최신 버전의 Decentraland SDK가 설치되어 있는지 확인하세요. 다음을 실행하세요:

  `npm i @dcl/sdk@latest`

#### 문제: 씬은 실행되는데 콘솔에서 다음이 보입니다 `순환 종속성` 경고.

이는 씬의 파일들이 서로를 상호 참조한다는 뜻입니다. 이것이 반드시 문제인 것은 아니지만, 디버깅하기 어려운 경쟁 상태 및 기타 문제로 이어질 수 있으므로 소프트웨어를 작성할 때 권장되는 패턴은 아닙니다. 이러한 경고가 있어도 씬은 아마 잘 동작할 것입니다.

이상적으로는, 씬의 코드 로딩이 명확한 순차적 순서를 따라야 합니다. 순환 종속성이 있는 코드는 닭과 달걀 문제를 겪을 수 있는데, 컴파일러가 무엇을 먼저 시작해야 하는지 알지 못하기 때문입니다. 이는 종종 문제 없이 해결되지만, 피하는 것이 좋은 관행입니다.

이러한 종속성을 수정하려면, 함수 인수에 이미 인스턴스화된 엔티티/오브젝트에 대한 참조를 전달하는 함수나 오브젝트 생성자를 호출하는 방법을 사용해야 하는 경우가 많습니다. 함수 안에 이러한 엔티티/오브젝트에 대한 참조를 하드코딩하는 대신, 인스턴스화되었을 수도 있고 아닐 수도 있는 참조를 사용하지 마세요.

## 배포 시 문제

#### 문제: 이 파셀들에 배포할 권한이 없습니다

* 다음이 `scene.json` 파일에 배포하려는 좌표가 올바르게 나열되어 있는지 확인하세요.
* Metamask가 올바른 지갑으로 거래에 서명하도록 제대로 설정되어 있는지 확인하세요. 이는 LAND 토큰을 소유한 지갑일 수도 있고, 소유자로부터 오퍼레이터 권한을 부여받은 지갑일 수도 있습니다.

#### 문제: 실행 `npm run deploy` 실패합니다

* 씬의 스폰 지점을 확인하세요. 스폰 지점의 x,y,z 좌표 세 개는 모두 숫자이거나 범위여야 합니다. 세 개가 모두 숫자이거나, 모두 범위여야 합니다. 일부는 범위이고 일부는 숫자인 경우는 지원되지 않습니다.

  예를 들어, 이는 지원되지 않습니다:

  `"position": {"x": [1,4], "y": 0, "z": [1,4]}`

  이는 지원됩니다:

  `"position": {"x": [1,4], "y": [0,0], "z": [1,4]}`
* 배정된 기본 catalyst 서버가 다운되었거나 문제가 있을 수 있습니다. 대신 특정 catalyst 서버에 배포하도록 `npm run deploy` 명령을 강제로 사용할 수 있습니다. Decentraland Editor에서 특정 서버에 배포하려면:

  1. 씬을 열고 다음을 클릭하세요 **게시**
  2. 옵션을 선택하세요 **다른 서버에 게시** 아래쪽의
  3. 드롭다운에서 다음을 선택하세요 **사용자 지정 서버**
  4. 서버 주소를 입력하세요. 예: `peer-ec1.decentraland.org`
  5. 클릭 **커스텀 서버에 게시**
  6. 일반 배포와 마찬가지로 거래를 승인하세요.

  CLI로 하려면:

  `npm run deploy -- --target-content <server-name>`

  예를 들면:

  `npm run deploy -- --target-content peer-ec1.decentraland.org`

  참조 [catalyst-monitor](https://decentraland.github.io/catalyst-monitor/) 를 사용해 catalyst 네트워크의 모든 서버 상태를 확인하세요. 각 카드 상단에서 각 서버의 주소도 복사할 수 있습니다.
* 씬의 `package.json`. 흔한 문제는 다음이 있다는 것입니다 `bundleDependencies` 그리고 또한 `bundledDependencies` (추가 d가 있는) 섹션입니다. 이는 같은 프로젝트에서 서로 다른 시점에 서로 다른 Node 버전을 실행했거나, 서로 다른 Node 버전이 설치된 사람들 사이에서 프로젝트를 공유했기 때문에 발생할 수 있습니다. `bundleDependencies`을 삭제하세요. 이는 이전 Node 버전과 관련이 있습니다.

또한 Node 버전이 최신인지, 최소 20버전인지 확인하세요.

#### 문제: 실행 `npm run deploy` 또는 `npm run build` 형식 오류를 보고합니다

씬에 TypeScript가 보고하는 형식 오류가 있을 수 있습니다. 예를 들어 특정 변수가 다음 형식일 수 있다고 말할 수 있습니다 `어떤` 또는 다음과 같다는 것입니다 `undefined` 또는 `null` 는 허용되지 않습니다. 실행할 때 `npm run deploy`도 함께 실행되며, `npm run build`은 이러한 검사에 대해 `npm run start`.

JavaScript와 달리 TypeScript는 모든 변수에 엄격한 형식 지정을 강제합니다. 예를 들어 특정 값이 절대 `undefined`가 되지 않도록 씬이 작성되어 있더라도, TypeScript는 그 시나리오에서 무슨 일이 일어나는지 알아야 하며, 아니면 그 값이 예를 들어 문자열만 될 수 있다고 명시적으로 알려야 합니다.

대안으로, 다음을 실행할 수 있습니다 `npm run deploy -- --skip-build` 실행을 건너뛰려면 `npm run build`을 건너뛰고, 이러한 검사가 실행되지 않도록 합니다.

#### 문제: 씬을 배포했지만 Decentraland에 들어가도 변경 사항이 보이지 않습니다

* 씬에 무거운 에셋이 포함되어 있으면 몇 분이 걸릴 수 있습니다.
* 새 버전의 3D 모델이 에셋 번들로 변환되는 동안, 플레이어에게는 의도적으로 씬의 마지막으로 완전히 작동하던 버전이 제공됩니다. 보통은 몇 초밖에 걸리지 않지만, 아주 큰 씬이거나 서버가 바쁠 때는 더 오래 걸릴 수 있습니다. 다음을 [변환 상태를 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 하여 씬을 직접 확인할 수 있습니다.
* 씬을 새로 고치는 것만으로는 새 버전을 반영할 수 없습니다. 새로 고침은 씬 코드를 다시 시작할 뿐, 새로 게시된 버전은 가져오지 않습니다. 변환이 완료되면 Decentraland를 완전히 종료했다가 다시 실행한 뒤, 점프 링크 또는 `/goto` 채팅 명령어를 통해 다시 씬에 들어가세요.

#### 문제: 일부 플레이어는 내 씬의 새 버전을 보지만, 다른 플레이어는 여전히 이전 버전을 봅니다

씬의 3D 모델 변환은 플랫폼(Windows 및 Mac)마다 완료 시점이 다르며, 각 플레이어의 클라이언트도 로컬 캐시를 유지합니다. 모든 플랫폼에서 변환이 완료되었는지 확인하려면, [변환 상태를 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 그리고 다음 항목을 비교하세요 `윈도우` 및 `맥` 값을 `assetBundles`아래에서 `완료`을 확인한 뒤, 영향을 받은 플레이어들에게 Decentraland를 완전히 종료하고 다시 실행하도록 요청하세요.

#### 문제: 게시가 Converting 단계에서 멈춰 있고, Jump In 버튼이 끝내 나타나지 않습니다

씬이 변환 중인 다른 씬들 뒤에서 대기열에 있거나, 변환에 실패한 것입니다.

* Open `https://asset-bundle-registry.decentraland.org/queues/status` 그리고 씬의 엔티티 ID를 찾으세요. 해당 ID는 다음에서 확인할 수 있습니다 `entityId` 필드에서 다음을 [변환 상태를 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 사용해 씬의 좌표로 조회할 때. ID가 대기열에 나열되어 있다면, 씬은 대기 중인 것이므로 기다리기만 하면 됩니다.
* 대기열에 없다면, [변환 상태를 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 씬의 `실패함`으로 표시되면, 씬을 다시 게시하세요. 두 번째도 실패하면, [문제를 신고하고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/) 엔티티 ID를 포함하세요.

#### 문제: 배포 후 일부 3D 모델이 사라지거나, 검게 보이거나, 텍스처가 없습니다

* 방금 게시했다면, 모델을 에셋 번들로 변환하는 작업이 아직 끝나지 않았을 수 있습니다. 다음을 입력하세요 `/detectabs` 채팅 창에: 빨간색으로 표시된 모델은 아직 변환되지 않은 것입니다. [변환 상태 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 그리고 완료될 때까지 기다리세요.
* 3D 모델과 그 바운딩 박스까지 모두 씬 경계 안에 있는지 확인하세요. 미리보기를 실행할 때 모델의 일부가 이 경계를 넘어가면, 넘어간 부분은 잘려 나가고 렌더링되지 않습니다. 이는 미리보기와 게시된 씬 모두에서 동일합니다.
* 텍스처가 문제라면, 3D 모델의 텍스처는 변환 중 최대 1024x1024 픽셀로 제한된다는 점을 기억하세요.

#### 문제: 씬이 가까이서는 괜찮아 보이지만, 멀리서 보면 깨지거나 사라집니다

멀리서 씬을 렌더링하는 데 쓰이는 자산의 낮은 Level of Detail(LOD) 버전은 게시의 마지막 단계에서 생성되며, 아직 완료되지 않았을 수 있습니다. 이는 가까운 거리에서 씬을 테스트하는 것을 막지는 않습니다. [변환 상태 확인](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/#check-the-conversion-status) 그리고 아래의 값을 확인하세요 `lods`또는 그냥 기다렸다가 나중에 다시 확인하세요.

#### 문제: 배포 후 내 3D 모델이 다르게 보입니다

* 텍스처가 다르게 보인다면, 3D 모델의 텍스처는 최대 1024x1024 픽셀로 제한된다는 점을 기억하세요. 이 변환은 Decentraland가 모두에게 원활하게 동작하도록 보장하기 위해 수행됩니다.
* 모델이 다르게 보인다면, 모델의 에셋 번들 변환에 문제가 있을 수 있습니다. 에셋 번들 압축에 대해 더 읽어보세요 [여기](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#asset-bundle-conversion).

  이를 검증하려면, URL 매개변수로 씬을 실행해 보세요 `&DISABLE_ASSET_BUNDLES`. 이 플래그를 사용했을 때 모델이 정상이라면, 문제는 모델 변환 버그와 관련이 있어야 합니다. 그런 경우, [문제를 신고하고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/) 그리고 배포의 엔티티 ID를 포함하세요.

  모델의 압축 에셋 번들 버전을 생성하는 데 서버에 약간의 시간이 걸린다는 점에 유의하세요(보통 몇 분, 아주 큰 씬이거나 서버가 바쁠 때는 더 오래 걸립니다). 다음 명령을 채팅 창에 입력하면 모델이 압축 에셋 번들로 로드되는지 여부를 확인할 수 있습니다 `/detectabs`. 압축된 모델은 초록색으로 표시되고, 비압축 모델은 빨간색으로 표시됩니다.

  또한 게시 전에 로컬에서 이 변환을 재현할 수 있습니다. 다음을 활성화하면 됩니다 [미리보기의 최적화된 에셋](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/#preview-with-optimized-assets). 이렇게 하면 씬을 배포하지 않고도 이러한 문제를 발견하고 디버그할 수 있습니다.

#### 문제: 씬이 완전히 사라졌습니다

* 씬이 World에 있는 경우: NAMEs, LAND 또는 MANA를 판매하거나 이전한 뒤 등으로 인해 Worlds 저장 공간 예산이 초과되었을 수 있습니다. Worlds에 접근할 수 없게 되기 전에 48시간 안에 공간을 확보하거나 예산을 늘려야 합니다. Creator Hub의 **관리** 탭 또는 **Worlds** 탭에서 [빌더](https://decentraland.org/builder/worlds)에서 예산을 확인한 뒤, 다시 게시하세요. 다음을 참조하세요 [World 크기 제한](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/#size-limits).
* 씬이 LAND에 있는 경우: 배포 권한이 있는 사람이 여러분의 파셀 위에 새 씬을 게시했을 수 있으며, 그로 인해 이전 콘텐츠가 지워집니다. 다음을 참조하세요 [씬 덮어쓰기](/docs/creator/content-creator-ko/sdk7/publishing/publishing/#scene-overwriting).
* 드문 경우지만, Decentraland의 콘텐츠 정책을 위반하는 콘텐츠는 콘텐츠 서버에서 차단 목록에 올려질 수 있습니다. 이것이 실수라고 생각된다면, 다음을 통해 연락하세요 [Decentraland Discord](https://decentraland.org/discord).

#### 문제: 미리보기에서는 매끄럽게 실행되는데, 프로덕션에서 씬의 FPS가 낮습니다.

나쁜 관행을 따르는 주변 씬들 역시 병렬로 실행되므로, 씬의 성능에 영향을 줄 수 있습니다. 설정을 열고 시야 범위를 최소로 설정해서 현재 씬 주변의 1개 파셀만 로드되도록 하면, 이것이 원인인지 확인할 수 있습니다.

다음 매개변수로 씬을 실행하면 시야 범위를 더 줄일 수 있습니다 `&LOS=0`를 사용하여 주변 씬을 전혀 로드하지 않도록 할 수 있습니다.

씬을 방금 배포했다면, 서버가 씬의 3D 모델을 압축 에셋 번들로 변환한 뒤 씬을 로드할 때의 부담도 줄어들 수 있습니다. 다음 명령을 채팅 창에 입력하면 모델이 압축 에셋 번들로 로드되는지 여부를 확인할 수 있습니다 `/detectabs`. 압축된 모델은 초록색으로 표시되고, 비압축 모델은 빨간색으로 표시됩니다.

### 버그 신고

문제가 장면 자체가 아니라 Decentraland SDK 일반에 관한 것이라면, 다음을 참고하세요. [버그 신고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/debugging/troubleshooting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
