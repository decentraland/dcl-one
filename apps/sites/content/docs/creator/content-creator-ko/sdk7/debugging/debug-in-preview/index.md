> 미리보기에서 로컬로 실행하는 동안 씬을 디버그하는 방법

# 미리보기에서 디버그

미리 보기를 실행하면 장면이 어떻게 렌더링되는지 이해하는 데 도움이 되는 유용한 디버깅 정보와 도구를 일부 제공합니다.

장면을 컴파일할 수 없으면, 아무것도 렌더링되지 않은 빈 바닥만 보이게 됩니다. 이런 경우, 무엇이 잘못되었는지 이해하는 데 도움이 되는 오류 메시지를 찾을 수 있는 곳이 몇 군데 있습니다:

1. 코드 편집기에서 구문 또는 논리 오류가 표시되지 않았는지 확인하세요.
2. 미리 보기 창의 콘솔에서 오류 메시지가 있는지 확인하세요. 다음을 클릭하여 콘솔을 엽니다. ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 오른쪽 상단 모서리에 있는
3. Creator Hub를 사용해 장면을 실행했다면, 장면의 콘솔 출력이 표시되는 창을 확인하세요. CLI를 사용해 장면을 실행했다면, 실행한 명령줄의 출력을 확인하세요. `npm run start`


> [!NOTE]
> **💡 팁**: 프로덕션 환경에 장면을 게시하기 전에, 다음에 게시할 수 있습니다. [Decentraland World](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 에서 프로덕션에 훨씬 더 가까운 환경에서 시험해 볼 수 있습니다.


## 콘솔 사용

다음을 클릭하여 콘솔을 엽니다. ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 오른쪽 상단 모서리에 있는

키보드에서 백틱 키를 눌러서 전환할 수도 있습니다: **`**. 이 키는 대부분의 영어 키보드에서 1 키 왼쪽에 있습니다. 또는 Shift + **\를 누르세요.`** 더 많은 텍스트를 보아야 할 경우를 대비해, 콘솔을 더 넓은 보기로 열 수 있습니다.

콘솔에 메시지를 출력하려면(다음을 사용하여 `console.log()`). 그런 다음 장면 콘솔을 열어 생성되는 대로 이 메시지들을 볼 수 있습니다.

콘솔에는 오류 메시지도 표시되며, 이는 빨간색으로 표시됩니다.

## 디버그 패널

다음을 클릭하여 디버그 콘솔을 엽니다. ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) 오른쪽 상단 모서리에 있는

그러면 렌더러 엔진에 대한 정보가 있는 패널이 오른쪽 아래 모서리에 열리며, 상황이 변함에 따라 실시간으로 업데이트됩니다.

![](/docs/_remote/eace962a130964afb6774cff.webp)

### 현재 장면

해당 **현재 장면** 탭에는 현재 서 있는 장면에 대한 정보가 포함되어 있습니다. 여기에는 다음이 포함됩니다:

* 장면의 파셀 수
* 건축이 허용되는 최대 높이(각 장면당 330미터)

다음도 확인할 수 있습니다. **장면 경계 표시** 옵션을 선택하면 장면 주변에 빨간 벽이 표시되어 장면의 경계를 나타냅니다.

### 메모리

해당 **메모리** 섹션에는 Decentraland가 사용하는 메모리가 표시됩니다. 이 값은 항상 다음 값보다 낮게 유지하도록 하세요. **메모리 예산 임계값**. 이 값이 너무 커지면, 장면을 방문하는 플레이어의 앱이 충돌할 위험이 있습니다. 프로덕션에서는 장면이 주변의 다른 장면과 플레이어와 함께 로드될 수 있으며, 이 모든 것이 미리 보기에서 보이는 것보다 메모리 소비를 더 높이는 데 영향을 줄 수 있다는 점을 유의하세요.

### 성능

해당 **성능** 탭에는 다음과 관련된 여러 필드가 포함되어 있습니다. *FPS* (초당 프레임 수). 장면이 더 복잡해질수록 이 값은 낮아지는 경향이 있으므로, 플레이어에게 좋은 경험을 보장하려면 항상 이 값을 25 FPS 이상으로 유지하세요. FPS는 실행하는 기기에 따라 달라집니다.

여기서 가장 중요한 값은 **평균 FPS**이며, 최근 1000프레임의 FPS를 반올림한 값입니다.

![](/docs/_remote/5ba9631495e357c71fac976e.webp)


> [!WARNING]
> **📔 참고**: 렌더러의 FPS는 초당 게임 루프에서 장면이 업데이트되는 횟수와 동일하지 않습니다. 장면의 로직은 30 FPS로 제한되며, 렌더러의 FPS는 60 이상까지 올라갈 수 있습니다.


끊김은 프레임 속도가 순간적으로 멈추는 것을 말하며, 전체 FPS 수치에는 큰 영향을 주지 않을 수 있지만 플레이어가 매우 눈에 띄게 느낄 수 있습니다. 이는 일반적으로 무거운 에셋이 로드될 때 등 발생할 수 있습니다.

### 웹 요청

해당 **웹 요청** 섹션에서는 Chrome Devtools의 Network 탭을 열 수 있으며, 장면에서 발생한 웹 요청 수와 그 내용을 표시합니다. 이는 장면이 서버에 너무 많은 요청을 보내고 있는지 알아보는 데 유용하며, 이는 장면 성능에 영향을 줄 수 있습니다.

장면이 서드파티 서버와 상호작용하는 경우, 어떤 데이터가 오가고 있는지 확인하는 것이 종종 유용합니다. 클릭하세요. **Chrome Devtools 열기** 를 눌러 Network 탭이 열린 새 Chrome 창을 엽니다.


> [!WARNING]
> **📔 참고**: 이 기능을 사용하려면 먼저 다음을 설치해야 합니다. [크리에이터 허브](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/), 필요한 종속 항목은 해당 설치에 포함되어 있습니다.
> 
> 이 기능을 사용하기 위해 Creator Hub를 열어 두거나 Creator Hub를 통해 장면 미리 보기를 실행할 필요는 없으며, CLI로 장면 미리 보기를 실행할 때도 사용할 수 있습니다.


대안으로, Decentraland 애플리케이션으로 들어오고 나가는 모든 데이터를 보고하는 무료 도구를 컴퓨터에서 실행할 수 있습니다. 예를 들어 [Charles](https://www.charlesproxy.com/) 또는 [Wireshark](https://www.wireshark.org/).

## 빠른 새로고침

장면에 변경 사항을 만들 때마다 미리 보기가 자동으로 새로고침되고 업데이트되므로, 미리 보기 창을 열고 닫을 필요가 없습니다.

서 있는 장면을 다시 불러와야 한다면, 다음을 클릭하세요. **장면 다시 불러오기** 미리 보기 창의 왼쪽 상단 모서리에 있는 버튼을 클릭하세요.

![](/docs/_remote/a2ec4266edb8ce7bb450b24d.webp)

또한 채팅에 다음을 입력하고 Enter를 눌러 장면을 다시 불러올 수 있습니다:

`/reload`

## 버그 신고

문제가 장면 자체가 아니라 Decentraland SDK 일반에 관한 것이라면, 다음을 참고하세요. [버그 신고](/docs/creator/content-creator-ko/sdk7/debugging/report-bug/).

## 종속성 버전

장면의 모든 종속성은 항상 최신 버전을 사용하도록 하세요. 겪고 있는 문제가 이미 새 버전에서 수정되었을 수 있기 때문입니다. Creator Hub는 Decentraland 장면에 설치할 업데이트가 있으면 알려줍니다.

![](/docs/_remote/a002cdf10ca3a37130f2959d.webp)

명령줄을 통해 종속 항목을 수동으로 업데이트할 수도 있습니다. Decentraland 장면을 로컬에서 실행하려면 두 가지 주요 라이브러리에 의존합니다: `@dcl/sdk` 및 `@dcl/js-runtime`는 각 프로젝트 폴더에 설치됩니다. 이 둘이 모두 최신 상태인지 확인하세요. 다음 명령어를 실행하여 이 두 라이브러리를 최신 안정 버전으로 수동 업데이트할 수 있습니다:

```
npm i @dcl/js-runtime@latest
npm i @dcl/sdk@latest
```

장면에서 스마트 아이템을 사용하는 경우, 다음도 사용 중일 수 있습니다. `@dcl/asset-packs` 라이브러리이며, 다음을 통해 업데이트할 수 있습니다:

```
npm i @dcl/asset-packs@latest
```

다음 중 하나를 사용 중이라면 [유틸 라이브러리](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) 이들도 최신 상태인지 확인하세요. 이러한 라이브러리의 이전 버전은 다음의 최신 버전과 호환되지 않을 수 있습니다. `@dcl/sdk`.

## 조명 조건

월드 내 시간대는 3D 모델의 외형에 큰 영향을 미칩니다. 광원 색상은 미묘하게 변하며, 밤에는 푸른빛을 띠고 일출과 일몰에는 붉은빛을 띱니다. 빛의 방향도 하늘을 가로질러 이동하며, 그림자를 다른 방향으로 드리웁니다.

게임 시계를 다른 값으로 바꿔서 하루 중 모든 시간대에서 장면이 잘 보이는지 확인하세요. 다음을 클릭하세요. **하늘박스 시간** 오른쪽 패널의 아이콘을 눌러 원하는 시간을 설정하세요. 이 슬라이더가 회색으로 비활성화되어 있다면 다음이 꺼져 있는지 확인하세요. **자동** 옵션

<figure><img src="/docs/_remote/be169c2886b8a8e49dfe5d5d.webp" alt="Dynamic skybox" width="300"><figcaption><p>월드 내 지침</p></figcaption></figure>

3D 모델의 재질은 제작에 사용한 모델링 도구에서 보던 것과 같지 않을 수 있습니다. 이는 모든 3D 렌더링 엔진이 빛과 재질을 처리하는 방식에 미묘한 차이가 있기 때문에 예상되는 일입니다.

또한 하루 중 시간이 고정되고 조명이 바뀌지 않도록 장면을 설정할 수도 있습니다. 다음을 참조하세요. [스카이박스 제어](/docs/creator/content-creator-ko/sdk7/interactivity/skybox-control/) 자세한 내용은.

## 멀티플레이어 테스트

장면 미리 보기를 실행하고 두 개(또는 그 이상)의 서로 다른 Explorer 창에서 열면, 각 열린 창은 별도의 플레이어로 해석되며 모의 통신 서버가 이 플레이어들을 동기화된 상태로 유지합니다.

한 창에서 장면과 상호작용한 다음, 다른 창으로 전환하여 그 상호작용의 효과도 거기에서 보이는지 확인하세요.

Creator Hub를 사용하여 다음을 켜세요. **다중 인스턴스 미리 보기** 을 **데스크톱 클라이언트** 의 하위 메뉴에서 **재생 옵션**, 그런 다음 다음을 클릭하세요. **재생** 버튼을 두 번째로 눌러 두 번째 Decentraland Explorer 창을 엽니다. 두 창 모두 서로 다른 주소로 연결해야 합니다. 장면이 다시 로드되는 동안 동일한 세션은 계속 열려 있습니다. 다른 계정을 선택하려면 두 번째 창에서 로그아웃한 뒤 다시 로그인해야 할 수도 있습니다.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

두 플레이어는 서로를 볼 수 있고, 서로 상호작용할 수 있습니다. 장면에 멀티플레이어 로직이 있다면 예상대로 작동하며 두 플레이어 사이에서 동기화됩니다.

대안으로, 브라우저 URL에 다음을 입력해 두 번째 Decentraland Explorer 창을 열 수 있습니다:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## 이더리움 테스트 네트워크 사용

장면을 테스트하는 동안 실제 MANA나 다른 통화를 전송하는 것을 피하려면, Ethereum Sepolia 테스트 네트워크를 사용하고 대신 가짜 테스트넷 MANA를 전송할 수 있습니다.

테스트 네트워크를 사용하려면 Metamask Chrome 확장 프로그램이 다음을 사용하도록 설정해야 합니다. *Sepolia 테스트 네트워크* 대신 *메인 네트워크*. 또한 Sepolia 블록체인에서 MANA를 보유하고 있어야 하며, Decentraland에서 무료로 획득할 수 있습니다.


> [!NOTE]
> **💡 팁**: 지갑으로 Sepolia MANA를 전송하는 거래를 실행하려면 Sepolia Ether로 가스 수수료를 지불해야 합니다.


이 모드에서 장면을 보며 승인하는 모든 거래는 테스트 네트워크에서만 발생하며, 실제 지갑의 MANA 잔액에는 영향을 주지 않습니다.

테스트 네트워크를 사용하여 장면을 미리 보려면 다음 URL을 브라우저 탭에 붙여넣으세요. 그러면 Decentraland 데스크톱 클라이언트에서 장면이 열립니다:

> decentraland://realm=<http://127.0.0.1:8000\&local-scene=true\&debug=true\&dclenv=zone\&position=0,0>


> [!NOTE]
> **💡 팁**: position 매개변수를 장면의 좌표로 변경하여 장면으로 직접 로드하세요.. 이 모드에서 장면을 보며 승인하는 모든 거래는 테스트 네트워크에서만 발생하며, 실제 지갑의 MANA 잔액에는 영향을 주지 않습니다.


Polygon Testnet에서 거래를 테스트해야 하고 해당 테스트넷에 MANA가 있어야 한다면, Sepolia에서 MANA를 획득한 뒤 해당 네트워크로 스왑해야 합니다. Sepolia MANA를 Polygon Testnet으로 브리지하려면 Sepolia의 Decentraland 계정 페이지를 방문한 다음 Ethereum MANA 쪽에서 ‘swap’을 클릭하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/debugging/debug-in-preview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
