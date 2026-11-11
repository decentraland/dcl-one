> Scene Editor 사용 방법

# Scene Editor 필수 사항

Scene Editor의 UI는 서로 다른 목적을 가진 몇 개의 섹션으로 나뉩니다.

![](/docs/_remote/ee9a6732ea7a70cc625e7341.webp)

* **캔버스**: 항목을 직접 조작하고 장면이 어떻게 보이는지 확인하세요.
* **엔티티 트리**: 장면의 모든 항목과 그 계층 구조의 목록이 들어 있습니다.
* **속성**: 현재 선택한 항목에 대한 세부 정보를 표시합니다.
* **리소스**: 사용할 수 있는 리소스를 보여줍니다.

## 이동하기

씬 에디터를 둘러보려면:

* 다음을 사용하세요 **W** 및 **S** 를 눌러 가까워지거나 멀어집니다. 마우스 스크롤 휠을 사용하거나, **+** 및 **-** 키를
* 다음을 사용하세요 **하나는** 및 **D** 를 눌러 좌우로 이동합니다.
* 다음을 사용하세요 **Q** 및 **E** 를 눌러 위아래로 이동합니다.
* 다음을 클릭하세요: **오른쪽 마우스 버튼** 을 누른 채 드래그하여 카메라를 회전합니다.
* 눌러 **스페이스바** 를 눌러 카메라를 기본 위치로 되돌립니다.
* 다음을 사용하세요 **왼쪽 마우스 버튼** 을 클릭하여 항목을 선택하고, 항목을 이동합니다.

## 모바일에서 미리보기

해당 **재생 옵션** 옆의 메뉴 **재생** 버튼에는 **모바일용 QR 코드 표시** 옵션이 있습니다. 같은 Wi‑Fi 네트워크에 연결된 휴대폰으로 QR 코드를 스캔하여 장면을 [Decentraland 모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/). 이는 모바일 플레이어용 UI, 조작, 성능을 검증하는 권장 방법입니다. 자세한 내용은 [모바일에서 미리보기](/docs/creator/content-creator-ko/build-for-mobile/develop/preview-on-mobile/) 에서 전체 가이드를 확인하세요.

## 지면 설정

장면의 지면에는 다양한 텍스처를 사용할 수 있습니다. 이러한 텍스처는 항목 메뉴의 각기 다른 테마 에셋 팩에서 찾을 수 있습니다.

유형이 **지면** 에는 페인트 통 아이콘이 있습니다. 이 항목 중 하나를 장면으로 끌어오면 장면의 모든 지면이 이 항목의 복사본으로 덮입니다.

![](/docs/_remote/e120bb99677fe9cb301458fb.webp)

또한 다음을 누른 채로 항목의 단일 복사본을 추가할 수 있습니다 **Shift** 지면을 장면으로 끌어오는 동안.

![](/docs/_remote/df027a655d783fcc8276005c.webp)

지면 항목 모음은 [엔티티 트리](#the-entity-tree) 의 폴더 안에 나타납니다. 각각은 실수로 선택되지 않도록 잠겨 있습니다. [토글 해제](#lock-or-hide-items) 하여 항목을 이동하거나 편집하세요.

## 항목 추가

아래 메뉴에서 테마별 에셋 팩 카테고리를 탐색해 씬에 배치할 수 있는 다양한 항목을 찾아보세요.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

검색 상자도 사용할 수 있습니다. 에셋 팩 안에 있을 때는 검색이 해당 에셋 팩 안에서만 수행된다는 점에 유의하세요.

항목을 배치하려면 에셋 팩 메뉴에서 항목을 클릭한 채 캔버스의 씬 위치로 끌어다 놓으세요.

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)


> [!NOTE]
> **💡 팁**: 장면의 항목을 추가, 이동 또는 속성을 편집할 때마다 변경 사항이 자동으로 저장됩니다.


항목을 복제하려면 선택한 다음 **Ctrl + C** 을 사용하고, 그다음 **Ctrl + V**. 또한 항목을 [엔티티 트리](#the-entity-tree) 에서 찾아 마우스 오른쪽 버튼을 클릭하고 다음 옵션을 선택할 수도 있습니다 **복제**. 새 항목은 원본과 정확히 겹쳐서 배치됩니다.

장면에서 항목을 삭제하려면 선택한 다음 *Delete* 키를 누르세요.

참조 [항목 가져오기](/docs/creator/content-creator-ko/scene-editor/build/import-items/) 디스크에서 사용자 지정 3D 모델을 추가할 때 사용합니다.


> [!WARNING]
> **📔 참고**: 3D 모델을 장면으로 끌어오면 프로젝트 폴더로 다운로드되며, 삭제하더라도 그대로 남아 있습니다. 이렇게 사용되지 않는 모델은 장면 크기를 늘릴 수 있습니다.
> 
> 다음을 엽니다 **로컬 에셋** 탭에서 사용되지 않는 모델을 삭제하세요.


## 항목 배치



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/cNl02PFPdcQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=cNl02PFPdcQ>">Watch on YouTube</a></p>



다음을 사용해 클릭하고 **왼쪽 마우스 버튼** 선택한 항목을 드래그하면 지면 수준에서 장면 안을 자유롭게 이동할 수 있습니다.

상단 메뉴의 도구도 사용할 수 있습니다:

![](/docs/_remote/c6834cebdebce2e0fb6608ba.webp)

* **이동 도구**: 각 화살표를 클릭해 드래그하면 한 번에 하나의 축으로 항목을 이동할 수 있습니다. 이 도구를 사용하면 지면보다 위에 물체를 배치할 수도 있습니다.
* **회전 도구**: 항목 주위의 각 고리를 클릭해 드래그하면 한 번에 하나의 축으로 항목을 회전할 수 있습니다.
* **크기 조절 도구**: 기즈모의 중앙을 클릭해 안팎으로 드래그하면 확대할 수 있습니다. 이 도구를 사용하면 한 축으로만 항목을 늘려 비율을 변경할 수도 있습니다. 이렇게 하려면 기즈모의 축 중 하나를 클릭해 드래그하세요.

![](/docs/_remote/11bce750b05bad2b59a8c3b8.webp)

항목을 이동, 회전, 크기 조절할 때 더 정밀하게 하려면 다음을 길게 누르세요 *Shift* 키를 누른 채로 조정하면 됩니다. 이렇게 하면 그리드에 스냅되는 것을 방지할 수 있습니다.

이동 단위와 기타 설정을 변경하려면 도구 오른쪽의 아래쪽 화살표를 클릭하세요. 다음 설정을 사용할 수 있습니다:

* **스냅**: 그리드를 켜거나 끕니다. 꺼져 있을 때는 다음의 동작이 **Shift** 반대로 바뀝니다. 기본적으로 그리드를 따르지 않으며, 다음을 누른 경우에만 따릅니다 **Shift**.
  * **위치**: 다음이 켜져 있을 때 이동 증가량의 크기(미터 단위)입니다 **스냅** 가 켜져 있을 때.
  * **회전**: 다음이 켜져 있을 때 회전 증가량의 크기(도 단위)입니다 **스냅** 가 켜져 있을 때.
  * **크기 조정**: 다음이 켜져 있을 때 크기 조절 증가량의 크기입니다 **스냅** 가 켜져 있을 때.
* **월드에 정렬**: 기즈모에서 사용하는 축을 지정하는 단일 체크박스입니다. 체크하면 이동 및 회전 도구의 축이 항상 월드에 맞춰지며, 객체의 방향에 따라 바뀌지 않습니다. 체크 해제하면 객체의 방향에 맞춰집니다.

여러 항목을 동시에 선택하려면 다음을 길게 누른 채로 *제어* 항목을 선택하세요. 그러면 한 번의 작업으로 모두 이동, 회전, 크기 조절, 복제 또는 삭제할 수 있습니다.

### 정확한 수치를 입력

드래그가 충분히 정밀하지 않을 때는 대신 수치를 입력하세요:

1. 도구 키를 누르세요: **M** 또는 **G** 은 이동, **R** 은 회전, **X** 은 크기 조절.
2. 눌러 **X**, **Y** 또는 **Z** 은 축을 선택합니다. 크기 조절에는 필요하지 않습니다. 축이 없으면 모든 축이 같은 비율로 조절됩니다.
3. 숫자를 입력하세요. 다음을 사용하세요 **.** 를 소수점에, **-** 를 사용해 양수와 음수를 전환하세요.
4. 눌러 **Enter** 를 눌러 적용하거나 **Esc** 를 눌러 취소하세요. **Backspace** 는 입력한 내용을 지웁니다.

캔버스 하단의 칩은 입력 중인 내용을 표시하고, 아직 필요한 것이 무엇인지 알려주며, 적용된 내용을 확인해 줍니다.

몇 가지 예:

* **R** **X** **1** **5** **Enter** 선택 항목을 X축 기준으로 15도 회전합니다.
* **X** **2** **Enter** 선택 항목을 두 배로 크게 만듭니다.
* **M** **Y** **-** **3** **Enter** 아래로 3미터 이동합니다.

알아두면 좋은 몇 가지 사항:

* 수치는 절대 좌표가 아니라 항상 현재 선택 위치를 기준으로 합니다.
* 입력한 값은 스냅 그리드의 영향을 받지 않습니다.
* 여러 항목이 선택된 경우, 모든 항목이 공통 중심을 기준으로 함께 이동, 회전 또는 크기 조절되며, 하나의 실행 취소 단계로 처리됩니다.
* 다음을 누르면 **M**, **G** 또는 **R** 두 번째에도 해당 기즈모가 꺼집니다. **X** 는 그렇지 않습니다. 입력 중일 때는 X축을 의미하기 때문입니다.


> [!NOTE]
> **💡 팁**: 캔버스 오른쪽 아래의 키보드 아이콘을 누르면 모든 단축키 목록이 열립니다.


## 스마트 아이템

스마트 항목은 내장된 상호작용 동작이 포함된 특별한 항목입니다. 자세한 내용은 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 자세한 내용은.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

## 엔티티 트리

왼쪽 여백에서 장면의 모든 엔티티가 표시된 트리 구조를 볼 수 있습니다. 여기에는 추가한 모든 항목과 몇 가지 기본 엔티티가 포함됩니다.


> [!NOTE]
> **💡 팁**: 장면의 모든 것은 엔티티이며, 이는 장면의 기본 구성 요소입니다. 항목은 최소한 위치와 보이는 형태를 가진 엔티티입니다.


장면의 3D 보기에서 항목을 클릭해 선택하는 대신, 트리 보기에서 선택할 수 있습니다. 엔티티를 마우스 오른쪽 버튼으로 클릭하면 더 많은 옵션이 표시됩니다. 이름 변경, 삭제, 복제는 물론 자식 엔티티 만들기, 사용자 지정 항목으로 전환하기, 엔티티에 컴포넌트 추가를 할 수 있습니다.

![Bird Fountain 엔티티에서 마우스 오른쪽 버튼 메뉴가 열린 엔티티 트리로, Rename, Add child, Duplicate, Delete, Create Custom Item 및 Add component가 나열되어 있습니다.](/docs/_remote/42bfc23b5b7dbda3a5127c7a.webp)

### 엔티티 트리 검색

엔티티 트리 상단의 검색 상자를 사용하면 이름으로 엔티티를 찾을 수 있습니다. 엔티티 이름의 일부를 입력하면 트리가 필터링되어 일치하는 엔티티와 해당 부모 계층만 표시되므로, 각 일치 항목이 트리의 어디에 있는지 확인할 수 있습니다. 검색은 대소문자를 구분하지 않습니다. 일치 항목의 부모 엔티티는 자동으로 확장되어 모든 결과가 보이게 됩니다.

눌러 **Escape** 또는 지우기 아이콘을 클릭해 필터를 제거하고 전체 트리로 돌아가세요. 엔티티가 선택되어 있으면 검색을 지울 때 선택한 엔티티로 다시 스크롤되어 현재 위치를 잃지 않습니다.

![검색어 fount로 필터링된 엔티티 트리: Camera, Scene 및 Bird Fountain 엔티티만 보이며, 일치를 표시하기 위해 Scene 부모가 확장되어 있습니다.](/docs/_remote/462138368861e2d4fd3aba3d.webp)

### 엔티티 계층 구조

엔티티는 원하는 만큼 많은 수준을 가질 수 있는 계층 구조를 따릅니다. 트리에서 한 항목을 다른 항목 위로 끌어 두 엔티티 사이에 부모-자식 관계를 설정하세요. 자식 엔티티는 부모의 위치를 상속하므로 부모가 이동하면 자식도 함께 이동합니다. 이는 장면을 만들 때 유용할 수 있습니다. 예를 들어 유리잔과 접시를 테이블의 자식으로 설정한 뒤, 다른 것을 조정할 필요 없이 테이블만 이동할 수 있습니다. 또한 장면에서 상호작용할 때 항목들이 함께 움직이도록 하는 데도 중요할 수 있습니다.

![](/docs/_remote/2ba2f0d9e5656796ad572f00.webp)

엔티티의 자식 항목을 최소화하거나 확장해 보기를 단순하게 유지할 수도 있습니다. 이 작업은 장면에는 영향을 주지 않습니다.

### 특수 엔티티

장면에는 엔티티 트리에서 볼 수 있는 몇 가지 특수 엔티티가 포함되어 있습니다.

* **씬**: 이는 루트 엔티티를 의미하며, 장면에 추가하는 모든 것은 이 엔티티의 자식입니다. 이를 열어 다음을 볼 수 있습니다 [장면 설정](#scene-settings).
* **플레이어**: 플레이어의 아바타입니다. 이 엔티티에 게임플레이 메커니즘을 바꿀 수 있는 특수 컴포넌트를 추가할 수 있습니다. 또한 다른 엔티티를 아바타의 자식으로 끌어다 놓을 수도 있습니다. 엔티티가 아바타의 자식이면 위치가 플레이어에 고정됩니다. 예를 들어 플레이어의 머리 위에 떠 있는 표시를 추가해 플레이어를 따라다니게 할 때 사용할 수 있습니다.
* **카메라**: 플레이어의 카메라입니다. 다른 엔티티를 카메라의 자식으로 끌어다 놓을 수 있습니다. 엔티티가 카메라의 자식이면 화면에 고정된 위치를 갖습니다. 예를 들어 슈팅 게임에서 플레이어가 위아래를 향하더라도 항상 보이는 총을 표시할 때 사용할 수 있습니다.

### 항목 잠금 또는 숨기기

항목을 실수로 선택하거나 이동하지 않도록 가끔 잠그면 편리할 수 있습니다. 이는 지면이나 건물 같은 배경 항목에 특히 유용합니다. 항목을 잠그려면 왼쪽의 엔티티 트리에서 해당 항목을 찾은 다음 마우스를 올리고 잠금 아이콘을 선택하세요. 같은 아이콘으로 이 동작을 켜고 끌 수 있습니다.

다른 항목을 배치하는 동안 시야를 가릴 수 있는 항목은 숨기고 싶을 수도 있습니다. 이는 실내를 작업할 때 지붕이나 건물을 숨기는 데 특히 유용합니다. 숨긴 항목은 Scene Editor의 캔버스 창에서만 숨겨지며, 장면에 들어오는 플레이어에게는 숨겨지지 않습니다. 항목을 숨기려면 왼쪽의 엔티티 트리에서 해당 항목을 찾은 다음 마우스를 올리고 눈 아이콘을 선택하세요. 같은 아이콘으로 이 동작을 켜고 끌 수 있습니다.

![](/docs/_remote/1360d2763c98c1ac75eca0d4.webp)

## 속성 패널

캔버스나 엔티티 트리에서 항목을 클릭해 선택하세요. 그러면 화면 오른쪽의 속성 패널에 해당 컴포넌트가 표시됩니다. 항목마다 서로 다른 컴포넌트가 있으며, 각 컴포넌트는 특정 설정을 표시합니다.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

대부분의 비상호작용 항목에는 다음 컴포넌트가 있습니다:

* **Transform**: 항목의 위치, 회전, 크기를 설정합니다.
* **GLTF**: 불러올 3D 모델입니다.

[스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 에는 다른 컴포넌트를 포함할 수 있습니다.

참조 [컴포넌트](/docs/creator/content-creator-ko/scene-editor/build/components/) 자세히 알아보세요.

## 장면 제한

Decentraland 장면은 서로 나란히 실행될 수 있도록 특정 제한을 따라야 합니다. 장면의 파셀 수에 비례하는 재질, 텍스처, 삼각형 등의 최대 수가 있습니다. 자세한 내용은 [씬 제한 사항](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/) 자세한 내용은.

장면의 콘텐츠가 이러한 제한 중 하나라도 초과하면 Scene Editor가 왼쪽 아래 모서리에 이를 알립니다.

![](/docs/_remote/ab87e37107598ee524831639.webp)

이 메뉴를 펼쳐 세부 정보를 볼 수 있습니다.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 팁**: Decentraland World를 만들고 있다면 언제든지 다음을 늘릴 수 있습니다 [장면 크기](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/#layout) 를 늘려 한도를 높일 수 있습니다.


Decentraland 장면의 콘텐츠는 인접한 파셀로 넘치지 않도록 해야 합니다. 장면의 모델 일부가 한계를 넘어가면 장면 미리보기를 열었을 때 해당 부분이 잘려 보입니다. Scene Editor는 모델 전체를 빨간색으로 표시하지만, 실제로는 장면 한계를 넘는 모델 부분만 신경 쓰면 됩니다.

![](/docs/_remote/8dcb8f74f41494a20d11ba4d.webp)


> [!NOTE]
> **💡 팁**: 표시하려는 모델이 맞지 않으면 장면 크기를 늘리고 싶을 수 있습니다. 자세한 내용은 [장면 크기](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/#layout) 에서 장면을 확장하세요.


3D 모델의 전체 지오메트리가 장면에 들어맞더라도, 모델의 바운딩 박스가 영역을 벗어나면 빨간색으로 표시될 수 있습니다. 이 경우에는 전체 모델이 올바르게 표시되므로 경고를 무시해도 됩니다. 자세한 내용은 [바운딩 박스](/docs/creator/content-creator-ko/3d/meshes/#bounding-boxes).

## 에셋 정리

더 이상 사용되지 않는 에셋을 제거해 프로젝트를 깔끔하게 유지하세요. 다음을 여세요 **로컬 에셋** 화면 하단의 탭에서 브러시 아이콘을 클릭하세요.

![](/docs/_remote/98ff3e388967f6b98ae40ab1.webp)

대화상자가 열리면 장면의 모든 에셋이 나열되며, 어떤 항목에서도 참조되지 않는 에셋이 강조 표시됩니다.

![](/docs/_remote/a20d113518b454ef45385873.webp)

삭제하려는 에셋을 체크박스로 선택한 다음 **선택 항목 제거** 를 클릭해 영구적으로 제거하세요.

장면에서 항목을 삭제해도 해당 파일은 제거되지 않습니다. 가져온 모델, 텍스처 또는 사운드는 정리할 때까지 프로젝트에 남아 있으므로 사용하지 않는 에셋을 주기적으로 검토하세요.


> [!WARNING]
> **중요:** 장면에 에셋을 참조하는 코드가 포함되어 있으면, 사용 중인 일부 에셋이 사용되지 않는 것으로 표시될 수 있습니다. 이 대화상자는 Creator Hub UI의 컴포넌트에서 참조된 에셋만 감지합니다. 다음을 클릭한 후 **선택 항목 제거**파일이 프로젝트 폴더에서 삭제되며, 이 작업은 영구적이고 되돌릴 수 없습니다.


## 씬 설정

다음을 클릭하세요: **연필 아이콘** 화면 오른쪽 상단에 있습니다. 그러면 편집할 장면 수준 속성들이 열립니다.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

여기에서 제목과 썸네일, 장면 크기, 장면 카테고리 및 연령 등급, 플레이어 스폰 위치, 기능 토글을 포함한 여러 속성을 설정할 수 있습니다.

참조 [씬 설정](/docs/creator/content-creator-ko/scene-editor/configure/scene-settings/).

## 또한 참조

* 참조 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/) 씬에 간단한 상호작용을 추가하는 방법은.
* 참조 [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/) 씬의 코드를 편집하는 방법은 다음을 참고하세요.
* 참조 [씬 게시](/docs/creator/content-creator-ko/scene-editor/publish/publish-scene/) 씬을 Decentraland에 게시하는 방법은.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
