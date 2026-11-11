> Creator Hub와 공식 Decentraland SDK Skills(npx skills add decentraland/sdk-skills)를 사용해 AI 어시스턴트로 Decentraland 씬을 만드세요.

# AI와 함께하는 바이브 코딩

원하는 내용을 설명하기만 하면 Decentraland 씬을 만들 수 있습니다. AI 어시스턴트가 SDK7 코드, ECS 아키텍처, 프로젝트 구조를 대신 처리해 줍니다.

처음 만드는 제작자든 숙련된 개발자든, AI 지원 "바이브 코딩"을 사용하면 아이디어를 몇 시간이 아니라 몇 분 만에 실행 중인 씬으로 만들 수 있습니다.


> [!NOTE]
> **💡 팁**: 시작하려면 TypeScript를 알 필요가 없습니다. AI 어시스턴트가 평이한 언어 설명만으로 동작하는 씬 코드를 생성해 줄 수 있습니다.


## 바이브 코딩이란?

바이브 코딩은 코드 한 줄 한 줄을 직접 작성하는 대신 AI 어시스턴트와 대화하면서 씬을 만드는 것을 뜻합니다. 원하는 것을 설명하면 — "클릭 가능한 문과 배경 음악이 있는 중세 선술집" — AI가 올바르고 배포 가능한 SDK7 코드를 작성합니다.

이 방식은 어떤 숙련도에서도 잘 작동합니다:

* **초보자 및 비개발자** — 코드를 직접 쓰지 않고도 처음부터 동작하는 씬까지 갈 수 있습니다.
* **숙련 개발자** — 보일러플레이트는 건너뛰세요. AI가 멀티플레이어 동기화, UI 스캐폴딩, 배포 설정을 처리하게 하고, 당신은 창의적인 결정에 집중하세요.
* **팀 및 스튜디오** — 전체 개발 리소스를 투입하기 전에 씬 콘셉트를 빠르게 프로토타이핑하세요.

## Creator Hub에서 AI 어시스턴트 사용

가장 빠르게 시작하는 방법은 Creator Hub에 내장된 [AI 어시스턴트](/docs/creator/content-creator-ko/scene-editor/extend-with-code/ai-assistant/). 이 기능은 열려 있는 씬을 보고, 엔티티, 컴포넌트, 스마트 아이템, 코드를 대신 편집해 주며, 미리보기를 실행해 스스로 작업 결과를 확인할 수도 있습니다. 아래의 스킬들을 프로젝트에 자동으로 설치하므로, 코딩 CLI에 로그인하는 것 외에는 별도 설정이 필요 없습니다.

이는 기본적으로 꺼져 있는 실험 기능입니다.

## 코드 편집기와 AI를 함께 사용하기

다음과 같은 범용 AI 코드 편집기를 사용하세요. [Cursor](https://www.cursor.com/) 또는 VS Code와 GitHub Copilot이나 Claude AI를 함께 사용하세요. Decentraland는 이러한 도구가 SDK를 이해할 수 있도록 컨텍스트 폴더를 제공합니다.

1. Creator Hub를 열고 씬을 만들거나 여세요.
2. 를 클릭하세요 **< > CODE** 버튼을 눌러 코드 편집기를 여세요.
3. 편집기에 내장된 AI 어시스턴트(Cursor의 채팅, Copilot 등)를 사용해 코드를 생성하거나 수정하세요.

## 어떤 AI 에이전트든 사용할 수 있는 스킬 설치

스킬은 AI 에이전트가 Decentraland SDK와 작업하는 방법을 익히도록 도와주는 미리 만들어진 지침 세트입니다. 각 스킬은 씬 만들기, 3D 모델 추가, 멀티플레이어 설정처럼 특정 주제를 다루므로, AI는 여러분이 일일이 설명하지 않아도 올바른 패턴, API, 제약 조건을 이미 알고 있습니다. 스킬을 설치하면 첫 프롬프트부터 실수가 줄고 결과가 더 좋아집니다.

```bash
# 대화형 선택기에서 설치할 Decentraland 스킬을 고르기
npx skills add decentraland/sdk-skills

# 또는 Decentraland 스킬 전체 설치
npx skills add decentraland/sdk-skills --all

# 또는 특정 스킬 선택
npx skills add decentraland/sdk-skills --skill create-scene

# 전역으로 설치(모든 프로젝트에서 사용 가능)
npx skills add decentraland/sdk-skills -g
```

이렇게 하면 스킬 파일이 에이전트의 설정에 복사되어 Decentraland의 패턴과 제약을 알게 됩니다.

## 스킬 업데이트

새 스킬은 계속 추가되고 기존 스킬도 개선됩니다. 최신 버전을 받으려면 다음 옵션으로 설치 명령을 다시 실행하세요. `--all`:

```bash
# 설치된 모든 스킬을 업데이트하고 새로 추가된 스킬도 다운로드
npx skills add decentraland/sdk-skills --all
```

다시 `add` 를 실행하면 저장소를 다시 가져오므로, 이미 설치된 스킬은 새로고침되고 첫 설치 이후 추가된 스킬도 설치됩니다. 스킬을 전역으로 설치했다면 이 명령에 `-g` 도 추가하세요.


> [!WARNING]
> **📔 참고**: 여기서는 `npx skills update` 를 사용하지 마세요. 이 명령은 이미 기기에 있는 스킬만 새로 고치므로, 마지막 설치 이후 저장소에 추가된 스킬은 조용히 건너뜁니다. 항상 `npx skills add decentraland/sdk-skills --all` 를 대신 사용하세요.


## 배포된 멀티플레이어 씬 디버그하기

업데이트된 `authoritative-server` 스킬은 배포된 씬을 조사해 달라고 요청하면 지속적 서버 로그를 검색하고, 일정 기간 로그를 추적하며, CPU, 메모리 및 기타 메트릭을 가져올 수 있습니다. 먼저 스킬과 씬의 `auth-server` SDK를 업데이트하세요. 설정, 권한, 제한은 [Scene Insights](/docs/creator/content-creator-ko/sdk7/debugging/scene-insights/) 를 참고하세요.

예를 들어 다음과 같이 요청하세요:

> 지난 2시간 동안 내 배포된 멀티플레이어 씬의 오류와 CPU 또는 메모리 증가를 조사해줘. Scene Insights를 사용하고 로그와 메트릭을 소스 코드와 연관 지어 봐줘.

지갑 로그인은 브라우저에서 직접 승인하세요. 그러면 어시스턴트가 읽기 전용 세션을 재사용할 수 있으며, 개인 키나 세션 파일은 필요하지 않습니다.

## 사용 가능한 AI 스킬

에이전트에 스킬을 설치하면 다음 기능을 사용할 수 있습니다:

| 스킬                     | 기능                                          |
| ---------------------- | ------------------------------------------- |
| `sdk-scenes`           | 에이전트 지침과 모든 토픽 스킬의 색인이 있는 진입점               |
| `create-scene`         | 처음부터 새 SDK7 씬 프로젝트를 스캐폴딩                    |
| `migrate-sdk6-to-sdk7` | 레거시 SDK6 씬을 SDK7로 포팅                        |
| `add-3d-models`        | 3D 모델 추가(`.glb`/`.gltf`) 배치, 크기 조정, 충돌체 포함  |
| `add-interactivity`    | 포인터 이벤트, 트리거, 레이캐스트                         |
| `build-ui`             | React-ECS를 사용한 2D 화면 공간 UI — HUD, 메뉴, 대화 상자 |
| `animations-tweens`    | Animator와 SDK tween을 사용한 GLTF 모델 애니메이션      |
| `multiplayer-sync`     | CRDT 네트워킹을 사용하는 피어투피어 멀티플레이어                |
| `authoritative-server` | 서버 검증형 씬용 Headless 멀티플레이어 서버(BETA)          |
| `audio-video`          | 사운드 효과, 음악, 오디오 스트리밍, 비디오 플레이어              |
| `audio-analysis`       | 오디오 반응형 씬을 위한 실시간 진폭 및 주파수 데이터              |
| `deploy-scene`         | Genesis City(LAND 기반)에 씬 배포                 |
| `deploy-worlds`        | Worlds(개인 3D 공간)에 씬 배포                      |
| `optimize-scene`       | 성능 최적화, 씬 제한, 모범 사례                         |
| `camera-control`       | 카메라 모드 감지, 시네마틱 카메라, 가상 카메라                 |
| `composites`           | 정적 씬 콘텐츠용 복합 파일 형식 참고                       |
| `lighting-environment` | 동적 조명, 그림자, 스카이박스, 안개, 환경 설정                |
| `particle-system`      | 입자 효과 — 불, 연기, 불꽃, 눈, 불꽃놀이                  |
| `npcs`                 | 비플레이어 캐릭터 — NPC Toolkit 라이브러리와 수동 접근 방식     |
| `player-avatar`        | 플레이어 위치, 프로필, 아바타 커스터마이징, 부착물               |
| `player-physics`       | 물리력 — 임펄스, 넉백, 지속적 힘                        |
| `nft-blockchain`       | NFT 표시 및 블록체인/암호화폐 상호작용                     |
| `advanced-rendering`   | 빌보드, TextShape, PBR 재질, 비디오 재질              |
| `advanced-input`       | 시스템 수준 입력 폴링 및 플레이어 이동 제어                   |
| `scene-runtime`        | 교차 기능 런타임 API — 비동기 작업, HTTP, 메시징           |
| `script-components`    | Creator Hub용 스크립트 컴포넌트 클래스                  |
| `game-design`          | 게임 디자인 패턴, 씬 제한, 성능 예산                      |
| `unity-explorer-mcp`   | 실행 중인 Explorer를 조작해 씬을 인월드에서 테스트하고 검증하기     |

참고: 이 스킬들 중 일부는 무료 에셋 카탈로그에서 3D 모델이나 기타 에셋을 가져오는 작업을 포함합니다. AI 에이전트는 씬 프로젝트에 새 에셋을 다운로드하기 전에 항상 사용자 확인을 받아야 합니다.

## AI가 씬을 인월드에서 보게 하기

보통 AI는 코드를 작성하고 *당신이* 미리보기를 실행해 확인한 뒤, 무엇이 잘못됐는지 다시 알려줍니다. Decentraland 데스크톱 클라이언트는 이 과정을 닫아 주며, 선택 사항인 **MCP 서버** 를 포함하고 있어 AI 에이전트가 실행 중인 Explorer를 직접 보고 제어할 수 있습니다. 에이전트는 자체적으로 스크린샷을 찍고, 씬의 콘솔 출력을 읽고, 플레이어를 돌아다니게 하며, 오브젝트를 클릭하고, 씬이 실제로 빌드해 달라고 요청한 대로 동작하는지 확인한 다음, 발견한 문제를 수정하고 다시 확인합니다.

이렇게 하면 바이브 코딩이 "설명하고, 기다리고, 검토하는" 방식에서 에이전트가 대부분 스스로 실행할 수 있는 루프로 바뀝니다.


> [!NOTE]
> **💡 팁**: 다음 스킬을 설치하세요 `unity-explorer-mcp` 먼저 설치하세요. 이 스킬은 클라이언트 실행 방법, 유용한 스크린샷 프레이밍 방법, 보이는 내용과 씬의 실제 상태를 대조하는 방법, 씬 로딩이 멈췄을 때 복구하는 방법까지 전체 워크플로를 에이전트에게 가르쳐 줍니다.
> 
> ```bash
> npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
> ```
> 


### 필요한 것

* 다음이 필요합니다. **Decentraland 데스크톱 클라이언트** 가 설치되어 있어야 합니다( Creator Hub가 미리보기를 위해 실행하는 바로 그 클라이언트입니다).
* HTTP를 통해 MCP 서버에 연결할 수 있는 AI 에이전트 — Claude Code, Cursor, Cline, MCP 지원 확장 프로그램이 있는 VS Code 등.
* 씬에 최신 SDK가 있어야 합니다: 다음을 실행하세요 `npm i @dcl/sdk@latest` 아래의 `--mcp` 플래그가 알 수 없는 옵션으로 거부되면.

### 1. MCP 서버를 활성화한 상태로 씬 실행

씬 폴더에서:

```bash
npm run start -- --mcp
```

이것은 `npm run start` 가 항상 하는 일을 수행합니다 — 씬을 `http://127.0.0.1:8000` 에서 제공하고, 파일을 저장할 때마다 핫 리로드하며, 추가로 MCP 서버가 다음에서 대기하는 상태로 데스크톱 클라이언트를 실행합니다. `http://127.0.0.1:8123/unity-explorer-mcp`.

클라이언트가 열리면 로그인하세요. 에이전트는 로그인 화면을 통과하고 월드가 로드된 뒤에만 작업을 시작할 수 있습니다.

### 2. AI 에이전트를 서버에 연결하기

에서 **Claude Code**에 한 번만 등록하세요:

```bash
claude mcp add --transport http --scope user explorer http://127.0.0.1:8123/unity-explorer-mcp
```

에서 **다른 MCP 클라이언트에서는**다음 세부 정보를 사용해 해당 클라이언트 문서에 나온 방법으로 서버를 추가하세요:

| 설정    | 값                                                                 |
| ----- | ----------------------------------------------------------------- |
| 전송 방식 | 스트리밍 가능한 HTTP(stdio 아님 — 실행할 명령이 없으며, 서버는 실행 중인 클라이언트 내부에서 동작합니다) |
| URL   | `http://127.0.0.1:8123/unity-explorer-mcp`                        |
| 인증    | 없음                                                                |
| 이름    | `explorer`                                                        |

많은 클라이언트는 이를 위해 JSON 구성 파일을 사용합니다(`.cursor/mcp.json`, `mcp.json`및 유사한 파일들 — 정확한 키 이름은 클라이언트 문서를 확인하세요):

```json
{
  "mcpServers": {
    "explorer": {
      "type": "http",
      "url": "http://127.0.0.1:8123/unity-explorer-mcp"
    }
  }
}
```

서버를 등록한 후에는 AI 클라이언트를 다시 시작하거나 다시 불러와 연결을 인식하게 하세요. 에이전트가 Explorer 도구를 사용할 수 없다고 말한다면, 보통 원인은 에이전트가 시작될 때 클라이언트가 실행 중이 아니었다는 점입니다 — Explorer가 열려 있는 상태에서 서버를 다시 연결하세요(Claude Code에서는 `/mcp`를 실행).


> [!NOTE]
> **안전한가요?** 이 서버는 클라이언트를 `--mcp`와 함께 실행했을 때만 동작하고, 자기 기기에서 오는 연결만 허용하며(`127.0.0.1`127.0.0.1


### 웹 페이지에서 오는 요청은 거부합니다. 인터넷에 노출되는 것은 없으며, 일반적인 클라이언트 실행에서는 완전히 꺼져 있습니다.

서버가 연결되면 평소처럼 작업을 요청하세요 — 차이점은 이제 에이전트가 자신의 결과를 직접 확인할 수 있다는 점입니다:

> "클릭하면 열리는 보물상자를 parcel 중앙에 추가한 다음, 직접 걸어가서 클릭해 뚜껑 애니메이션이 재생되는지 확인해줘."

> "네온사인이 너무 어두워 보여. 스크린샷을 찍고, 발광 강도를 조정한 다음, 전/후를 보여줘."

> "엘리베이터에 문제가 있는 것 같아. 플랫폼 위로 걸어가서 로그를 확인하고, 왜 움직이지 않는지 알려줘."

내부적으로 에이전트는 다음을 할 수 있습니다:

* **보기** — 스크린샷을 찍고, 씬의 `console.log` 출력과 오류를 읽고, 씬이 로드되었는지 또는 충돌했는지 확인하며, 씬의 엔티티 목록을 가져와 컴포넌트를 검사하고, 플레이어와 카메라 위치를 읽을 수 있습니다.
* **제어** — 플레이어를 이동하고 텔레포트하며, 실제 충돌을 통해 특정 방향으로 걸어가고, 카메라를 조준하고, 특정 샷을 위해 자유 카메라를 배치하고, 카메라 모드를 전환하고, 씬 오브젝트를 클릭하고, 채팅 메시지와 `/commands`를 보내고, 감정 표현을 트리거하고, 씬을 다시 불러올 수 있습니다.
* **측정** — Decentraland의 제한과 비교해 씬 콘텐츠의 실시간 수치(삼각형, 엔티티, 텍스처, 재질 등)를 읽고, 어느 시점에서 어떤 3D 모델이 가장 큰 부담을 주는지 순위를 매기며, 플레이어가 어떤 위치에서 실제로 경험하는 프레임률을 샘플링할 수 있습니다.

### 이를 사용해 씬을 최적화하세요

에이전트는 보기와 측정을 모두 할 수 있으므로, 성능 작업을 직접 맡길 수 있습니다. 씬을 다음 기준과 비교할 수 있고 [씬 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/)을 확인하며, 어떤 3D 모델이 가장 무거운지 식별하고, 특정 지점에서 프레임률을 측정하며, 무엇보다 변경 후 다시 측정해 실제로 도움이 되었음을 증명할 수 있습니다:

> "내 씬이 Decentraland의 콘텐츠 제한 내에 있는지 확인하고, 어떤 예산 항목이 상한에 가장 가까운지 알려줘."

> "분수 근처에서 씬이 끊기는 것 같아. 거기 서서 프레임률을 측정하고, 그 위치에서 어떤 모델이 가장 많은 비용을 차지하는지 알려줘."

> "씬을 최적화해줘: 프레임률을 떨어뜨리는 원인을 찾아 수정하고, 전/후 측정값을 보여줘."

가장 좋은 결과를 위해 `optimize-scene` 스킬을 `unity-explorer-mcp`과 함께 설치하세요. 그러면 에이전트가 Decentraland의 최적화 모범 사례도 알게 됩니다. 3D 모델 자체를 자동으로 수정하는 방법까지 포함한 이 워크플로는 [성능 최적화](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/#optimize-with-ai) 에서 자세히 볼 수 있으며, Blender MCP 서버와 함께 사용해 3D 모델 자체의 수정을 자동화하는 방법도 설명합니다.

### 팁

* **주장이 아니라 증거를 요청하세요.** "스크린샷으로 확인해줘" 또는 "로그에서 확인해줘"가 이 워크플로의 핵심입니다. 좋은 에이전트는 둘 다 교차 확인합니다. 픽셀은 멀쩡해 보여도 내부 상태는 깨져 있을 수 있고, 그 반대도 마찬가지입니다.
* **스크린샷은 토큰을 소모합니다.** 에이전트가 보는 각 스크린샷은 컨텍스트의 일부를 소모합니다. 긴 시각 점검 — 시간에 따른 애니메이션, 여러 장소를 둘러보는 작업 — 을 원한다면, 파일로 프레임을 저장하게 하고 중요한 것만 읽게 하세요. The `unity-explorer-mcp` 스킬에는 이를 정확히 수행하는 스크립트가 포함되어 있습니다.
* **한 번만 저장하세요, 연달아 다섯 번 저장하지 마세요.** 빠르게 연속 저장하면 클라이언트가 반쯤 작성된 번들을 불러와 씬 전체를 놓칠 수 있으며, 복구하려면 클라이언트를 재시작해야 합니다. 에이전트에게 편집 내용을 하나의 저장으로 묶도록 요청하세요.
* **클라이언트를 열어 두세요.** 닫으면 연결이 끊기고 에이전트는 눈을 잃게 됩니다. 같은 명령으로 다시 실행하면 돌아옵니다.
* **로컬 씬 개발에서는 텔레포트가 다르게 동작합니다.** 다음을 사용해 parcel 간 이동하는 것은 `/goto` 허용되지 않으므로, 에이전트는 텔레포트 대신 씬 내부에서 플레이어 위치를 재배치해야 합니다.

## 효과적인 프롬프트 작성 팁

AI에서 최상의 결과를 얻으려면 명확하고 구체적인 프롬프트를 주는 것이 중요합니다. 몇 가지 팁을 소개합니다:

### 원하는 것을 구체적으로 말하세요

대신:

> "내 씬을 더 좋게 만들어줘"

이렇게 해보세요:

> "(8, 0, 8) 위치에 문을 추가하고, 클릭하면 회전 애니메이션으로 열리며 삐걱거리는 효과음을 재생하게 해줘"

### 기존 항목을 참조하세요

> "테이블 위 빨간 버튼이 엘리베이터를 위로 올라가게 트리거하도록 해줘"

### 한 번에 한 가지씩 요청하세요

복잡한 요청은 단계로 나누세요:

1. "오른쪽 상단에 점수판 UI를 추가해줘"
2. "플레이어가 대상을 클릭할 때 증가하는 카운터를 추가해줘"
3. "점수판에 카운터 값을 표시해줘"

### 반복하며 다듬기

각 변경 후:

1. 씬 미리보기( Creator Hub에서 Play를 클릭하거나 명령줄에서 실행) **Play** 를 클릭하거나 `npm run start` 명령줄에서
2. 무엇이 잘 작동하고 무엇이 안 되는지 확인하세요
3. AI에게 무엇을 조정할지 알려주세요: "NPC를 왼쪽으로 2미터 이동시키고 플레이어를 바라보게 해줘"

### 예시 프롬프트

새로 클론한 상태에서 실행할 대상 [sdk7-scene-template](https://github.com/decentraland/sdk7-scene-template/).

#### 프롬프트 예시 1

```
현재 씬 코드를 전부 없애고 작은 단순한 미로 게임을 만들어줘. 1개의 parcel 안에서, 벽은 큐브로 만들 수 있고 충돌이 있어야 해.

게임은 플레이어가 실제로 A 지점에서 미로에 들어갈 때 시작하고, B 지점(미로의 유일한 출구)에서 나올 때 끝나야 해.

미로 안에 서로 다른 3개의 문을 추가하고, 포인팅 입력으로 상호작용해야 열리고 지나갈 수 있게 해줘.

플레이어가 실제로 플레이하고 승리할 수 있는지 확인해야 해.
```

#### 프롬프트 예시 2

```
현재 씬 코드를 전부 없애고 간단한 플랫폼 게임을 만들어줘.

완료로 간주하기 전에 게임을 클리어할 수 있는지 확인해야 해.
```

## AI가 도와줄 수 있는 것

* 설명을 바탕으로 새 씬 스캐폴딩하기
* 3D 모델 추가 및 배치
* 클릭 핸들러 및 상호작용 작성
* UI 구축(HUD, 메뉴, 대화 상자)
* 멀티플레이어 동기화 설정
* 치트 방지를 위한 멀티플레이어 서버 구성
* 오디오, 비디오, 스트리밍 추가
* 애니메이션 및 tween 만들기
* 씬 성능 최적화
* 배포를 위한 씬 준비
* 기존 코드의 문제 디버깅
* 실행 중인 Explorer에서 씬을 테스트하고 시각적으로 검증하기(참조 [AI가 씬을 인월드에서 보게 하기](#let-the-ai-see-your-scene-in-world))

## 제한 사항

AI 도구는 강력하지만, 다음 사항을 염두에 두세요:

* **항상 미리보기** — AI가 생성한 코드는 예상과 정확히 같지 않을 수 있습니다. 미리보기를 실행해 확인하세요.
* **씬 제한은 여전히 적용됩니다** — AI는 Decentraland의 [씬 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) (삼각형 수, 파일 크기, parcel 경계)를 우회할 수 없습니다.
* **복잡한 게임 로직** — 정교한 게임 메커니즘의 경우 AI를 단계별로 안내하거나 결과를 수동으로 다듬어야 할 수 있습니다.
* **맞춤 3D 모델** — AI는 기존 무료 에셋을 참조하거나 제공한 모델을 불러올 수는 있지만, 3D 모델을 처음부터 생성할 수는 없습니다(Blender 공식 MCP 서버와 같은 다른 도구를 동시에 사용하지 않는 한).

## 다음 단계

* [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/) — SDK7 기초 배우기
* [코드와 결합하기](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) — 시각적 편집과 코드를 함께 사용하기
* [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) — 서버 권한 기반 멀티플레이어
* [씬 예시](/docs/creator/content-creator-ko/tutorials-and-examples/examples/) — 재사용 가능한 씬 메커니즘과 저장소 살펴보기
* [유용한 자료](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) — 워크플로를 더 빠르게 해 주는 추가 AI 도구, 에셋 라이브러리, 애드온


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/vibe-coding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
