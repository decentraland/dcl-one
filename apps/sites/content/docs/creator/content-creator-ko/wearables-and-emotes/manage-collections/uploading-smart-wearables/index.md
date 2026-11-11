> 에디터에 스마트 웨어러블을 업로드하는 가이드라인

# 스마트 웨어러블 업로드

다음을 사용해 Smart Wearable을 생성한 후 [SDK7](https://github.com/decentraland/docs/blob/main/creator/sdk7/projects/smart-wearables.md), 다음 단계는 빌더에 업로드하는 것입니다. 이 문서에서는 업로드하고, 게시하고, Smart Wearable을 판매하는 방법을 설명합니다.

## 파일 업로드

파일을 업로드하기 전에 컬렉션을 만들어야 한다는 점을 기억하세요. 방법을 모른다면 확인해 보세요 [컬렉션 만들기](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/manage-collections/creating-a-collection.md). Smart Wearable을 업로드하려면 파일을 ***새 항목*** 창에 끌어다 놓거나 컴퓨터에서 찾아보세요. 파일이 Smart Wearable인지 자동으로 감지합니다. **컬렉션의 최대 파일 크기는 3MB입니다**.

![](/docs/_remote/48697c84cb446eeea05d89d3.webp)

파일을 업로드하면 영상 쇼케이스를 업로드하라는 요청이 표시됩니다.

## 영상 쇼케이스 업로드하기

이 영상 쇼케이스는 마켓플레이스에 표시되어 구매자가 Smart Wearable이 어떻게 작동하는지 볼 수 있습니다.

영상 쇼케이스를 업로드하려면 **.mp4** 비디오 파일을 ***Smart Wearable용 동영상 업로드*** 창에 끌어다 놓거나 컴퓨터에서 찾아보세요. **동영상의 최대 파일 크기는 4MB이고 최대 재생 시간은 15초입니다**.

![](/docs/_remote/763eb735dd9262151b2d4576.webp)

영상 쇼케이스를 업로드하면 이름을 입력하고 희귀도와 카테고리를 지정하라는 요청이 표시됩니다. 원하면 Smart Wearable의 썸네일도 추가할 수 있습니다.

### 희귀도

항목의 희귀도를 선택하세요.

![](/docs/_remote/5aa59b7826603f649de1aade.webp)

| 희귀도 | 항목 수    |
| --- | ------- |
| 유니크 | 1       |
| 신화  | 10      |
| 이국적 | 50      |
| 전설  | 100     |
| 에픽  | 1,000   |
| 희귀  | 5,000   |
| 언커먼 | 10,000  |
| 일반  | 100,000 |

### 카테고리

Wearable은 아바타의 어느 부분을 수정하는지에 따라 여러 카테고리로 나뉩니다. 이 단계에서는 적절한 아이템 카테고리를 선택해야 합니다.

![](/docs/_remote/805bd4e946bcf6044e5f79e7.webp)

### 사용자 지정 썸네일

카메라 아이콘을 클릭하고 컴퓨터에서 원하는 이미지를 선택하여 썸네일을 추가할 수 있습니다. **썸네일은 투명한 배경의 256px 정사각형 .png 파일 형식이어야 합니다.** 큐레이션 위원회는 투명한 배경이 없는 썸네일이 있는 컬렉션을 승인하지 않으니 참고하세요.

![](/docs/_remote/d45b65391e8cc81a9f4c7a8c.webp)


> [!WARNING]
> ⚠️ Wearable을 잘 렌더링하는 것은 잠재적인 마켓플레이스 사용자에게 더 매력적으로 보이게 하는 데 매우 중요합니다. **Wearable 자체 외의 그래픽을 추가하지 않는 것이 중요합니다. 그렇게 하면 큐레이션 위원회가 거절할 수 있습니다.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### 영상 쇼케이스 업데이트

영상 쇼케이스를 업데이트하려면 카메라 아이콘을 클릭하고 컴퓨터에서 동영상을 찾아보세요. **허용되는 최대 파일 크기는 4MB이며 재생 시간은 15초를 넘지 않아야 합니다**.

![](/docs/_remote/3375e0716ee39e30f5ccb5aa.webp)

### 속성

썸네일 옆에는 모델의 삼각형 수, 재질 수, 텍스처와 같은 Wearable의 속성이 표시됩니다.

![](/docs/_remote/41d92d3af0ff7eebd17ea3cf.webp)

## 입, 눈, 눈썹 업로드

입, 눈, 눈썹 카테고리는 .png 파일일 뿐이므로 편집기에서 다른 동작을 보입니다. 이를 업로드하려면 투명 이미지(256x256픽셀)로 png 파일을 끌어다 놓기만 하면 됩니다. 입은 피부색으로 자동 색조가 입혀지고, 눈썹은 머리색으로 색조가 입혀집니다.


> [!WARNING]
> 에셋을 마스크 처리하여 입이나 눈썹의 일부가 색조의 영향을 받지 않게 하려면 png 파일과 마스크 파일이 모두 들어 있는 zip 파일을 업로드하세요. 마스크 파일이 작동하려면 접미사 "\\\_mask"가 있어야 한다는 점을 기억하세요.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

그런 다음 Wearable을 업로드하면 컬렉션의 아이템을 보여주는 다음과 같은 화면이 표시됩니다.

![](/docs/_remote/83043b56afafb5b81fe2be0f.webp)

## Smart Wearable의 가격 설정

다음을 클릭하여 Wearable의 가격을 설정할 수 있습니다 ***가격 설정***. 이 모든 내용은 언제든지 수정할 수 있으니 나중에 변경하고 싶어도 걱정하지 마세요. 가격은 MANA로 설정됩니다. Wearable을 민팅할 때는 Matic/Polygon에서 직접 민팅된다는 점을 기억하세요. 실제로 사용자가 아이템을 구매하면 거래는 Matic/Polygon MANA로 진행됩니다.

또는 ***무료로 설정***, 즉 가격은 0 MANA가 되고 수령인 주소는 null이 됩니다. 1차 판매에서 제품을 무료로 제공하더라도 2차 판매에서 어떤 가격으로든 판매되는 것을 막지는 못합니다.

판매 수익으로 MANA를 받을 수혜자 주소를 설정하는 것을 잊지 마세요. 원하는 Ethereum 주소를 사용할 수 있습니다. 로그인한 주소를 자동으로 채우려면 ***저는 수혜자입니다.***

![](/docs/_remote/44c1580896b6f7d4ff6a5349.webp)

가격을 저장하면 컬렉션의 Wearable 목록으로 돌아갑니다. 아이템을 클릭하면 일반 정보를 확인할 수 있습니다. 버튼을 클릭하세요 ***편집기에서 미리보기*** 편집기에서 확인하세요.

![](/docs/_remote/8170f9434918babf3565876f.webp)

## 편집기

을 클릭하면 ***편집기에서 미리보기***, 편집기가 열립니다. 설명, 태그, 오버라이드를 포함한 Smart Wearable 정보를 편집할 수 있습니다.

![](/docs/_remote/cabb347fa07def0f5e919e48.webp)

## 설명

이것은 마켓플레이스에 표시될 항목을 설명하는 간단한 문구입니다.

## 오버라이드

오버라이드는 항목이 어떤 웨어러블 카테고리나 아바타의 신체 부위를 숨길지를 결정합니다. 예를 들어, 머리카락이 붙은 모자는 *머리카락* 카테고리를 숨겨야 할 수 있습니다. 심해 잠수부 헬멧은 귀걸이, 안경, 티아라 등 보이지 않게 될 머리 장식을 숨겨야 할 수 있습니다. 각 오버라이드에는 여러 옵션을 선택할 수 있습니다.

* **기본 신체**: 이는 *머리* 및 *손*과 같은 핵심 아바타 부위를 의미합니다. 예를 들어 **손 착용** 아이템, 예를 들어 로봇 정비공의 손을 만들고 있다면 아마도 *손* 를 숨겨 겹침과 클리핑을 방지해야 합니다.
* **웨어러블**: 여기에는 다른 Wearable 카테고리도 포함됩니다. 여러 카테고리를 숨길 수 있습니다. 다음을 확인해 보세요 [**웨어러블 만들기**](https://github.com/decentraland/docs/blob/main/creator/wearables-and-emotes/wearables/creating-wearables.md) 각 카테고리와 아이템 간 상호작용에 대한 자세한 내용은 여기를 참조하세요.


> [!WARNING]
> 참고: 선택한 오버라이드는 웨어러블의 권장 기본 설정이 됩니다. 하지만 사용자는 백팩에서 숨길 웨어러블과 표시할 웨어러블을 사용자 지정할 수 있습니다.


## 태그

태그는 사용자가 대회나 이벤트에서 활용할 수 있는 설명용 단어입니다!

## 미리보기

Smart Wearable을 미리 보려면 왼쪽 상단의 wearable 요소 위에 마우스를 올리고 눈 모양 아이콘을 클릭하세요.

![](/docs/_remote/38a5124d67833aaa252218c1.webp)

왼쪽 하단의 아이콘을 클릭하면 아바타를 편집할 수 있습니다. Wearable의 남성용 및 여성용 버전이 있는 경우 유용하며, 편집기에서 두 버전이 어떻게 보이는지 확인하고 다양한 이모트를 테스트하여 스키닝 문제를 찾아보며 다른 Wearable과 섞어서 다른 옷과 어떻게 어울리는지 확인할 수 있습니다. Wearable 편집을 마치면 다음을 클릭하세요 ***저장***.

![](/docs/_remote/2d3fb93f880674b537bb022c.webp)

## Decentraland에서 테스트하기

편집기에서 Smart Wearable을 테스트한 후에도 Decentraland에서 어떻게 보이고 작동하는지 확인하는 것이 중요합니다. 그곳에서 시험해 보려면 컬렉션 탭으로 이동하세요. 원하는 컬렉션을 선택하고 버튼을 클릭하세요 ***Decentraland에서 보기***.

![](/docs/_remote/f98021fb76ec1a330e1a9f4a.webp)

다음을 클릭한 후 ***Decentraland에서 보기*** 버튼을 누르면 다음 팝업이 나타납니다. 다음을 선택하면 ***빈 토지*** 를 선택하면 콘텐츠가 많지 않은 곳으로 이동하여 더 빨리 로드됩니다. 선택하면 ***제네시스 플라자*** 메인 광장으로 이동합니다.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

텔레포트할 위치를 선택하면 브라우저에서 새 탭이 열리고 이 메시지가 표시됩니다.

![](/docs/_remote/61eed76d8ed92cce5d514ea9.webp)

다음을 클릭하세요 ***Sepolia로 전환***, 그리고 네트워크 전환을 요청하는 지갑 팝업이 나타납니다. 네트워크 전환을 클릭하면 새 탭이 자동으로 새로고침됩니다. Smart Wearable을 테스트하려면 배낭으로 가서 선택하면 됩니다.


> [!WARNING]
> Ethereum 테스트넷 네트워크를 활성화하지 않았다면 다음을 따라 하세요 [Metamask 문서](https://support.metamask.io/hc/en-us/articles/13946422437147-How-to-view-testnets-in-MetaMask).


![](/docs/_remote/91716ea1b880be3f761e1a9c.webp)

## 게시하기 전

가격을 올바르게 설정하고, 설명을 멋지게 추가한 뒤, 모든 정보와 설정이 맞는지 다시 확인하세요. 필요한 모든 정보를 입력하면 ***완료*** 가 항목의 상태로 표시됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
