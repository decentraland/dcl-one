# CLI 예제

이 실습에서는 터미널에서 콘텐츠 API를 다루는 방법을 보여줍니다. Decentraland Foundation의 인스턴스를 다음 주소에서 사용합니다 `peer.decentraland.org`, 그리고 필요한 것은 `curl` 명령줄 도구입니다.


> [!NOTE]
> 다음 도구를 사용할 수 있습니다 [`httpie`](https://github.com/httpie/httpie) 또는 다음으로 응답을 서식 지정할 수 있습니다 [`jq`](https://github.com/stedolan/jq) 각 요청에서 더 읽기 쉬운 JSON을 얻을 수 있습니다. 이 가이드에서는 명확성을 위해 서식이 지정된 문서를 보여드리겠습니다.


우리가 할 작업은 다음과 같습니다:

1. 콘텐츠 서버의 상태를 조회합니다.
2. 일별 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/) 엔티티 목록이 포함된
3. 엔터티의 매니페스트를 가져옵니다.
4. 엔터티의 파일 중 하나를 다운로드합니다.

먼저 다음을 사용해 서버 상태를 조회해 봅시다 `/about`:

```bash
curl "https://peer.decentraland.org/about"
```

```js
{
  "healthy": true,
  "content": {
    "healthy": true,
    "version": "6.5.0",
  },
  // ... 더 많은 서버 정보(기능 플래그, 버전, 경로 등)
}
```

서버가 정상적으로 가동 중인 것 같습니다 (`"healthy": true`), 그리고 각 기능 세트에 대해 구현된 버전과 인스턴스의 일부 구성 옵션에 대한 정보를 제공합니다.

콘텐츠를 다운로드하는 데 관심이 있으므로 사용 가능한 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/) 통해 몇몇 식별자를 얻기 위해 `/content/snapshots`.

```bash
curl "https://peer.decentraland.org/content/snapshots"
```

```js
[
  {
    "hash": "bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224",
    "timeRange": {
      "initTimestamp": 1689120000000,
      "endTimestamp": 1689206400000
    },
    "replacedSnapshotHashes": [],
    "numberOfEntities": 981,
    "generationTimestamp": 1689219353866
  },
  // ...더 많은 스냅샷 파일
]
```

배열의 각 항목은 하나의 [스냅샷](/docs/contributor/contributor-ko/content/snapshots/). 하나를 가져와 봅시다 `해시` 그리고 다음에서 파일을 다운로드합니다 `/content/contents` 엔드포인트.

```bash
curl "https://peer.decentraland.org/content/contents/bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224" > snapshot
```


> [!NOTE]
> 더 큰 스냅샷도 실험해 볼 수 있습니다. 예를 들어 [고급 Python 예제](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py). 대화형으로 시도해 보려면 아마 더 작은 것 중 하나를 원할 것입니다.


첫 줄을 보면 우리가 알고 있는 형식으로 올바른 파일을 다운로드했는지 확인할 수 있습니다:

```bash
head -n1 snapshot
```

```
### Decentraland JSON 스냅샷
```

좋습니다! 이제 해당 스냅샷에 캡처된 모든 엔터티의 로컬 요약이 있습니다. 목록의 첫 번째 항목(파일의 두 번째 줄), 즉 하나를 가져와 봅시다 `프로필`:

```bash
tail -n+2 snapshot | head -n1 
```

```js
{
  "entityId": "bafkreif7hjremkxlvixyxoxnoo7bdcnf7qqp245sjb2pag2nk3n6o6yc4c",
  "entityType": "profile",
  "pointers": [
    "0x271cdb3b1c792c336c4b2bdc52c4f415d0046b92"
  ],
  "authChain": [
    // 자세한 내용은 https://docs.decentraland.org/contributor/auth/authchain/ 참조
  ],
  "entityTimestamp": 1689120135624
}
```


> [!NOTE]
> 스냅샷은 만료되고 엔터티는 교체되므로 이 문서의 식별자는 작동하지 않습니다. 실제로 활성화된 파일 ID를 얻으려면 명령줄에서 따라 해 보세요.


이 정보는 저장할 수 있습니다. 우리는 다음을 사용할 것입니다 `entityId` 엔터티의 JSON 매니페스트를 다운로드하는 데 사용하지만, 나열된 [포인터](/docs/contributor/contributor-ko/content/pointers/) 을(를) 보존해 두는 것이 좋습니다. 그래야 나중에 이 엔터티와 업데이트된 버전을 찾을 수 있습니다.

또한 우리는 [인증 체인](/docs/contributor/contributor-ko/content/entities/#ownership) 이 엔터티에 서명하는 데 사용되었으며, 다운로드하는 관련 파일의 진위를 확인하기 위해 나열된 서명을 검증할 수 있습니다.

엔터티 매니페스트를 가져와 봅시다. 기억하세요, `entityId` 는 [파일 식별자](/docs/contributor/contributor-ko/content/file-system/#identifiers) 우리가 필요한 것이며, 우리는 `/content/contents` 엔드포인트를 다시 사용할 수 있습니다:

```bash
curl "https://peer.decentraland.org/content/contents/bafkreigcreq7rv6b2wf4zc4fsnif43ziwb4q46v4qhsewpf7gbsyxew3om"
```

```js
{
  "version": "v3",
  "type": "profile",
  "pointers": [
    "0x273cdb3b1c791c336c4b2bcc52c4f415d0046b91"
  ],
  "timestamp": 1689120135624,
  "content": [
    {
      "file": "body.png",
      "hash": "bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u"
    },
    // ...더 많은 파일
  ],
  "metadata": {
    // ...아바타 및 기타 정보
  }
}
```

이 `프로필` 엔터티에는 World Explorers가 플레이어를 렌더링하고 애니메이션하는 데 사용하는 모든 정보가 들어 있습니다. 패키지된 파일 중 하나를 얻는 데 관심이 있다면 계속해서 다음을 사용할 수 있습니다 `/content/contents` 엔드포인트.

우리가 `콘텐츠` 필드를 보면, 우리는 `해시` 내부적으로 다음이라고 불리는 파일의 `body.png`. 가져와 봅시다:

```bash
curl "https://peer.decentraland.org/content/contents/bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u" > body.png
```

이것을 열 수 있습니다 `png` 파일을 이미지 뷰어나 웹 브라우저에서 열어 작성자의 작업을 확인할 수 있습니다. 좋네요!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/practice/cli-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
