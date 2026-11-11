> Decentraland 컬렉션을 만드는 가이드라인

# 컬렉션 만들기

![](/docs/_remote/fd8372396891641073804c66.webp)

## Wearables Editor란 무엇인가요?

Wearables Editor는 Decentraland의 Builder 내 도구로, 사용자 지정 웨어러블을 업로드하고 메타데이터를 추가하며 게시할 수 있게 해줍니다.

> 주의: 이 문서는 웨어러블을 구성하는 모델, 메시, 텍스처를 만드는 방법을 설명하지 않습니다. 웨어러블을 업로드하고 게시하기 위해 Wearables Editor를 사용하는 방법만 설명합니다. 실제 웨어러블 제작 과정에 대한 소개는 여기에서 시작하세요.

### **로그인**

웨어러블을 업로드하고 게시하려면 다음으로 이동하세요 [**Decentraland Create**](https://decentraland.org/create).

클릭 **로그인**, 그런 다음 **연결** 을 눌러 Metamask 또는 Fortmatic 지갑으로 로그인하세요. 로그인한 후에는 **컬렉션** 탭.

## 컬렉션 만들기

웨어러블 또는 이모트를 업로드하려면 먼저 컬렉션을 만들어야 합니다. 그렇게 하려면 [**Decentraland Create**](https://decentraland.org/create) 로 이동한 다음 ***컬렉션*** 탭을 클릭하세요. 아직 컬렉션이 없다면 ***새 컬렉션*** 버튼을 눌러 새 컬렉션을 만드세요.

![](/docs/_remote/64f05d583b001a1dc5539128.webp) ![](/docs/_remote/e6e4a96a1938033d36033f9e.webp)

버튼을 클릭하면 아래에 표시된 것처럼 컬렉션 이름을 변경하라는 메시지가 나타납니다. 마음이 바뀌면 언제든지 컬렉션 이름을 수정할 수 있습니다 **게시하기 전까지**.

![](/docs/_remote/c9f95e65a853d6a8ec8d25ad.webp)

이제 항목을 추가할 준비가 되었습니다. 그러려면 Add Item 버튼을 클릭한 다음, 웨어러블/이모트 파일을 끌어다 놓거나 컴퓨터에서 찾아 선택하세요.

![](/docs/_remote/fc50d25f7580cdac09ae4a49.webp)

클릭하면 ***항목 추가***, 아래에 표시된 것과 같은 창이 나타납니다. 웨어러블/이모트 파일을 끌어다 놓거나 컴퓨터에서 찾아 선택하세요.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

이미 컬렉션이 있고 여기에 더 많은 항목을 추가하고 싶다면 Add Item을 클릭하고 파일을 업로드하세요. 컬렉션을 게시하기 전까지 원하는 만큼 파일을 추가할 수 있습니다.

![](/docs/_remote/5857f67a59bae2774cf081c8.webp)

*주의: 컬렉션이 게시된 후에는 항목을 추가, 제거하거나 희귀도를 변경할 수 없습니다.*

컬렉션의 각 항목에는 다음이 있습니다:

* **항목 이름:** 이 이름은 마켓플레이스에서 웨어러블 또는 이모트를 배포할 때 표시됩니다.
* **설명:** *(선택 사항)* 항목을 설명하는 간단한 문구로, 마켓플레이스에서 웨어러블 또는 이모트를 배포할 때 표시됩니다.
* **카테고리:** - 웨어러블이 속한 카테고리입니다(상체, 안경, 스킨 등)
* **희귀도:** - 희귀도는 게시 후 해당 웨어러블 또는 이모트로 발행할 수 있는 NFT의 최대 수를 결정합니다. 이는 컬렉션 게시 후 수정할 수 없는 유일한 속성입니다.
* **태그:** *(선택 사항)* 사용자가 항목을 검색하거나 필터링할 때 사용할 수 있는 설명 단어입니다. 경연이나 이벤트에 유용합니다!

### **희귀도**

항목의 희귀도는 발행할 수 있는 NFT의 총 수를 결정합니다. 희귀도는 이모트와 웨어러블 모두에 동일하며, 파일을 업로드하면 설정할 수 있습니다.

| 희귀도 | 항목 수    |
| --- | ------- |
| 유일  | 1       |
| 신화  | 10      |
| 엑조틱 | 50      |
| 전설  | 100     |
| 에픽  | 1,000   |
| 레어  | 5,000   |
| 언커먼 | 10,000  |
| 커먼  | 100,000 |

#### 주의!


> [!WARNING]
> ⚠️ **컬렉션 이름과 항목 수, 그리고 희귀도는 게시된 후에는 변경할 수 없습니다!**



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/manage-collections/creating-a-collection.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
