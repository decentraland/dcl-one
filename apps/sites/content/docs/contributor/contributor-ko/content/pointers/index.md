# 포인터

포인터는 활성 엔터티를 참조하는 고유한 대소문자 구분이 없는 문자열입니다. [엔티티](/docs/contributor/contributor-ko/content/entities/). 콘텐츠 서버는 이러한 포인터를 해석하여 엔터티의 식별자를 얻을 수 있습니다.

기억해 두세요, 둘 다 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 및 그들의 [파일이며](/docs/contributor/contributor-ko/content/file-system/) Decentraland 콘텐츠 시스템에서는 변경 불가능하며, 대체 버전이 업로드되면 해당 식별자가 변경됩니다. 반면 포인터는 교체가 이루어져도 유지되는 안정적인 참조입니다. 이는 소유자가 대체본을 업로드하면 포인터를 새 엔터티로 자동 리디렉션함으로써 달성됩니다.

여러 포인터가 동일한 [엔티티](/docs/contributor/contributor-ko/content/entities/), 이는 흔히 [씬](/docs/contributor/contributor-ko/content/entity-types/scenes/).

짐작하시겠지만, 포인터의 가장 일반적인 용도는 활성 버전(즉, 가장 최근 대체본)의 ID를 얻는 것입니다. [엔티티](/docs/contributor/contributor-ko/content/entities/), 이를 다운로드하기 위해서입니다.

### 포인터 유형

콘텐츠 서버가 엔터티로 해석할 수 있는 포인터에는 4가지 유형이 있으며, 각각 고유한 구문을 가집니다:

* [씬 포인터](/docs/contributor/contributor-ko/content/entity-types/scenes/#pointers) 는 다음과 같은 구획 좌표입니다 `"0,0"`.
* [프로필 포인터](/docs/contributor/contributor-ko/content/entity-types/profiles/#pointers) 는 소유자의 이더리움 주소입니다.
* [웨어러블 포인터](/docs/contributor/contributor-ko/content/entity-types/wearables/#pointers) 및 [이모트 포인터](/docs/contributor/contributor-ko/content/entity-types/emotes/#pointers) 는 다음 중 하나입니다 [collection](/docs/contributor/contributor-ko/content/collections/) 아이템 또는 오프체인 URN입니다.
* [스토어 포인터](/docs/contributor/contributor-ko/content/entity-types/stores/#pointers) 는 소유자의 이더리움 주소가 포함된 오프체인 URN입니다.
* [아웃핏 포인터](/docs/contributor/contributor-ko/content/entity-types/outfits/#pointers) 는 소유자의 이더리움 주소 뒤에 `:outfits`.

자세한 내용은 각 해당 섹션에서 확인할 수 있습니다.

### 포인터 해석

콘텐츠 서버에는 포인터를 활성 엔터티의 매니페스트로 해석할 수 있는 엔드포인트가 있으며, 다음과 같은 `GET` 요청을 보냅니다:

```
https://<content-base-url>/entities/<entity-type>?pointer=<pointer>
```


> [!NOTE]
> 역사적인 이유로, 이 엔드포인트의 응답은 하나의 요소를 포함하는 배열입니다.


Foundation의 콘텐츠 서버를 사용한 몇 가지 유용한 예시는 다음과 같습니다:

```bash
# 웨어러블에 대한 온체인 컬렉션 포인터:
curl "https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:matic:collections-v2:0x30517529cb5c16f686c6d0b48faae5d250d43005:0"

# 기본 에셋에 대한 오프체인 컬렉션 포인터:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:off-chain:base-avatars:BaseFemale

# 구획 포인터:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=0,0

# 프로필 포인터:
curl https://peer.decentraland.org/content/entities/profiles/?pointer=0xe2c2b80ca5ad868f4b30fa83cca2bf12cc95b4fe
```

이미 엔터티의 ID를 알고 있다면, 다음을 할 수 있습니다 [매니페스트를 다운로드할 수 있습니다](/docs/contributor/contributor-ko/content/file-system/#downloading) 파일을 직접 다운로드할 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/pointers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
