> 씬 관리자는 씬에서 일어나는 일을 실시간으로 특별하게 제어할 수 있습니다.

# 씬 관리자

특정 플레이어에게 다음의 특별 역할을 부여하세요 **관리자** 당신의 씬에서.

라이브 이벤트 중에는 관리자가 Decentraland 내부에서 미리 일정을 잡거나 제3자 서비스에 의존하지 않고도 씬에서 일어나는 일을 즉석에서 제어할 수 있습니다. 충분한 군중이 모였을 때 음악을 시작하거나, 때가 되었다고 느껴질 때 конфetti를 뿌리거나 우주선을 나타나게 할 수 있습니다.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/efjJN7Jr7Qo" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=efjJN7Jr7Qo>">Watch on YouTube</a></p>



씬 관리자가 당신의 씬을 방문하면, 화면 오른쪽 위에 관리자만 볼 수 있는 특별한 관리자 패널이 표시됩니다. 이 패널은 상단의 아이콘 탭을 사용해 섹션(비디오, 공지사항, 스마트 아이템, 권한) 사이를 전환합니다. 이 UI를 통해 비디오나 라이브 스트림을 재생하고, 공지사항을 보내고, 플레이어를 차단하거나, 이렇게 활성화되도록 설정된 스마트 아이템을 활성화할 수 있습니다. 이러한 동작은 관리자가 같은 comms island에 연결되어 있는 씬의 다른 모든 플레이어에게도 보입니다.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

## 관리자 설정

관리자를 지정하려면 다음을 추가해야 합니다: **관리 도구** 스마트 아이템을 씬에.

![](/docs/_remote/360f0190e175f35ce4c17c72.webp)


> [!WARNING]
> **📔 참고**: 씬을 최신 종속성으로 업데이트하세요.
> 
> <img src="/docs/_remote/83f4693cc61cdbab26246a7d.webp" alt="" data-size="original">


씬을 개발하고 로컬에서 테스트하는 동안에는 항상 관리자로 간주됩니다. 씬이 게시되면, 해당 씬에 대한 게시 권한이 있는 사람도 자동으로 관리자입니다. 여기에는 다음이 포함됩니다:

* 씬이 게시된 LAND 구획 또는 World NAME의 소유자
* 다음을 부여받은 사람은 누구나 **운영자 권한** 이 구획 또는 이름에 대해. 자세한 내용은 [권한 부여](/docs/player/marketplace/land-manager/#give-permissions).
* 해당 땅을 임대 중인 모든 사용자. 자세한 내용은 [임대](/docs/player/marketplace/rentals/).

게시 권한은 없지만 씬에서 라이브 운영을 할 수 있는 추가 관리자를 지정하려면:

1. 씬을 게시하고 관리자로 라이브 버전을 방문하세요
2. 다음을 엽니다 **권한** 탭(관리자 패널 상단의 아이콘 탭을 통해).

   ![](/docs/_remote/ed908a8f1e3f969689af965f.webp)
3. 추가하려는 사람의 지갑 주소를 다음 옆에 입력하세요 **관리자 추가** 그리고 다음을 클릭하세요: **추가**.

다음을 클릭하면 씬에서 누가 관리자인지 확인할 수 있습니다: **관리자 목록 보기** 버튼. 이 화면에서 또한 **제거** 사람들을 관리자 목록에서 제거할 수 있습니다.

![](/docs/_remote/983d682c49d98b1ce3fb7227.webp)


> [!WARNING]
> **📔 참고**: 다음을 통해 목록에 수동으로 추가된 플레이어에게서만 관리자 역할을 제거할 수 있습니다: **권한 및 관리** 탭. 씬의 소유자, 운영자 또는 임차인은 이 목록에 표시되지만 이 UI에서는 관리자 역할을 제거할 수 없습니다. 운영자의 관리자 역할을 제거하려면 먼저 운영자 역할을 제거해야 합니다.


관리자 플레이어가 씬에 들어오면, 오른쪽 위 모서리에 특별한 UI가 표시됩니다. 관리자 권한이 없는 플레이어는 이 UI를 보지 못합니다.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

### 코드로 관리자 상태 확인

코드를 사용해서 플레이어가 씬의 관리자인지 알 수도 있습니다. 이를 통해 특정 엔티티와의 상호작용 허용처럼 플레이어에게 다른 동작을 제공할 수 있는지 여부를 결정할 수 있습니다.

```ts
import { isAdmin } from "@dcl/asset-packs/dist/admin";

async function onPlayerSpawn() {
  const isAdminUser = await isAdmin();
  if (isAdminUser) {
    // 관리자 전용 UI를 표시하고, 무대로 텔레포트하고, 엔티티를 표시하는 등.
  }
}
```


> [!NOTE]
> **💡 팁**: 비동기 함수에 대한 자세한 내용은 다음을 참조하세요 [비동기 함수](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/).


## 비디오 재생

관리자가 가장 자주 하는 작업 중 하나는 비디오를 재생하는 것입니다. 관리자 패널에는 비디오와 관련된 모든 것을 제어할 수 있는 비디오 플레이어 섹션이 포함되어 있습니다.

이를 활성화하려면 다음을 추가해야 합니다: **비디오 플레이어** 씬에 스마트 아이템을 추가하고 Admin Tools 스마트 아이템에 연결하세요.

1. 추가하세요 **비디오 플레이어** 씬에 스마트 아이템 추가

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

   참조 [비디오 화면](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/) 기본 미디어 소스 및 Video Player 스마트 아이템의 다른 설정을 구성하는 방법에 대한 자세한 내용은 다음을 참조하세요. 이러한 구성 대부분은 씬이 실행 중일 때 관리자가 재정의할 수 있습니다.

   <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 참고</strong>: 관리자는 SDK 코드로 추가된 화면이 아니라 Video Screen 스마트 아이템에서 재생되는 비디오만 관리할 수 있습니다.</p><p>원하는 만큼 비디오 화면을 포함할 수 있습니다. 일반적으로 동시에 서로 다른 비디오가 하나 이상 재생되지 않도록 하세요. 성능이 크게 저하됩니다.</p></div>
2. Admin Tools 스마트 아이템을 열고 다음이 활성화되어 있는지 확인하세요: **비디오 화면** 이 섹션을 표시하려면 체크박스가 활성화되어 있어야 합니다. 그런 다음 드롭다운 목록에서 화면을 선택하고 관리자 UI에 표시할 친숙한 이름을 지정하세요. 원하는 만큼 Video Screen을 추가할 수 있으며, 각 화면은 개별적으로 제어됩니다.

   <img src="/docs/_remote/34ed9d039629babf7b485b20.webp" alt="씬 이름" width="300">

위 구성이 완료되면, 씬의 관리자 사용자는 관리자 패널을 열고 상단의 아이콘 탭을 통해 비디오 탭을 선택하여 이 비디오 화면들을 제어할 수 있습니다.

<img src="/docs/_remote/48468cefefacba6545b47b45.webp" alt="씬 이름" width="300">

씬에 서로 독립적인 비디오 화면이 여러 개 있는 경우, **화면** 드롭다운에서 제어할 비디오 화면을 선택할 수 있습니다. 목록에는 Admin Tools 스마트 아이템 설정에서 각 비디오 화면에 지정한 이름이 표시됩니다.


> [!NOTE]
> **💡 팁**: 하나처럼 제어할 수 있는 여러 화면에 같은 비디오를 표시하려면 다음을 참조하세요 [여러 비디오 화면](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/#multiple-video-screens).


## 미디어 소스

비디오 섹션 상단의 분할 제어를 사용하면 세 가지 미디어 소스 사이를 전환할 수 있습니다. 현재 활성 소스는 캡슐 형태의 레이블로 표시됩니다. 탭을 전환하면 해당 소스의 설정만 표시되며, 다음을 클릭하기 전까지 현재 재생 중인 내용은 중단되지 않습니다 **활성화**.

* **비디오 URL**: URL에서 비디오를 재생합니다. 비디오 URL을 필드에 붙여넣고 다음을 클릭하세요 **활성화**. 선택한 화면에서 모든 플레이어에게 비디오가 재생되기 시작합니다. 정지, 일시 중지, 다시 시작, 볼륨 조절도 할 수 있습니다.

  <img src="/docs/_remote/77179ebe47d00636f626ed06.webp" alt="씬 이름" width="300">

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 참고</strong>: 모든 비디오 URL이 작동하는 것은 아닙니다. 일부 사이트의 비디오는 콘텐츠에 대해 엄격한 정책을 가지고 있어 Decentraland에서의 접근을 차단합니다. 다음을 참조하세요 <a href="/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/#streaming-from-other-sources">다른 소스에서 스트리밍</a> Decentraland에서 재생할 수 있는 것과 없는 것에 대한 자세한 내용은 다음을 참조하세요.</p></div>
* **DCL Cast**: Decentraland의 무료 스트리밍 웹 앱을 사용하면 스트리밍 소프트웨어를 설정하지 않고도 자신의 카메라나 화면을 씬의 다른 플레이어와 쉽게 공유할 수 있습니다. 프레젠테이션이 활성화되면 프레젠테이션 제어(이전, 다음, 비디오 재생, 정지)가 인라인으로 표시됩니다.

  <img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="300">
* **스트림**: Decentraland의 무료 스트리밍 인프라와 OBS 또는 StreamYard 같은 스트리밍 소프트웨어를 사용해 라이브 스트림을 재생합니다.

  <img src="/docs/_remote/79d537ec46382a7153d0fb2f.webp" alt="씬 이름" width="300">

  참조 [라이브 스트리밍](/docs/creator/content-creator-ko/scene-editor/operate-live/live-streaming/) 라이브 스트림 설정 방법에 대한 자세한 내용은 다음을 참조하세요.

각 미디어 소스 섹션에는 다음이 포함됩니다: **볼륨 슬라이더** 이를 통해 볼륨을 0%(음소거)에서 100%까지 설정할 수 있습니다.

![](/docs/_remote/6fa0aca94ae7997a1988bcad.webp)

## 공지사항

다음에서 **공지사항** 관리자 패널의 탭에서 관리자는 씬의 모든 플레이어에게 보이는 메시지를 작성할 수 있습니다. 이런 메시지는 관리자만 보낼 수 있으므로, 다른 플레이어들은 자신이 관리자라고 주장하는 누군가가 채팅에 남긴 메시지보다 더 신뢰할 수 있는 것으로 인식할 것입니다.

관리자 패널의 공지사항 탭을 선택하세요. 메시지를 작성하고 다음을 클릭하세요 **공유**. 메시지는 최대 90자까지 가능합니다. 전송 후 입력 필드는 자동으로 지워지므로 바로 새 메시지를 입력할 수 있습니다.

![](/docs/_remote/a7a34d2c60e405a1f61bb7f6.webp)

## 플레이어 차단

다음을 선택하여 씬에서 플레이어를 차단할 수 있습니다: **권한** 관리자 패널의 탭에서 차단하려는 플레이어의 이름 또는 지갑 주소를 입력한 다음 다음을 클릭하세요: **차단** 버튼이 있습니다.

![](/docs/_remote/ed908a8f1e3f969689af965f.webp)


> [!NOTE]
> **💡 팁**: 플레이어의 지갑 주소를 얻으려면 해당 아바타를 클릭해 프로필을 연 다음 다음을 클릭하세요: **클립보드에 복사** 지갑 주소 옆의 버튼.


차단된 플레이어는 당신의 씬을 불러오거나 그 안의 어떤 콘텐츠와도 상호작용할 수 없습니다. 다른 플레이어들은 씬에서 그들을 보지 못하며, 그들의 채팅 메시지도 읽을 수 없습니다.


> [!WARNING]
> **📔 참고**: 차단의 효과는 즉시 적용되며 영구적입니다. 플레이어가 한 번 차단되면 차단 해제 전까지 계속 차단 상태로 남아 있습니다. 차단된 동안에는 플레이어가 당신의 씬을 보거나 상호작용할 수 없고, 그 안에 있는 동안 Nearby 채널에서 채팅할 수 없으며, 씬의 다른 플레이어들도 그들을 볼 수 없습니다.
> 
> 플레이어가 씬 경계 밖으로 나가면 더 이상 씬의 차단 규칙의 영향을 받지 않으며 차단된 플레이어를 다시 볼 수 있습니다.


클릭 **차단 목록 보기** 현재 차단된 플레이어 목록을 보세요. 이 목록에서 또한 **차단 해제** 플레이어를 차단 해제할 수 있습니다.

## 스마트 아이템 트리거

씬의 모든 스마트 아이템에서 동작을 트리거하려면:

* 씬에 스마트 아이템 추가
* 다음의 설정을 여세요: **관리 도구** Creator Hub의 스마트 아이템
* 다음에서 **스마트 아이템 액션** 섹션에서 드롭다운에서 스마트 아이템을 추가하고, 사용자 지정 이름을 지정한 뒤 기본 동작을 선택하세요

위 설정이 완료되면 관리자는 다음을 열어 동작을 트리거할 수 있습니다: **스마트 아이템입니다** 관리자 패널의 탭에서 드롭다운 목록의 항목을 선택하세요. 그런 다음 해당 항목의 동작 중 하나를 다음에서 선택할 수 있습니다: **동작** 드롭다운에서 선택하고 다음을 클릭하세요: **동작 재생** 이를 트리거합니다. 또한 다음 버튼도 있습니다: **숨기기/표시** 선택한 항목을.

<img src="/docs/_remote/078360766f2efe8ad3431ea0.webp" alt="씬 이름" width="300">

이 목록의 모든 스마트 아이템은 해당 동작이 없어도 표시하거나 숨길 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/operate-live/scene-admin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
