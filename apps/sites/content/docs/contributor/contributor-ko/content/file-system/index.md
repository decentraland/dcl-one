# 파일 시스템

Decentraland 프로토콜의 여러 콘텐츠 API 뒤에는 분산 파일 시스템의 구현이 있습니다. 각 콘텐츠 서버는 전체 저장소의 복사본을 보유하고, 수신한 업데이트를 다른 인스턴스와 동기화합니다.

시스템에 업로드된 파일은 **불변입니다**. 배포되면 식별자도 내용도 바뀌지 않습니다. 새 버전이 업로드되고 애플리케이션이 그것을 사용하도록 선택하는 방식으로만 업데이트할 수 있으며, 이전 버전은 폐기할 수 있습니다.

파일을 다음에서 실험해볼 수 있습니다 [실습](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) 섹션.

### 파일 식별자 <a href="#identifiers" id="identifiers"></a>

Decentraland의 파일 시스템은 평면 인덱스로 구성되어 있습니다(즉, 디렉터리의 자연스러운 계층 구조가 없으며), 각 파일은 고유한 문자열로 식별됩니다.

식별자 문자열의 접두사는 [base32](https://en.wikipedia.org/wiki/Base32) 파일 내용의 SHA-256 해시를 인코딩한 값이며, 다음을 사용합니다 [IPFS CID v1](https://docs.ipfs.tech/concepts/content-addressing/) 알고리즘입니다. 예를 들면 다음과 같습니다:

```
bafybeicgclohdfaccu2sqqkzrzuenjxzcry3m5vcb4mpxgucjl3oheq5tq
```

처음 몇 바이트의 접두사는 식별자 자체의 인코딩, 버전 및 해시 유형을 나타냅니다.


> [!NOTE]
> 역사적인 이유로 일부 API는 파일 식별자를 *해시* 대신 *id*라고 부르지만, 둘은 같은 것을 가리킵니다. 오래된 콘텐츠에서는 조금 다르게 보이는 이전 방식의 식별자도 찾을 수 있습니다.


실제로는 콘텐츠를 찾고 다운로드하는 데 이러한 세부 사항이 필요하지 않습니다. 프로토콜의 대부분의 클라이언트는 기능을 잃지 않고 식별자를 불투명한 문자열처럼 취급할 수 있습니다.

최신 CIDv1 식별자는 IPFS와 호환되며, 같은 파일에 대해 동일한 CID를 생성하므로 IPFS를 파일 제공자로 사용할 수 있습니다.

### 소유권 및 지속성

다음을 제외하고는 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/), 네트워크에 저장된 파일은 다음과 연결됩니다 [엔티티](/docs/contributor/contributor-ko/content/entities/) 이더리움 계정이 소유합니다. 소유자만 엔티티와 관련 파일을 업데이트할 수 있습니다.

프로토콜에 따라 콘텐츠 서버는 항상 엔티티와 해당 파일의 최신 버전을 저장해야 하지만, 각자의 설정에 따라 이전 버전을 보관할지 여부를 선택할 수 있습니다.

### 파일 다운로드 <a href="#downloading" id="downloading"></a>

이 [`/contents/<fileId>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) 콘텐츠 서버의 엔드포인트를 사용해 어떤 파일이든 다운로드할 수 있습니다. 해당 식별자는 상위 [엔티티의](/docs/contributor/contributor-ko/content/entities/)매니페스트.

### 파일 업로드

파일은 개별적으로 업로드되지 않습니다. 대신 엔티티 내부에 패키징되어, 엔티티를 콘텐츠 서버에 배포하는 동안 업로드됩니다.

다음을 참조하세요 [콘텐츠 제작자 문서](https://github.com/decentraland/docs/tree/main/creator/README.md) 자세한 내용은.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/file-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
