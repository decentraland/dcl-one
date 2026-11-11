# 엔티티

세계의 각 개별 콘텐츠 조각(예: 씬 또는 착용 아이템)은 하나의 *엔티티*.

엔터티는 변경 불가능한 패키지입니다 [파일이며](/docs/contributor/contributor-ko/content/file-system/) 내포된 데이터로부터 결정적으로 도출된 고유한 문자열 식별자를 가지며, 이를 사용해 콘텐츠 서버에서 관련 파일을 찾아 다운로드할 수 있습니다.

엔터티의 주 파일은 *매니페스트*이며, 엔터티의 일반 속성과 각 [유형](#types). 엔터티의 식별자는 실제로 [파일 식별자](/docs/contributor/contributor-ko/content/file-system/#identifiers) 입니다.

변경 불가능하므로 엔터티는 전통적인 의미에서 업데이트할 수 없습니다. 대신 동일한 안정적인 [포인터](/docs/contributor/contributor-ko/content/pointers/). 엔터티의 최신 버전은 *활성*.

모든 엔터티는 소유자(이더리움 계정과 연결됨)에 의해 서명됩니다. 소유자는 나중에 동일한 서명 키를 사용하여 엔터티의 새 버전을 업로드하고, 그것이 이전 버전을 대체함을 나타낼 수 있습니다. 콘텐츠 서버는 클라이언트에서 직접 오든 다른 서버를 통해 전달되었든, 새 엔터티를 수락하기 전에 이러한 서명을 검증합니다.

실제로 배포된 엔터티는 [실습](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 섹션.

### 엔터티 유형 <a href="#types" id="types"></a>

엔터티에는 7가지 유형이 있습니다:

* [**씬**](/docs/contributor/contributor-ko/content/entity-types/scenes/): 세계 내의 가상 공간으로, 자체 객체와 동작을 가집니다.
* [**프로필**](/docs/contributor/contributor-ko/content/entity-types/profiles/): 이름과 아바타와 같은 특정 플레이어에 대한 정보입니다.
* [**착용 아이템**](/docs/contributor/contributor-ko/content/entity-types/wearables/): 플레이어가 자신의 아바타에 추가할 수 있는 의류와 아이템입니다.
* [**이모트**](/docs/contributor/contributor-ko/content/entity-types/emotes/): 플레이어의 아바타가 수행할 수 있는 애니메이션입니다.
* [**상점**](/docs/contributor/contributor-ko/content/entity-types/stores/): 플레이어가 구매할 수 있는 착용 아이템과 이모트의 마켓플레이스 사이트입니다.
* [**코디**](/docs/contributor/contributor-ko/content/entity-types/outfits/): 특정 플레이어를 위한 저장된 코디입니다.

모든 유형은 생성, 식별, 소유권, 호스팅에 대해 동일한 절차를 따릅니다.

### 공통 속성 <a href="#properties" id="properties"></a>

모든 엔터티는 매니페스트에 모든 유형에 적용되는 몇 가지 공통 속성을 가집니다. 다음 최상위 필드는 항상 존재합니다:

| 필드      | 값                                                                           |
| ------- | --------------------------------------------------------------------------- |
| `유형`    | 다음 중 하나: `씬`, `프로필`, `웨어러블`, `이모트`, `스토어` 또는 `아웃핏`.                         |
| `포인터`   | 다음의 배열 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 엔터티와 연결됩니다. |
| `타임스탬프` | 이 엔터티가 업로드된 Unix UTC 타임스탬프입니다.                                              |
| `콘텐츠`   | 추가 [파일이며](/docs/contributor/contributor-ko/content/file-system/) 엔터티 패키지 안에 있는 |
| `메타데이터` | 이 엔터티 유형에 특화된 정보를 담은 객체입니다.                                                 |

구조와 값은 `메타데이터` 각 유형의 필드는 해당 페이지에서 자세히 설명합니다.  `포인터` 배열 역시 유형에 따라 서로 다른 내용을 가집니다.


> [!NOTE]
> 오래된 엔터티 매니페스트에는 `버전` 필드가 포함될 수 있으며, 이는 [ADR-45](https://adr.decentraland.org/adr/ADR-45). 이는 무시해도 안전합니다. 왜냐하면 `타임스탬프` 필드가 이제 버전 관리에 사용되기 때문입니다.


이는 엔터티를 설명하는 전형적인 JSON 매니페스트입니다:

```json
{
  "type": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0xbdf21eaf54ebf4a6cadc2dcb371df7afce98bc1d:0"],
  "timestamp": 1628181913506,
  "content": [
    // ...파일 참조, 아래 참조
  ],
  "metadata": {
    // ...이 엔터티 유형의 특정 필드, 관련 페이지를 참조하세요
  }
}
```

이 JSON 구조의 스키마와 Decentraland 프로토콜의 다른 객체는 [공통 스키마](https://github.com/decentraland/common-schemas) 저장소에서 찾을 수 있습니다.


> [!NOTE]
> 엔터티 매니페스트를 살펴보면 문서화되지 않은 필드를 발견할 수 있습니다. 이는 엔터티 스키마가 소유자가 자유롭게 설정할 수 있는 추가 사용자 지정 속성을 허용하기 때문입니다.


### 파일 <a href="#files" id="files"></a>

위에서 언급했듯이, 모든 엔터티에는 최소 하나의 연결된 파일이 있습니다. 바로 엔터티 자체를 설명하는 JSON 매니페스트입니다. 엔터티 식별자는 실제로 [파일 식별자](/docs/contributor/contributor-ko/content/file-system/#identifiers) 이 특별한 파일의 파일 식별자입니다.

이 `콘텐츠` 필드 안에 있는 배열은 추가 파일에 대한 참조 배열입니다. 이는 일반적으로 3D 모델과 애니메이션, 또는 씬용 스크립트와 같은 에셋입니다.

모든 파일은 Decentraland의 [분산 파일 시스템](/docs/contributor/contributor-ko/content/file-system/)에 저장되며, 배열의 각 항목에는 두 가지 속성이 있습니다:

| 필드   | 값                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------- |
| `파일` | 이 엔터티의 파일들이 서로를 참조할 때 사용하는 내부 이름입니다.                                                              |
| `해시` | 전역 [이 파일의 식별자](/docs/contributor/contributor-ko/content/file-system/#identifiers)이며, 모든 콘텐츠에서 고유합니다. |

이는 일반적으로 `콘텐츠` 필드 안에서 다음과 같이 보입니다:

```json
[
  {
    "file": "thumbnail.png",
    "hash": "bafkreiglecvpnqibvf6pltcnid5nhbx3caj77lu4ia4xitpmp3lrcouuhm",
  },
  {
    "file": "model.glb",
    "hash": "bafkreic2i3awiu7srhatf3k47l3c5lmadisznjigppor2a35saosjfbo25",
  },
  // ...더 많은 파일
]
```


> [!NOTE]
> 이 `파일` 필드 값은 항상 소문자여야 합니다. 이는 파일명 대소문자가 중요할 수 있는 서로 다른 운영 체제에서 엔터티를 빌드할 때 발생할 수 있는 문제를 방지하기 위함입니다.


파일의 수명은 이를 포함하는 엔터티와 연결됩니다. 활성 엔터티(즉, 아직 소유자에 의해 대체되지 않은 엔터티)의 경우, 프로토콜상 콘텐츠 서버는 관련된 모든 파일을 보존해야 합니다. 엔터티가 삭제되면 파일은 서버의 재량에 따라 보관하거나 폐기할 수 있습니다.

### 소유권 및 인증 <a href="#ownership" id="ownership"></a>

소유권을 증명하고 엔터티 관련 작업을 승인하려면, [인증 체인](/docs/contributor/contributor-ko/authentication/authchain/) 메커니즘이 사용됩니다.

이 [`decentraland-crypto`](https://github.com/decentraland/decentraland-crypto) 저장소에는 모든 암호학적 절차의 구현이 들어 있습니다.

### 엔터티 검색 및 다운로드

콘텐츠 서버는 [포인터](/docs/contributor/contributor-ko/content/pointers/)를 사용해 엔터티를 찾는 데 사용할 수 있으며, 해당 매니페스트와 추가 파일을 다운로드하는 데도 사용할 수 있습니다.

* 포인터를 엔터티 ID로 변환하려면 [`/entities/active`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getListOfEntities) 엔드포인트.
* 엔터티 ID를 사용하면 매니페스트를 [`/contents/<id>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) 엔드포인트.
* 특정 유형의 모든 활성 엔터티를 가져오려면 먼저 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/).

를 확인하세요 [실습](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 섹션에서 예시와 가이드를 확인할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
