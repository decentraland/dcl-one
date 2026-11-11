# 저장소

스토어는 오프체인에 있습니다 [엔티티입니다](/docs/contributor/contributor-ko/content/entities/) 사용자가 관리하는 마켓플레이스 사이트를 나타냅니다. 여기에는 구매자에게 보여줄 설명과 이미지, 그리고 스토어 소유자에 대한 정보가 포함됩니다.

### 포인터 <a href="#pointers" id="pointers"></a>

스토어 포인터는 URN입니다 `오프체인` 네임스페이스에 있으며, 형식은 다음과 같습니다:

```
urn:decentraland:off-chain:marketplace-stores:<owner-address>
```

주소 부분은 스토어 소유자의 이더리움 주소입니다. 예:

```
urn:decentraland:off-chain:marketplace-stores:0xa2a1dc503be6fdb7878f58f053ded40564e3b9b2
```

### 메타데이터 필드

| 필드            | 값                                                                                       |
| ------------- | --------------------------------------------------------------------------------------- |
| `id`          | 이 [포인터](/docs/contributor/contributor-ko/content/pointers/) 이 스토어로 해석됩니다(또는 과거에는 해석되었습니다). |
| `description` | 이 스토어의 표시 제목입니다.                                                                        |
| `owner`       | 소유자의 이더리움 주소입니다.                                                                        |
| `이미지`         | 다음의 배열 `{ file, name }` 이미지를 참조하는 객체(아래 참조).                                            |
| `링크`          | 다음의 배열 `{ name, url }` 소셜 미디어 또는 웹사이트 링크가 포함된 객체(아래 참조).                                |

#### 이미지

스토어에는 마켓플레이스에서 구매자에게 보여줄 이미지가 있으며, 각각 역할을 식별하는 이름을 가집니다. 일반적인 `이미지` 배열은 이름이 `표지`인 단일 요소를 가집니다. 예:

```json
[
  {
    "name": "표지",
    "file": "imgs/store-logo.png"
  }
]
```

#### 링크

소유자는 소셜 미디어 프로필과 외부 웹사이트 같은 외부 링크를 스토어에 추가할 수 있습니다. Facebook 및 Discord URL이 일반적으로 포함됩니다. 예:

```json
[
  {
    "name": "facebook",
    "url": "https://www.facebook.com/mydclstore"
  },
  {
    "name": "website",
    "url": "https://mydclstore.com"
  }
]
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/content/entity-types/stores.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
