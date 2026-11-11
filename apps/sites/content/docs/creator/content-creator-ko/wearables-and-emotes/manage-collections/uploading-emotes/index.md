> 에디터에 이모트를 업로드하는 가이드라인

# 이모트 업로드

이모트를 내보낸 후에는 빌더에 업로드해야 합니다. 이 문서에서는 이모트 업로드 과정을 다룹니다.

## **파일 업로드**

파일을 업로드하기 전에 컬렉션을 만들어야 한다는 점을 기억하세요. 방법을 모른다면 [컬렉션 만들기](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). 이모트를 업로드하려면 파일을 그냥 끌어다 놓기만 하면 됩니다. ***새 항목*** 창에 놓거나 컴퓨터에서 찾아보세요. 파일에 애니메이션이 있는지 자동으로 감지하여 이모트로 식별합니다.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

업로드하려면 애니메이션 파일을 끌어다 놓으세요.

이모트의 이름을 입력하고, 희귀도, 카테고리, 재생 모드를 지정하라는 요청을 받게 됩니다. 썸네일 아래에는 파일의 애니메이션 클립 수, 애니메이션 길이(초), 길이(프레임 수), 프레임 속도가 표시됩니다.

![](/docs/_remote/e376a1e332918af541d1a910.webp)

## **.zip 파일을 사용해 이모트 업로드**

이모트에 소리가 있는 경우 (*mp3* 또는 *ogg*)는 다음과 함께 압축해야 합니다. `.glb`. 그 후에는 그냥 끌어다 놓으세요. `.zip` 를 빌더에 업로드하세요. 또한 다음을 추가할 수도 있습니다: `.json` 파일을 다른 에셋과 함께 같은 `.zip` 에 넣어 이름, 설명, 희귀도, 카테고리, 재생 모드 및/또는 태그를 추가할 수 있습니다. 각 항목의 정의는 다음과 같습니다:

* `이름`: 이모트 이름
* `설명`: 이모트 설명(공백을 포함하여 총 64자를 초과할 수 없음)
* `카테고리`: 이모트 카테고리("dance", "stunt", "greetings", "fun", "poses", "reactions", "horror", "miscellaneous")
* `희귀도`: 아이템 희귀도("unique", "mythic", "legendary", "epic", "rare", "exotic", "uncommon", "common")
* `재생 모드`: 단일 또는 루프 애니메이션("simple", "loop")
* `태그`: 마켓플레이스에서 쉽게 찾을 수 있도록 하는 태그.

이 정의를 이모트에 추가하려면 텍스트 파일을 만들고 이름을 **emote.json** 으로 지정한 다음, 예시처럼 아래 줄을 추가하세요:

```
{
  "name": "Tennis Shot",
  "description": "테니스를 할 수 있다는 걸 보여줘요",
  "category": "fun",
  "rarity":"epic",
  "play_mode": "simple",
  "tags":["tennis", "emote", "shot"]

}

```

이렇게 하면 빌더가 모든 .json 정보를 가져와 자동으로 이모트에 적용합니다.

**희귀도**

에서 희귀도 차트를 확인하세요 [여기](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/#rarity).

## **카테고리**

이모트를 가장 잘 설명하는 카테고리를 선택하세요.

* 댄스
* 스턴트
* 인사
* 재미
* 포즈
* 반응
* 공포
* 기타

## **재생 모드**

현재 두 가지 재생 모드가 있습니다:

* ***한 번 재생***: 이모트가 한 번만 재생된다는 뜻입니다. 그 후 아바타는 *대기* 자세로 돌아갑니다.
* ***루프***: 사용자가 다른 동작을 입력할 때까지 이모트가 반복 재생됩니다.

## **사용자 지정 썸네일**

이모트의 경우 편집기에 썸네일을 만들기 위한 내장 도구가 있으므로 이미지를 업로드할 필요가 없습니다. 동작을 가장 잘 나타내는 프레임을 선택하기만 하면 됩니다.

사람들이 썸네일만 보고도 애니메이션이 무엇인지 알아볼 수 있어야 합니다. 정면 샷이 충분하지 않다면 모델을 회전시키거나, 확대/축소하거나, 위아래로 이동하고, 클립에서 아무 프레임이나 선택해 보세요. 가장 좋은 장면을 선택하는 것이 정말 중요합니다!

![](/docs/_remote/9efbcbc5b36b6a3edfbcf38b.webp)

회전하고, 확대/축소하고, 위아래로 이동하세요. 도구를 사용해 애니메이션의 가장 좋은 장면을 확보하세요!

## 편집기

썸네일을 설정하면 편집기가 열립니다. 여기에서 3D 아바타가 동작을 수행하므로 애니메이션이 잘 재생되는지 확인할 수 있습니다.

![](/docs/_remote/58fef5ae57e3864b91e06697.webp)

왼쪽 아래 아이콘을 클릭하면 아바타를 편집할 수 있습니다. 오른쪽 아래의 실린더 아이콘을 클릭하여 애니메이션이 높이와 공간의 경계를 벗어나지 않는지 확인하세요.

![](/docs/_remote/c32c351247d541f21bafadb2.webp)

왼쪽 아래의 아이콘으로 아바타를 편집할 수 있습니다.

![](/docs/_remote/f4f828d6e5577c46357b79cf.webp)

실린더 아이콘은 애니메이션의 경계를 표시합니다.

이모트의 이름, 썸네일, 카테고리, 희귀도, 재생 모드를 수정할 수 있으며, 애니메이션 설명과 태그도 추가할 수 있습니다. 다음을 클릭하세요 ***저장*** 작업이 끝나면.

* **설명:** 이것은 마켓플레이스에 표시될 항목을 설명하는 간단한 문구입니다.
* **유틸리티** 웨어러블 또는 이모트의 월드 내 활용도를 설명하세요.
* **태그:** 태그는 사용자가 아이템을 검색하거나 필터링할 때 사용할 수 있는 단순한 설명 단어입니다. 이는 대회나 이벤트와 관련이 있습니다!

![](/docs/_remote/ae584da404403aeb0e47bcee.webp)

## 월드에서 테스트하기

편집기에서 애니메이션을 테스트한 후에도, Decentraland에서 실제로 어떻게 보이고 작동하는지 확인하는 것이 중요합니다. 월드에서 테스트하려면 Collections 탭으로 가서 원하는 컬렉션을 선택하면 그 안의 모든 아이템이 표시됩니다. Decentraland에서 보기를 클릭하세요.

\
\\

![](/docs/_remote/7fc9c8d40af1b921993b8bfb.webp)

\
\\


> [!NOTE]
> Empty Parcel 또는 Genesis Plaza에서 테스트할지 선택하라는 요청을 받게 됩니다. Empty Parcel을 선택하면 콘텐츠가 많지 않은 장소로 순간이동하게 되어 더 빠르게 로드됩니다. Genesis Plaza를 선택하면 메인 광장으로 이동합니다.


\
\\

![](/docs/_remote/d1c728c7c0ffee7fe42730da.webp)

\
\\


> [!NOTE]
> 선택한 후에는 사용자 지정 Catalyst를 사용하려고 한다는 메시지가 표시된 새 탭이 브라우저에 열립니다. 계속하려면 TRUST PEER-TESTING.DECENTRALAND.ORG를 클릭하세요.


\
\\

![](/docs/_remote/5e0eef48997d27eadb27149b.webp)

\
\\


> [!NOTE]
> 그 후 Decentraland Explorer를 열겠냐는 팝업이 표시됩니다. Explorer가 실행되면 Jump Into Decentraland를 클릭하세요.


\
\\

![](/docs/_remote/0518f3adcaa8974ae07124e4.webp)

\
\\


> [!NOTE]
> 월드에 들어가면 I 키를 눌러 백팩을 열고, 이모트 탭에서 테스트할 이모트를 선택하세요.


\
\\

![](/docs/_remote/80aeb1e924a01a378e86f206.webp)

\
\\

## **게시하기 전**

좋은 설명을 추가하고 모든 정보와 설정이 올바른지 확인하세요. 썸네일도 다시 한 번 확인하세요. 필요한 모든 정보를 입력했다면 아이템 상태가 Ready to Submit로 표시됩니다. 이제 이모트를 게시할 준비가 되었습니다!

![](/docs/_remote/e404c18c57fe014dd11e6091.webp)

Ready to Submit 상태는 파일이 게시할 준비가 되었음을 의미합니다!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
