> 비디오 화면과 관리자 도구를 사용하여 라이브 비디오를 씬에 스트리밍합니다.

# 라이브 스트리밍

다음을 사용하세요 **비디오 화면** 스마트 아이템과 함께 **관리 도구** 씬에 라이브 비디오를 스트리밍하는 스마트 아이템입니다.

Decentraland는 씬에 라이브 비디오를 스트리밍하는 여러 방법을 제공합니다:

* **DCL Cast** *(쉬운 모드)*: Decentraland의 무료 스트리밍 웹 앱을 사용해 씬의 다른 플레이어들과 카메라나 화면을 쉽게 공유할 수 있으며, 별도의 스트리밍 소프트웨어를 설정할 필요가 없습니다. 이 모드는 지연 시간이 가장 짧고 설정이 가장 쉽습니다.
* **스트림** *(고급 모드)*: 다음과 같은 스트리밍 소프트웨어를 사용하세요 [OBS](https://obsproject.com/) 를 통해 Decentraland의 스트리밍 인프라로 스트리밍합니다. 이 모드는 화면 레이아웃과 오디오 소스 등 스트림을 더 세밀하게 제어할 수 있습니다.
* **비디오 URL** *(고급 모드)*: URL을 다음 항목에 붙여넣어 자체 스트리밍 인프라를 지정하세요 **비디오 URL** 필드.

<img src="/docs/_remote/9fe1dc304a4f62e468f108b3.webp" alt="스트리밍 방법" width="400">

스트리밍은 Worlds와 Genesis City에서 작동하며, 씬 측에서는 관객 수 제한이 없습니다.

## 씬 구성하기

다음 단계는 DCL Cast와 Stream 방식 모두에 공통됩니다:

1. 추가하세요 **비디오 화면** 스마트 아이템을 씬에.

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)
2. 다음 항목을 추가하세요: **관리 도구** 스마트 아이템을 추가하고 다음을 활성화하세요 **비디오 화면** 섹션입니다. 드롭다운에서 각 화면을 선택하고 관리 UI에서 표시할 친숙한 이름을 지정하세요.

   ![](/docs/_remote/34ed9d039629babf7b485b20.webp)
3. 씬(World 또는 Genesis City)을 게시한 후 관리자 권한이 있는 사용자로 입장하세요.

   ![](/docs/_remote/522b72b559e9bf32de19bcab.webp)

씬이 게시되면 관리자 권한이 있는 사용자로 입장해 스트리밍 설정을 구성할 수 있습니다.


> [!NOTE]
> **💡 팁**: 동일한 비디오를 표시하기 위해 여러 비디오 화면을 추가하는 경우, 하나를 제외한 모든 화면의 소스를 같은 비디오 플레이어를 가리키도록 구성하세요. 참고: [여러 비디오 화면](/docs/creator/content-creator-ko/scene-editor/interactivity/video-screen/#multiple-video-screens) 자세한 내용은.


## DCL Cast(쉬움)

### 앱 액세스 공유하기

게시된 씬에 관리자 사용자로 입장한 다음 관리자 패널을 여세요. 다음을 선택하세요: **비디오** 탭을 연 다음 **DCL Cast** 기능을 선택하세요.

<img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="400">

복사하여 다른 사람과 공유할 수 있는 두 개의 링크가 표시됩니다.

* **발표자 캐스트**: 이 링크는 발표자가 자신의 비디오를 씬에 캐스트할 때 사용합니다.

  <div data-gb-custom-block data-tag="hint" data-style="danger" class="hint hint-danger"><p><strong>❗경고</strong>: 스트리밍 링크는 비밀로 취급하고, 신뢰할 수 있는 사람에게만 공유하세요. 필요하면 발표자 사이에 링크를 재설정하세요.</p><p>스트리밍을 마치면 채널을 해제하기 위해 DCL Cast 브라우저 탭을 닫으세요.</p></div>
* **시청자**: 이 링크는 관객이 브라우저나 모바일에서 비디오를 시청할 때 사용합니다. 현재 Decentraland 안에 있지 않은 플레이어에게 유용합니다.

다음을 클릭하세요: **링크 복사** 버튼을 눌러 링크를 클립보드에 복사하세요.

스트리밍할 준비가 되면 다음을 클릭하세요: **활성화** 버튼을 눌러 씬의 관객에게 스트림을 표시하세요.

<img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="스트림 활성화" width="150">

어떤 이유로든 방을 재설정해야 한다면 다음을 클릭하세요: **방 재설정** 버튼을 눌러 새 방을 생성하세요. 현재 스트리밍 중인 모든 사람은 연결이 끊깁니다.

<img src="/docs/_remote/5daf35dba544baf2c6504619.webp" alt="방 재설정" width="150">

### DCL Cast 앱 사용하기

누군가 발표자 링크를 브라우저에 붙여넣으면 다음과 같은 화면이 표시됩니다:

<img src="/docs/_remote/8b74b9e4f571aec33f619bc5.webp" alt="DCL Cast 앱" width="400">

브라우저에서 카메라와 마이크를 공유할 수 있도록 권한을 요청합니다. 스트림에 사용할 다양한 입력 장치도 구성할 수 있습니다.


> [!NOTE]
> **📔 팁**: Google Chrome 또는 Chrome 엔진 기반 브라우저를 사용하세요. 이러한 브라우저는 브라우저 탭에서 비디오와 오디오를 직접 쉽게 공유할 수 있는 기능을 제공합니다.


사용자는 이름을 입력하고(Decentraland 사용자 이름과 일치할 필요 없음) 다음을 클릭할 수 있습니다: **지금 참여** 버튼을 눌러 스트리밍을 시작하세요.

스트리밍이 시작되면 이 앱은 익숙한 여러 화상 회의 앱과 비슷하며, 음소거/음소거 해제, 카메라와 화면 공유, 채팅 인터페이스 버튼이 있습니다.

채팅은 읽기 전용이며 Decentraland 씬 안의 플레이어가 보낸 모든 메시지를 수신합니다. 다른 장치에서 스트리밍하더라도 관객과 소통하기에 좋습니다.

<img src="/docs/_remote/66b24803959f06d37231b10c.webp" alt="DCL Cast 앱" width="700">

다음의  **참여자** 탭에서 세 가지 목록을 볼 수 있습니다:

* **발표자**: 현재 씬으로 스트리밍 중인 사람들입니다.
* **시청자**: 현재 브라우저에서 스트림을 시청 중인 사람들입니다.
* **인월드 참여자**: 현재 씬 안에서 인월드로 스트림을 보고 있는 플레이어들입니다.

<img src="/docs/_remote/1ff69ac3627fa7cbbad58fca.webp" alt="참여자 탭" width="200">

DCL Cast 세션에 여러 발표자가 있는 경우, 인월드 플레이어는 모든 발표자의 목소리를 듣게 되며, 비디오는 현재 소리를 내고 있는 사람을 자동으로 표시하도록 전환되어 항상 누가 말하는지 보여줍니다.

이 기본 동작을 변경하려면:

* 다음을 클릭하세요: **발표자** 씬 관리자 패널의 버튼

<img src="/docs/_remote/2e5e3dd8ae5ce493e7c7cc13.webp" alt="참여자 탭" width="200">

* 발표자 중 한 명을 선택하고 보여줄 소스를 선택하세요(해당 발표자의 카메라 또는 화면)

<img src="/docs/_remote/7e4c6bd675830ab1e698bcfc.webp" alt="참여자 탭" width="200">

이렇게 하면 다른 발표자가 말하더라도 이 소스가 항상 화면에 표시됩니다.


> [!NOTE]
> **📔 팁**: 인월드에서 스트림을 함께 보고 있다면 DCL Cast 앱과 Decentraland 씬 양쪽에서 오디오가 반복되어 울려 퍼지는 것이 거슬릴 수 있습니다. 화면 왼쪽 아래의 스피커 아이콘을 전환해 DCL Cast 앱의 모든 오디오를 쉽게 음소거할 수 있습니다
> 
> <img src="/docs/_remote/f1c102f46a10285862b60d5a.webp" alt="Participants tab" data-size="original">
> 
> 그렇지 않으면 Decentraland 설정에서 오디오를 음소거할 수 있습니다.


### 프레젠테이션 공유

슬라이드 프레젠테이션의 내용을 이미지의 대체 소스로 공유할 수도 있습니다.

* 인월드에서 다음을 클릭하세요: **프레젠테이션 공유** DCL Cast 탭의 인월드 버튼. DCL Cast 앱에서 다음 옆의 드롭다운을 클릭하세요: **화면 공유** 버튼을 누르고 다음을 선택하세요: **프레젠테이션 공유**.

<img src="/docs/_remote/021196a0a453b918bd70cad9.webp" alt="참여자 탭" width="200">

* Google Slides 링크, Drive에 호스팅된 .pdf 링크 또는 유사한 출처의 링크를 붙여넣거나 .pdf 파일을 업로드하세요.

이제 프레젠테이션은 화면에 표시할 수 있는 소스가 되며, 모든 발표자의 목소리는 계속 들립니다.


> [!WARNING]
> **📔 참고**: 프레젠테이션 파일은 100MB 이하여야 합니다. Google Slides 프레젠테이션은 다음으로 설정해야 합니다: *공개*.
> 
> DCL Cast 세션에서는 동시에 하나의 프레젠테이션만 활성 상태일 수 있습니다.


그다음 슬라이드를 전환하거나, DCL Cast 앱과 인월드의 Admin Tools UI 둘 다에 있는 버튼을 눌러 슬라이드에 포함된 동영상을 재생하거나 일시정지할 수 있습니다.

## 스트림(고급)

씬에서 라이브 스트리밍 기능을 사용하려면 RTMP 엔드포인트로 출력할 수 있는 스트리밍 소프트웨어를 설치해야 합니다(예: [OBS](https://obsproject.com/), [XSplit](https://www.xsplit.com/), [StreamYard](https://streamyard.com/)).


> [!WARNING]
> **경고**: 라이브 스트리밍은 싱글 플레이어 월드에서 지원되지 않습니다. 월드에 **싱글 플레이어** 가 월드 설정에서 활성화되어 있거나( `fixedAdapter` 다음으로 설정되어 있거나 `"offline:offline"` 에서 `scene.json`), 스트리밍을 설정하기 전에 비활성화하세요. 스트리밍 기능은 통신 레이어에 의존하며, 이는 싱글 플레이어 모드에서 비활성화됩니다.


### 스트림 자격 증명 받기

1. 씬에서 Admin UI를 여세요(오른쪽 상단 아이콘).

   ![](/docs/_remote/3a78785f1b44cd50b0963098.webp)
2. 다음에서 **비디오** 탭으로 전환한 다음 **스트림** 그리고 다음을 클릭하세요: **스트림 키 받기**.

   ![](/docs/_remote/548a347a2432d40b40d20705.webp)
3. 다음을 복사하세요: **RTMP 서버** 및 **스트림 키** 를 스트리밍 소프트웨어에 입력하세요.

   ![OBS configuration](/docs/_remote/683eec84be8a03a78e4ed799.webp)


> [!CAUTION]
> **❗경고**: 한 번에 한 사람만 씬으로 스트리밍할 수 있습니다. 스트리밍을 마치면 다음을 클릭하세요: **스트리밍 중지** 를 소프트웨어에서 눌러 채널을 해제하세요.


### 스트림 시작 및 제어

1. 소프트웨어에서 스트리밍을 시작하세요.
2. Admin UI에서 다음을 클릭하세요: **활성화** 를 눌러 씬에 스트림을 표시하세요.

   <img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="스트림 활성화" width="100">

### 스트림 키

스트림 키는 씬마다 생성되며 4일(96시간) 동안 유효합니다. 단일 라이브 세션은 최대 4시간 연속으로 실행될 수 있습니다.

![](/docs/_remote/8fdba6b9787070f6aacf6827.webp)

* 클릭 **스트림 키 재설정** 를 눌러 현재 키를 폐기하고 새 키를 발급하세요. 진행 중인 스트림은 중지됩니다.
* 각 씬에는 고유한 스트리밍 주소와 키가 있습니다. 관리자는 이 키를 외부 스트리머와 공유할 수 있습니다.
* 씬당 동시에 하나의 스트림만 활성화할 수 있으며, 새 스트림을 시작하면 현재 스트림이 덮어써집니다.


> [!CAUTION]
> **❗경고**: 스트림 키는 비밀로 취급하세요. 필요하면 발표자 사이에 키를 재설정하세요.


## URL에서 스트리밍(고급)

비디오 화면을 다음 옵션을 사용하도록 구성하여 스트리밍할 수도 있습니다 **비디오 URL** 그리고 스트림 URL을 붙여넣으세요.

대부분의 인기 비디오 스트리밍 사이트의 비디오를 가리키는 URL을 붙여넣을 수 있어야 합니다. 이러한 플랫폼의 서비스 약관을 유의하세요.

로컬 컴퓨터에 있는 비디오 파일에서 스트리밍하려면, 이 비디오를 공개 Google Drive에 업로드한 뒤 링크를 붙여넣는 것이 가장 쉽습니다.

* URL은 다음이어야 합니다 `https`로 설정하는 것입니다. 참조: [외부 스트리밍 정보](/docs/creator/content-creator-ko/sdk7/media/video-playing/#about-external-streaming).
* 추천 제공업체에는 다음이 포함됩니다 [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) 및 [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback).
* 인코더 설정 팁: [성공적인 스트리밍을 위한 OBS 설정](/docs/creator/content-creator-ko/sdk7/media/video-playing/#setting-up-obs-for-successful-streaming).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/operate-live/live-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
