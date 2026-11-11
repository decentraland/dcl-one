> 지속적으로 저장되는 멀티플레이어 서버 로그를 검색하고, 실시간 로그를 추적하며, 씬 성능 지표를 확인하세요.

# Scene Insights

Scene Insights를 사용하면 배포된 [멀티플레이어 서버](/docs/creator/content-creator-ko/sdk7/networking/authoritative-servers/) 씬을 조사할 수 있습니다. 과거 서버 로그를 검색하고, 새 메시지를 실시간으로 따라가며, 터미널이나 AI 어시스턴트에서 CPU, 메모리 및 기타 메트릭을 가져오세요.

**로그와 메트릭은 7일 동안 보관됩니다**, 서버 재시작과 씬 재배포를 거쳐 유지됩니다. 이는 서버 측 실행을 포함합니다. 다음을 사용하세요 [프로덕션에서 디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-prod/) 클라이언트 측 및 렌더링 문제에는 이를 사용하세요.

## 프로젝트 업데이트

최신 버전을 설치하려면 씬 프로젝트에서 다음 명령을 실행하세요 `auth-server` SDK 및 런타임:

```bash
npm install @dcl/sdk@auth-server @dcl/js-runtime@auth-server
```

프로젝트에 다음이 함께 나열되어 있다면 `@dcl/sdk-commands` 직접 `package.json`, 다음으로 업데이트하세요 `@auth-server` 로도 업데이트하세요. 설치된 CLI에 다음이 포함되어 있는지 확인하세요 `login`, `logs`, `tail`, `metrics`와 `executions`:

```bash
npx sdk-commands scene-insights --help
```

AI 지원 디버깅을 위해 다음도 설치하거나 갱신하세요 [SDK Skills](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/):

```bash
npx skills add decentraland/sdk-skills --all
```

AI 동반자에게 설정을 맡길 수 있습니다:

> 이 씬을 최신 auth-server SDK 및 런타임으로 업데이트하고, Decentraland SDK Skills도 업데이트하세요. Scene Insights에 로그인하도록 도와준 다음, 지난 2시간의 서버 오류와 CPU 및 메모리 사용량을 조사하세요.

이미 배포된 씬의 텔레메트리를 읽는 데는 재배포가 필요하지 않습니다. 권한 변경은 필요합니다.

## 누가 데이터를 읽을 수 있나요?

다음 조건 중 하나를 만족하는 월렛으로 로그인하세요:

* 배포된 씬의 최상위 수준에 있는 `logsPermissions` 배열에 포함되어 있습니다.
* 해당 월드의 소유자입니다.
* 부여 범위가 전체 월드 또는 씬의 배포된 모든 parcel을 포함하는 월드 배포 협업자입니다.
* Genesis City의 경우, 씬의 **기본 parcel**, 또는 해당 parcel에 대해 위임된 LAND 관리 권한이 있습니다: `operator`, `updateOperator`, `updateManager`, 또는 `approvedForAll`.

소유자와 적격 관리자는 다음이 필요하지 않습니다 `logsPermissions` 항목이나 씬 재배포가 필요하지 않습니다. 다른 월렛에 명시적으로 접근 권한을 부여하려면 다음에 추가하세요 `scene.json` 그리고 재배포하세요:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

Genesis City의 경우, 다른 parcel을 쿼리하더라도 권한은 배포된 씬의 base parcel을 기준으로 확인됩니다. 다른 parcel에 대한 소유권이나 관리 권한만으로는 씬 접근 권한이 부여되지 않습니다.

동일한 권한이 과거 로그, 실시간 로그, 메트릭, 실행 기록을 모두 포함합니다. 접근은 **현재 권한**: 현재 승인된 사람은 누구나 동일한 씬 키 아래 보관된 모든 기록을 최대 7일 동안 읽을 수 있습니다. 여기에는 다른 사람이 씬을 배포하는 동안 기록된 로그도 포함됩니다. 여기에는 새 소유자, operator, 적격 tenant, 그리고 새로 추가된 월렛도 포함됩니다 `logsPermissions`. 접근 권한을 부여하기 전에 그 기록을 고려하세요.

권한 변경은 즉시 반영되지 않습니다. 기본적으로 권한 캐시는 60초 동안 유지되며, 열려 있는 실시간 스트림은 5분마다 접근 권한을 다시 확인합니다. 열려 있는 스트림이 권한 회수를 감지하려면 약 6분 정도 기다리세요. 이는 기본값일 뿐 엄격한 마감 시간은 아닙니다. 구성 및 일시적인 용량 실패 중의 제한된 유예 기간으로 인해 지연이 더 길어질 수 있습니다. 로그인한다고 해서 다른 씬을 읽을 권한이 부여되지는 않습니다.

## 로그인

```bash
npx sdk-commands scene-insights login
```

일반 월렛 또는 스마트 컨트랙트 월렛으로 브라우저에서 로그인을 승인하세요. 그러면 로그, 메트릭, 실행만 읽을 수 있는 Insights 세션이 생성됩니다. 세션은 최대 **12시간**, 상위 세션의 만료 시각에 의해 제한됩니다. 이후 명령은 다시 승인하지 않아도 이를 재사용합니다.

다음으로 로그인한 주소와 만료 시간을 확인하세요:

```bash
npx sdk-commands scene-insights session --json
```

세션이 만료되면 다음을 실행하세요 `login` 다시. AI 어시스턴트가 대신 조사 중이라면, 월렛 로그인은 직접 승인하세요. 개인 키나 캐시된 세션 파일은 그것과 공유하지 마세요.

### 세션 파일은 비공개로 유지하세요


> [!WARNING]
> **로그인 세션 파일을 절대 커밋, 배포 또는 공유하지 마세요.** 기본적으로 다음 위치에 저장됩니다 `~/.dcl/scene-insights-session.json`여기에는 임시 개인 서명 키와 월렛의 읽기 전용 위임이 포함됩니다. 이를 복사한 사람은 세션이 만료될 때까지 해당 자격 증명을 사용해 월렛이 접근할 수 있는 씬의 로그, 메트릭 및 실행 기록을 읽을 수 있습니다. 이를 팀원, AI 어시스턴트, 지원 채팅 또는 공개 이슈 보고에 보내거나 프로젝트 아카이브에 포함하지 마세요.


SDK가 이를 자동으로 보호합니다:

* 기본적으로 파일은 씬 프로젝트 밖의 홈 디렉터리 아래에 저장됩니다. 다음을 설정하면 `DCL_SCENE_INSIGHTS_SESSION_DIR`, 해당 디렉터리의 `.dcl` 폴더를 대신 사용합니다.
* 다음을 추가하고 `*` 세션 폴더의 `.gitignore` Git이 이미 추적 중인 세션 파일은 저장하거나 사용하지 않습니다.
* 세션 파일은 소유자 전용 권한(`0600`)으로 기록하며 폴더는 다음 권한으로 생성합니다 `0700` 해당 권한을 지원하는 시스템에서.
* 표준 SDK 배포 파일 수집기는 점으로 시작하는 파일과 디렉터리를 제외합니다. 여기에는 `.dcl`, 다음과는 별개로 `.dclignore`.

다음 확인 사항을 기억하세요:

* 커밋하기 전에 스테이징된 파일을 검토하세요. 세션 파일을 강제로 추가하지 마세요; `.gitignore` 이는 Git이 이미 추적 중인 파일은 보호하지 않습니다.
* 세션 위치를 사용자 지정하는 경우, 가능한 한 씬 프로젝트 밖에 두세요. 세션을 씬 자산이나 다른 배포 가능한 폴더로 복사하거나 이름을 바꾸지 마세요.
* 사용자 지정 배포, 백업 또는 ZIP 스크립트를 사용하는 경우 세션 파일과 `.dcl` 자격 증명을 명시적으로 제외하세요. 이러한 도구는 SDK의 제외 규칙을 적용하지 않을 수 있습니다.
* 다음에서 나온 안전한 상태 출력만 공유하세요 `npx sdk-commands scene-insights session --json` 필요할 때만, 세션 파일 자체는 절대 공유하지 마세요. 팀원들은 자신이 승인된 월렛으로 로그인해야 합니다.

`npx sdk-commands scene-insights logout` 로컬에 저장된 세션을 삭제합니다. 그러나 **하지 않습니다** 이미 공유된 복사본은 무효화하지 않습니다. 해당 자격 증명은 만료될 때까지 계속 사용할 수 있습니다. 파일이 노출되었다면, 해당되는 경우 공유된 산출물과 Git 기록에서 제거하세요. 로컬 파일을 삭제하거나 다시 로그인해도 노출된 복사본은 무효화되지 않습니다.

## 씬 선택

씬 프로젝트에서 명령은 다음에서 월드를 추론합니다 `scene.json`의 `worldConfiguration.name`, 또는 다음을 사용하세요 `scene.base` Genesis City의 경우. 또한 명시적으로 선택할 수도 있습니다:

```bash
# 월드
npx sdk-commands scene-insights logs --world myworld.dcl.eth

# 여러 씬으로 구성된 월드의 한 씬
npx sdk-commands scene-insights logs --world myworld.dcl.eth --position 10,10

# Genesis City 씬: 좌표가 음수일 때는 =를 사용하세요
npx sdk-commands scene-insights logs --position=-125,-96
```

명령은 기본적으로 프로덕션(`.org`). 개발 네트워크의 씬에는 다음을 추가하세요 `--target https://multiplayer-scenes-insights-service.decentraland.zone` 를 로그인, 세션 확인 및 쿼리에 사용하세요. 전체 과정에서 동일한 대상(target)을 사용하면 스마트 컨트랙트 월렛이 올바른 네트워크에서 검증됩니다.

## 로그와 실행 검색

```bash
# 지난 6시간의 최신 오류 200개
npx sdk-commands scene-insights logs --from 6h --level error --limit 200

# 고정된 UTC 시간 창에서 텍스트 찾기
npx sdk-commands scene-insights logs --from 2026-11-01T14:00:00Z --to 2026-11-01T14:30:00Z --query "player joined"

# 지난 하루 동안의 서버 실행 목록
npx sdk-commands scene-insights executions --from 1d

# 한 실행의 로그: EXECUTION_ID를 executions의 ID로 바꾸세요
npx sdk-commands scene-insights logs --execution EXECUTION_ID --from 1d
```

`logs` 기본값은 지난 1시간과 가장 최신의 일치하는 100개 줄이며, 시간순으로 표시됩니다. CLI는 다음을 허용합니다 `--limit` 1에서 10,000 사이이며 필요에 따라 여러 API 페이지를 가져옵니다. `--level` 최소 심각도입니다 (`warn` 에는 오류가 포함됩니다); `--query` 대소문자를 구분하지 않는 ASCII 일치로 리터럴 텍스트를 검색합니다. 다음을 사용하세요 `--source scene,engine,stderr` 로그 출처를 선택하거나, 이 출처들의 일부만 전달하세요.

`executionId` 는 서버 실행을 식별합니다; `entityId` 는 배포된 씬 버전을 식별합니다. 새 실행은 유휴 상태의 중지/시작, 롤아웃 또는 크래시를 의미할 수 있습니다. 크래시라고 결론 내리기 전에 주변 로그를 확인하세요.

다음을 추가하세요 `--json` 기계 판독 가능한 출력을 위해. CLI는 과거 로그를 하나의 JSON 문서로 반환하며, 그 `more: true` 필드는 반환 제한을 넘는 추가 일치 항목이 있음을 의미합니다. 범위를 좁히거나 한도를 늘리세요. 직접 API 클라이언트는 페이지당 최대 1,000행을 받으며, 페이지네이션에는 커서를 사용합니다.

## 실시간 로그 따라가기

```bash
npx sdk-commands scene-insights tail
```

누르세요 **Ctrl+C** 로 중지합니다. AI 어시스턴트에게 조사를 요청할 때를 포함해, 제한된 캡처를 하려면:

```bash
npx sdk-commands scene-insights tail --for 30s --lines 20 --json
```

이 명령은 가장 최신 20줄을 다시 재생한 다음 30초 동안 새 줄을 따라갑니다. JSON 출력에는 한 줄당 하나의 로그 항목이 포함됩니다. CLI는 실행 중인 tail 동안 재연결하고 이어서 진행합니다. 로그는 보통 씬이 기록한 뒤 몇 초 후에 도착합니다.

`tail` 는 수준 및 텍스트 필터를 지원합니다. 과거 시간 범위나 실행/출처 필터에는 다음을 사용하세요 `logs`.

다음을 사용하는 경우 `npm run server-logs`, 프로젝트의 `package.json` 에 다음이 포함되어 있는지 확인하세요:

```json
{
  "scripts": {
    "server-logs": "sdk-commands scene-insights tail"
  }
}
```

업데이트된 SDK로 전체 프로젝트를 빌드하면 SDK가 생성한 기본 스크립트를 만들거나 마이그레이션합니다. 사용자 지정 스크립트는 보존되며, 다음을 사용한 미리보기는 `--skip-build` 및 단일 파일 빌드는 이를 업데이트하지 않습니다. 로그를 확인하기 위해서라면 재빌드하지 않고도 스크립트를 직접 편집할 수 있습니다.

## 성능 메트릭 검사

먼저 데이터가 있는 메트릭을 나열한 다음 시리즈를 쿼리하세요:

```bash
npx sdk-commands scene-insights metrics --json
npx sdk-commands scene-insights metrics cpu.percent --from 2h --step 60 --json
npx sdk-commands scene-insights metrics mem.heap_used --from 2h --step 60 --json
```

`--step` 는 버킷 크기를 초 단위로 설정합니다. 사용 가능한 메트릭은 엔진 버전과 씬 활동에 따라 달라집니다. 메트릭 검색과 `executions` 의 기본값은 지난 24시간입니다. 다음을 사용하세요 `--execution EXECUTION_ID` 를 메트릭 시리즈에 적용해 특정 실행만 분리하세요.

| 메트릭                                                     | 의미                                                                          |
| ------------------------------------------------------- | --------------------------------------------------------------------------- |
| `cpu.percent`                                           | 실행 스레드의 CPU 사용률을 한 코어 대비 퍼센트로 나타낸 값입니다. 100%에 가까운 값이 지속되면 해당 스레드가 바쁘다는 뜻입니다 |
| `cpu.run_ms`                                            | 다음에 사용된 CPU 시간 `onStart`/`onUpdate`, 벽시계 지연 시간이 아닙니다                        |
| `mem.heap_used`, `mem.heap_limit`, `mem.external_bytes` | JavaScript 힙 사용량, 힙 한도, 그리고 V8 외부 메모리                                       |
| `logs.lines`, `logs.bytes`                              | 레이트 제한 전의 콘솔 출력입니다. 과도한 로깅을 발견하는 데 유용합니다                                    |
| `fetch.failed`, `storage.failed`                        | 외부 HTTP 또는 저장소 호출에 대한 요청 전송 오류입니다. 오류 로그와 대조하세요                             |
| `fetch.status_4xx`, `fetch.status_5xx`                  | HTTP 오류 응답입니다. 이는 다음으로 집계되지 않습니다 `fetch.failed`                             |
| `logs.dropped`                                          | 엔진의 틱당 로그 예산 때문에 버려진 줄                                                      |
| `comms.msgs_out`, `comms.bytes_out`                     | 플레이어에게 전송된 메시지와 바이트                                                         |

CPU 메트릭은 전체 서버를 측정하지 않으며, 메모리 메트릭은 프로세스의 총 메모리나 컨테이너 한도가 아닙니다. 누수나 성능 문제를 진단하기 전에 동일한 실행, 시간 범위, 워크로드를 비교하세요.

게이지는 반환합니다 `avg` 및 `max` 를 버킷당 반환합니다. 합계는 반환합니다 `sum` 및 초당 `rate`. `null` 포인트는 저장된 샘플이 없음을 의미하며, 사용량이 0이라는 뜻은 아닙니다. 유휴 씬, 지연, 수집 실패 모두 공백을 만들 수 있습니다.

## 보관 및 제한

| 제한              | 기본값                                                     |
| --------------- | ------------------------------------------------------- |
| 로그 및 메트릭 보관     | 7일                                                      |
| 최대 쿼리 시간 범위     | 7일                                                      |
| CLI 쿼리당 로그 결과 수 | 기본값은 100개이며, 페이지네이션으로 최대 10,000개까지 가능합니다                |
| 메트릭 시리즈         | 최대 1,500개 버킷; 증가시키세요 `--step` 더 긴 창에는                   |
| 지속적인 씬 로깅       | 초당 50줄 및 초당 64 KiB                                      |
| 로깅 버스트 허용량      | 500줄 및 512 KiB                                          |
| 일일 씬 로그 할당량     | 서버 프로세스당 500,000줄 또는 128 MiB의 소프트 상한; UTC 00:00에 재설정됩니다 |

일일 할당량 카운터는 메모리에 보관됩니다. 서버 프로세스 재시작이나 씬이 다른 복제본으로 이동해도 카운터가 재설정되므로, 일일 할당량은 이러한 변경 전반에 대한 대략적인 상한입니다.

레이트 제한과 일일 할당량은 초과 로그를 버립니다. 버려진 줄은 나중에 복구할 수 없습니다. 엔진 경고 메시지는 드롭과 할당량 재설정을 보고합니다. 일일 할당량에 도달하면 재설정될 때까지 추가 로그 저장이 중지되지만 씬은 계속 실행됩니다. 유용한 메시지가 예산 안에 들어가도록 프레임마다 반복되는 로깅을 줄이세요.

엔진에는 별도의 틱당 로그 예산(기본값 1,000줄)도 있습니다. 초과 줄은 내보내기 전에 버려지며 다음에 집계됩니다 `logs.dropped`.

큰 메시지는 잘릴 수 있습니다(API 기본값은 메시지당 8,192자). 수집은 최선 노력 방식입니다. 장애, 내보내기 실패, 레이트 제한으로 인해 공백이 생길 수 있습니다. 수집된 텔레메트리만 사용할 수 있으며, 새 CLI를 설치해도 더 오래된 데이터나 버려진 데이터를 복구할 수는 없습니다. 결과가 비어 있다고 해서 씬이 정상이라는 뜻은 아닙니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/debugging/scene-insights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
