# 스냅샷

콘텐츠 서버는 자신이 호스팅하는 활성 엔터티들의 요약을 주기적으로 생성하며, 이를 *스냅샷*. 이는 일반적인 [파일이며](/docs/contributor/contributor-ko/content/file-system/) 식별자를 사용해 다운로드할 수 있습니다.

스냅샷은 일별, 주별, 월별, 연별로 생성됩니다. 각 스냅샷에는 해당 기간에서 이전 스냅샷 이후 변경된 활성 엔터티 집합이 포함됩니다.

스냅샷에는 동일한 엔터티의 충돌하는 버전(즉, 서로 다른 [매니페스트 파일](/docs/contributor/contributor-ko/content/entities/#properties) 가 같은 포인터에 연결될 수 있으며, 업데이트되면서 서로 충돌할 수 있습니다. 이를 스캔할 때 클라이언트는 가장 최근 스냅샷의 버전을 유지해야 합니다. 콘텐츠 서버는 비활성 파일을 삭제할 수 있으므로, 오래된 엔터티 버전이 다운로드 가능하다고 보장되지 않습니다.

새 스냅샷이 *대체할 때* 이전 스냅샷들(예: 여러 일별 스냅샷을 합친 주별 스냅샷)을 대체하면, 해당 메타데이터는 어떤 이전 파일이 대체되었는지를 알려 주므로 클라이언트가 이를 다운로드할 필요가 없습니다.

활성 엔터티의 전체 집합은 사용 가능한 모든 스냅샷을 결합하여(아래에서 더 설명함), 그 과정에서 각 [포인터](/docs/contributor/contributor-ko/content/pointers/) 를 따라 발견된 것 중 가장 최신 엔터티를 유지함으로써 알아낼 수 있습니다.

스냅샷은 practice 섹션의 작동하는 코드로 실험해 볼 수 있습니다. [실습](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 섹션입니다.

### 스냅샷 찾기 <a href="#discover" id="discover"></a>

현재 스냅샷 집합을 찾으려면 [`스냅샷` 엔드포인트를](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots)사용하세요. 응답에는 다음 필드를 가진 항목 배열이 포함됩니다:

| 필드                        | 값                                                             |
| ------------------------- | ------------------------------------------------------------- |
| `generationTimestamp`     | 이 스냅샷이 생성된 시점의 Unix UTC 타임스탬프입니다.                             |
| `hash`                    | 스냅샷 [파일](/docs/contributor/contributor-ko/content/file-system/). |
| `numberOfEntities`        | 스냅샷 파일의 항목 수입니다.                                              |
| `replacedSnapshotHashes`  | 해시가 들어 있는 배열 `hash` 이것으로 대체된 모든 스냅샷의                          |
| `timeRange.initTimestamp` | 스냅샷 범위의 시작 시점을 나타내는 Unix UTC 타임스탬프(밀리초)입니다.                   |
| `timerange.endTimestamp`  | 스냅샷 범위의 종료 시점을 나타내는 Unix UTC 타임스탬프(밀리초)입니다.                   |

예를 들면:

```json
{
  "generationTimestamp": 1684979298844,
  "hash": "bafybeiflmm46nr4vv2h3wuzbx3pukcz7ju4fhbfzt6yxmoo533uktlgru4",
  "numberOfEntities": 12345,
  "replacedSnapshotHashes": [ "bafybeicw6x75ieaxfwynekbyhpcsgctpjkt6cb4j6oa7s57qjj6e4b5phd" ],
  "timeRange": {
     "initTimestamp": 1684281600000,
     "endTimestamp": 1684886400000
  }
}
```

### 스냅샷 다운로드 <a href="#download" id="download"></a>

다음을 사용하면 `hash` 스냅샷의 필드를 사용해 클라이언트는 해당 시간 범위에 생성되거나 업데이트된 관련 엔터티를 다운로드할 수 있습니다.

스냅샷 파일은 다음 정확한 줄로 시작합니다:

```
### Decentraland JSON 스냅샷
```

그다음 각 줄은 다음을 설명하는 JSON 문서입니다: [엔터티](/docs/contributor/contributor-ko/content/entities/) 그리고 다음 필드를 가집니다:

| 필드                | 값                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------ |
| `entityId`        | 이것의 변경 불가능한 식별자입니다. [엔터티](/docs/contributor/contributor-ko/content/entities/).                |
| `entityType`      | 다음 중 하나: `씬`, `프로필`, `웨어러블`, `이모트`, `스토어` 또는 `아웃핏`.                                        |
| `포인터`             | 다음의 배열 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 엔터티를 가리키는(또는 가리키던) 항목들입니다. |
| `entityTimestamp` | 이 엔터티가 업로드된 시점의 Unix UTC 타임스탬프(밀리초)입니다.                                                    |
| `authChain`       | 이 [인증 체인](/docs/contributor/contributor-ko/content/entities/#ownership) 이 엔터티에 대한 것입니다.       |

일반적인 항목은 다음과 같습니다:

```json
{
  "entityId": "bafkreigrvaqynmiglpvewwhn2yd63q5dvagrrt5jbhimzvbrn5kimj5zne",
  "entityType": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632a4e7226816a16139fb33:0"],
  "entityTimestamp": 1671117456129,
  "authChain": [
    // ... authentication chain payloads and signatures
  ]
}
```


> [!NOTE]
> 스냅샷을 줄 단위로 파싱하려면, 첫 번째 헤더 줄은 건너뛰거나(더 좋게는) 검증해야 하며, 파일 끝의 빈 줄도 처리할 준비를 해 두어야 합니다.


#### 엔터티 인덱스 시작하기 <a href="#index-start" id="index-start"></a>

활성 엔터티 전체 집합을 인덱싱하려는 클라이언트는 현재 사용 가능한 모든 스냅샷을 처리하고, 가장 최신의 [엔터티](/docs/contributor/contributor-ko/content/entities/) 각 [포인터](/docs/contributor/contributor-ko/content/pointers/).

가장 간단한 전략은 스냅샷을 역시간순(즉, 최신 순서부터)으로 처리하면서 이미 발견된 포인터는 무시해 최신 엔터티에 대한 참조를 유지하는 것입니다.

의사코드로는:

```py
# 현재 스냅샷 집합을 다운로드하고, 최신에서 오래된 순으로 정렬합니다:
snapshots = get_snapshots()
snapshots.sort('timeRange.initTimestamp', DESCENDING)

seen_pointers = set()

# 각 포인터에 대해 가장 최신 엔터티를 유지하며 스냅샷을 처리합니다:
for snapshot in snapshots:
    items = get_snapshot_items(snapshot) 

    for item in items:
        if any(pointer in seen_pointers for pointer in item.pointers):
            discard(item)
        else:
            keep(item)
            seen_pointers.update(item.pointers)
```

개별 엔터티는 여러 포인터로 참조될 수 있으므로(예를 들어 [씬](/docs/contributor/contributor-ko/content/entity-types/scenes/)가 흔한 경우), 항목을 유지할지 버릴지 결정하기 전에 모두 확인해야 합니다.


> [!NOTE]
> 더 긴 시간 범위의 스냅샷 파일은 매우 클 수 있습니다. 전체 엔터티 집합을 인덱싱할 필요가 없는 개발 및 실험 목적이라면 더 작은 스냅샷을 사용하는 것이 좋습니다. 그렇게 얻은 엔터티 집합은 불완전하지만 유효합니다.


#### 엔터티 인덱스 업데이트하기 <a href="#index-update" id="index-update"></a>

최신 엔터티 인덱스를 유지하는 클라이언트는 [`스냅샷`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots) 엔드포인트에 주기적으로 요청하고, 다음을 고려해 각 파일을 다운로드할지 판단할 수 있습니다:

* 스냅샷이 `hash` 이미 다운로드되었습니까?
* 다음이 `hash` 안에 `replacedSnapshotHashes` 이미 다운로드된 다른 스냅샷의
* 다음이 `timeRange가` 현재 목적에 적합합니까?

새 스냅샷을 처리해야 한다면, 위와 같은 전략으로 기존 데이터셋을 업데이트할 수 있습니다.

### 예시

다음 [실습](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 섹션에서 스냅샷 시스템과 함께 작동하는 코드 예제를 찾을 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/snapshots.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
