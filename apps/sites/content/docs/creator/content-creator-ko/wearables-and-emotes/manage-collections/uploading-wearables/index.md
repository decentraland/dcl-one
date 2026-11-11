> 에디터에 웨어러블을 업로드하는 가이드라인

# 웨어러블 업로드

웨어러블을 내보낸 후에는 빌더에 업로드해야 합니다. 이 문서에서는 웨어러블 업로드 과정을 다룹니다.

## 파일 업로드

파일을 업로드하기 전에 컬렉션을 만들어야 한다는 점을 기억하세요. 방법을 모른다면 [컬렉션 만들기](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). 웨어러블을 업로드하려면 파일을 ***새 항목*** 창에 끌어다 놓거나 컴퓨터에서 찾아보세요. 파일이 이모트인지 웨어러블인지 자동으로 감지합니다. **컬렉션의 최대 파일 크기는 3MB입니다**.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

파일을 업로드하면 체형을 선택하고 이름을 입력한 뒤 희귀도와 카테고리를 정의하라는 요청을 받게 됩니다. 웨어러블의 썸네일도 추가할 수 있습니다.

### **체형**

웨어러블이 의도한 아바타에게 제대로 착용되도록 하려면, 각 체형마다 별도의 GLB 파일을 업로드해야 합니다. 남성용과 여성용으로 서로 다른 두 버전의 웨어러블이 있다면, 업로드 과정에서 한 가지 표현을 추가한 뒤 편집기를 사용해 나중에 다른 하나를 추가할 수 있습니다. 웨어러블이 유니섹스용이라면 남성과 여성 버전에 모두 맞도록 설계된 단일 GLB 파일을 업로드해야 합니다.

![](/docs/_remote/9c4ad1dbfd25f7f986e2b0d5.webp)

### **희귀도**

항목의 희귀도를 선택하세요.

![](/docs/_remote/a57649e01874c1a4789f0456.webp)

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

### **카테고리**

웨어러블은 아바타의 어느 부분을 수정하는지에 따라 여러 카테고리로 분류됩니다. 항목에 맞는 카테고리를 선택하세요.

![](/docs/_remote/ac233f1f8c3b34463031b787.webp)

### **사용자 지정 썸네일**

카메라 아이콘을 클릭하고 컴퓨터에서 찾아보면 직접 만든 사용자 지정 썸네일을 추가할 수 있습니다. **썸네일은 배경이 투명한 256px 정사각형 .png 파일이어야 합니다.** 투명 배경이 아닌 썸네일이 포함된 컬렉션은 큐레이션 위원회에서 승인되지 않습니다.

![](/docs/_remote/5a563c26949da7b96f8c2028.webp)


> [!WARNING]
> ⚠️ 웨어러블의 좋은 렌더링은 마켓플레이스에서 잠재 사용자에게 더 매력적으로 보이게 하는 데 매우 중요합니다. **웨어러블 자체 외에 다른 그래픽을 추가하지 않는 것이 중요합니다. 그렇지 않으면 큐레이션 위원회에서 거부할 수 있습니다.**
> 
> <img src="/docs/_remote/b8bdd9d193e7703e63def953.webp" alt="" data-size="original">


### **스프링 본**

웨어러블에 `springbone` 라는 이름이 붙은 본이 포함되어 있다면, 빌더가 이를 자동으로 감지하고 **스프링 본** 구성 패널을 표시합니다. 이 패널에서 각 스프링 본 체인의 강성, 중력, 드래그 같은 물리 파라미터를 조정할 수 있습니다. 자세한 내용은 [스프링 본](/docs/creator/wearables-and-emotes/wearables/spring-bones/) 를 참고하여 모델에서 스프링 본을 설정하는 방법을 확인하세요.

### **속성**

썸네일 아래에는 웨어러블의 속성, 모델의 삼각형 수, 재질 수 및 텍스처 수가 표시됩니다.

![](/docs/_remote/c1c0778d6b438f3e33e8c6cf.webp)

## **입, 눈, 눈썹 업로드**

입, 눈, 눈썹 카테고리는 .png 파일일 뿐이므로 편집기에서 다른 동작을 보입니다. 이를 업로드하려면 투명 이미지(256x256픽셀)로 png 파일을 끌어다 놓기만 하면 됩니다. 입은 피부색으로 자동 색조가 입혀지고, 눈썹은 머리색으로 색조가 입혀집니다.


> [!WARNING]
> 에셋을 마스크 처리하여 입이나 눈썹의 일부가 색조의 영향을 받지 않게 하려면 png 파일과 마스크 파일이 모두 들어 있는 zip 파일을 업로드하세요. 마스크 파일이 작동하려면 접미사 "\\\_mask"가 있어야 한다는 점을 기억하세요.


![](/docs/_remote/ba0ba79e9d5699b2b18f666a.webp)

그다음 웨어러블을 업로드하면 아래와 같이 컬렉션의 항목이 표시되는 화면이 나타납니다.

![](/docs/_remote/b1d31fcc4dddce1716af7bc8.webp)

## **웨어러블 가격 설정**

컬렉션을 발행하고 승인을 받으면 컬렉션의 판매를 활성화할 수 있습니다.

![](/docs/_remote/dd9d2d46268767048dd15f53.webp)

을 클릭하여 웨어러블 가격을 설정할 수 있습니다 ***판매 등록***. 이 모든 것은 언제든지 수정할 수 있으므로 나중에 바꾸고 싶어도 걱정하지 마세요. 가격은 MANA로 설정됩니다. 웨어러블을 민팅하면 Matic/Polygon에서 직접 민팅된다는 점을 기억하세요. 사용자가 항목을 구매하면 거래는 Matic/Polygon MANA로 진행됩니다.

또는 ***무료로 설정***, 즉 가격이 0 MANA로 설정되고 수혜자 주소는 null이 됩니다. 무료로 설정하는 것(1차 판매)이 2차 판매에서 어떤 가격으로도 팔리는 것을 막지는 않는다는 점을 알아두세요.

판매 수익으로 MANA를 받을 수혜자 주소를 설정하는 것을 잊지 마세요. 원하는 Ethereum 주소를 사용할 수 있습니다. 로그인한 주소를 자동으로 채우려면 ***저는 수혜자입니다.***

![](/docs/_remote/f25c9cc68be82373f6fe392e.webp)

항목을 판매로 등록하면 컬렉션의 웨어러블 목록으로 돌아갑니다. 항목을 클릭하면 일반 정보를 볼 수 있습니다. 버튼을 클릭하세요 ***미리보기*** 편집기에서 보려면.

![](/docs/_remote/528700c2d810068ed39ccdb2.webp)

## 편집기

을 클릭하면 ***미리보기***&#xD3B8;집기가 열립니다. 설명, 태그, 오버라이드와 같은 새 항목을 추가하는 것은 물론, 웨어러블의 모든 정보를 편집할 수 있습니다. 여기에서 웨어러블에 다른 체형 표현도 추가합니다.

![](/docs/_remote/c21cb9360544651f12054903.webp)

## **설명**

이것은 마켓플레이스에 표시될 항목을 설명하는 간단한 문구입니다.

## **오버라이드**

오버라이드는 항목이 어떤 웨어러블 카테고리나 아바타의 신체 부위를 숨길지를 결정합니다. 예를 들어, 머리카락이 붙은 모자는 *머리카락* 카테고리를 숨겨야 할 수 있습니다. 심해 잠수부 헬멧은 귀걸이, 안경, 티아라 등 보이지 않게 될 머리 장식을 숨겨야 할 수 있습니다. 각 오버라이드에는 여러 옵션을 선택할 수 있습니다.

* **기본 신체**: 이는 *머리* 및 *손*과 같은 핵심 아바타 부위를 의미합니다. 예를 들어 **손 착용** 아이템, 예를 들어 로봇 정비공의 손을 만들고 있다면 아마도 *손* 를 숨겨 겹침과 클리핑을 방지해야 합니다.
* **웨어러블**: 여기에는 다른 웨어러블 카테고리가 포함됩니다. 여러 카테고리를 숨길 수 있습니다. 각 카테고리와 아이템이 어떻게 상호작용하는지 자세한 내용은 [**웨어러블 만들기**](/docs/creator/wearables-and-emotes/wearables/creating-wearables/).


> [!WARNING]
> 참고: 선택한 오버라이드는 웨어러블의 권장 기본 설정이 됩니다. 하지만 사용자는 백팩에서 숨길 웨어러블과 표시할 웨어러블을 사용자 지정할 수 있습니다.


## **태그**

태그는 사용자가 아이템을 검색하거나 필터링할 때 사용할 수 있는 단순한 설명 단어입니다. 이는 대회나 이벤트와 관련이 있습니다!

## **VRM 내보내기 권한**

이 속성을 활성화하면 해당 항목의 소유자가 VRM 아바타 내보내기에 이를 포함하여 데센트럴랜드 밖에서도 자랑할 수 있게 됩니다.

## **윤곽선**

윤곽선 토글은 웨어러블이 데센트럴랜드의 윤곽선 렌더링 시스템(중간/높음 품질 설정에서 사용됨)과 호환되는지를 제어합니다. 기본적으로 모든 새 웨어러블에 대해 활성화되어 있습니다.

웨어러블이 다음과 같은 경우 이 옵션을 비활성화해야 합니다:

* 모델에 수동으로 만든 윤곽선이 있는 경우
* 반전 노멀을 사용하는 경우
* 윤곽선 효과를 적용했을 때 시각적 아티팩트가 나타나는 경우

대부분의 웨어러블은 기본 설정(활성화됨)으로 잘 작동합니다. 웨어러블에서 윤곽선 관련 시각적 문제가 보일 때만 비활성화하세요. 윤곽선을 비활성화한 후에는 ***저장***.

![](/docs/_remote/69f438abb59b36e0dc7bcc84.webp)

## **다른 표현 추가**

웨어러블에 남성과 여성용으로 다른 표현이 있다면 다른 파일을 업로드해야 합니다. 현재는 하나만 업로드된 상태입니다. 예시에서는 Krampus Sweater의 여성 버전이었습니다. 다른 표현을 추가하려면 오른쪽 위, 다음 항목 옆의 점 세 개(*…*)를 클릭하세요 ***속성*** 를 누르고 ***남성/여성 추가*** 표현. 아래 예시에서는 남성 버전이 필요했습니다.

![](/docs/_remote/21001d89e30650778f2bd9b8.webp)

그것을 클릭하면 남성/여성 표현 추가 창이 열립니다. 다른 표현 파일을 끌어다 놓아 업로드하세요. 업로드가 완료되면 다른 창이 웨어러블을 표시하고, 문제가 없으면 저장을 클릭하세요.

![](/docs/_remote/983512d9a30c68af5edb426d.webp) ![](/docs/_remote/7debe500c2fea74349f7fa23.webp)

## **미리보기**

웨어러블을 미리 보려면 왼쪽 상단의 웨어러블 아이콘 위에 마우스를 올리고 눈 모양 아이콘을 클릭하세요.

![](/docs/_remote/4634ace580b72aed8b781aed.webp)

왼쪽 아래 아이콘을 클릭하면 아바타를 편집할 수 있습니다. 이는 웨어러블의 남성 및 여성 버전이 있는 경우 매우 유용하므로, 편집기에서 두 버전이 어떻게 보이는지 확인하고, 다양한 이모트를 테스트해 스키닝 문제가 있는지 파악하고, 다른 웨어러블과 조합하여 다른 옷과 얼마나 잘 어울리는지 확인하세요. 웨어러블 편집이 끝나면 ***저장***.

![](/docs/_remote/cec3d31a2ab318de454d52a0.webp)

## **월드에서 테스트하기**

편집기에서 웨어러블을 테스트한 후에도 실제로 데센트럴랜드에서 어떻게 보이고 동작하는지 확인하는 것이 중요합니다. 월드에서 테스트하려면 컬렉션 탭으로 이동하세요. 원하는 컬렉션을 선택하고 버튼을 클릭하세요 ***월드에서 보기***.

![](/docs/_remote/4a233b404952959b105fa854.webp)

클릭하면 다음 팝업이 나타납니다. ***빈 토지*** 를 선택하면 콘텐츠가 많지 않은 곳으로 이동하여 더 빨리 로드됩니다. 선택하면 ***제네시스 플라자*** 메인 광장으로 이동합니다.

![](/docs/_remote/fa3231abc7a9d5db5a275577.webp)

월드에서 보기를 선택하면 브라우저에서 새 탭이 열리고 이 메시지가 표시됩니다.

![](/docs/_remote/a95471fb4beb838e7547a9bf.webp)

다음을 클릭하세요 ***TRUST PEER-TESTING.DECENTRALAND.ORG*** 라는 팝업이 나타납니다. 간단히 데센트럴랜드 열기를 클릭하세요. 웨어러블을 테스트하려면 백팩으로 이동해 착용하세요.

![](/docs/_remote/bd6c470a523472534ab4282f.webp)

## **게시하기 전**

가격을 올바르게 설정하고, 설명을 멋지게 추가한 뒤, 모든 정보와 설정이 맞는지 다시 확인하세요. 필요한 모든 정보를 입력하면 ***완료*** 가 항목의 상태로 표시됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
