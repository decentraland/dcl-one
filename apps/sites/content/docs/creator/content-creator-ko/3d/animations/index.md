> Decentraland로 가져온 3D 모델에 포함할 수 있는 애니메이션을 만드는 방법을 알아봅니다.

# 애니메이션

애니메이션은 사물에 생명을 불어넣는 예술입니다. 3D 모델에 몇 가지 애니메이션을 추가하는 것보다 씬을 더 생동감 있게 만드는 좋은 방법은 없습니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 유용한 여러 기능이 포함되어 있습니다.


이를 하는 방법은 몇 가지가 있습니다: **오브젝트 애니메이션**, 또는 **리그(스켈레탈 애니메이션)**, 또는 **셰이프 키(모프 타깃 애니메이션)**.

**오브젝트 애니메이션** 은 튀는 공, 도는 지구본, 떠 있는 의자처럼 단순한 모델에 가장 적합하며, 아마추어가 필요하지 않습니다. 오브젝트 애니메이션은 **버텍스 애니메이션**과는 다르다는 점을 알아두는 것이 중요합니다. 오브젝트 애니메이션에서는 모델 전체가 하나로 애니메이션되지만, 버텍스 애니메이션에서는 오브젝트의 각 버텍스를 개별적으로 애니메이션할 수 있습니다.

**셰이프 키(모프 타깃 또는 블렌드 셰이프라고도 함)** 는 **데센트럴랜드의 엔진이 지원하는** 버텍스 애니메이션의 한 형태입니다. 이는 얼굴 표정, 립싱크, 또는 뼈대만으로는 구현하기 어려운 모든 변형에 특히 유용합니다. glTF/GLB 파일에 포함된 셰이프 키 애니메이션은 스켈레탈 또는 오브젝트 애니메이션처럼 월드 안에서 재생됩니다.

사람, 생물, 기계처럼 더 복잡한 모델이라면 **리그**가 필요합니다. 리그는 메시를 움직이고 변형시키는 디지털 스켈레톤일 뿐입니다. 메시를 스켈레톤에 바인딩하는 과정을 스키닝이라고 하며, 여기서 어떤 뼈가 각 버텍스 그룹에 영향을 주는지, 그 영향력이 얼마나 강한지를 정의하여 모델이 가능한 한 가장 좋은 방식으로 변형되도록 합니다.

어느 방법이든, 3D 모델의 모든 애니메이션은 *glTF*  파일 내부에 포함되어야 합니다. 별도의 파일에 있는 애니메이션을 참조할 수 없기 때문입니다.

## 애니메이션 만들기

Blender를 사용해 애니메이션을 만드는 방법은 다음과 같습니다.

### 오브젝트 애니메이션

* 다음 탭이 열려 있는지 확인하고 ***Dope Sheet*** > ***Action Editor*** 을 클릭해 ***New*** 애니메이션 클립을 추가하세요.
* 원하는 대로 이름을 바꾸고 ***Fake User*** (방패 아이콘)를 활성화하여 애니메이션이 저장되도록 하세요.
* 오브젝트를 선택한 뒤 ***I***&#xB97C; 눌러 첫 프레임에 키프레임을 설정하세요. 그런 다음 오브젝트를 이동, 회전 또는 스케일하고, 타임라인의 다른 프레임에서 ***I*** 를 다시 눌러 현재 변형으로 또 다른 키프레임을 설정하세요.
* 애니메이션의 마지막 프레임은 첫 번째 프레임과 같아야 하므로, 첫 프레임을 복사해 마지막 프레임에 붙여넣으면 됩니다.

![](/docs/_remote/da0a0df92981981477fc8dbd.webp)

*애니메이션 클립을 만들고 키프레임을 추가하기.*

### 리그 애니메이션

리그 애니메이션을 위해서는 아마추어가 필요합니다. 직접 하고 싶다면 [리그 만들기](https://www.notion.so/686e6f59a1604585b059f990a36b2d55) 를 참고하여 일관된 아마추어를 설정하는 방법을 확인하세요.

리깅이 완료되고 스키닝이 잘 변형된다면 이제 애니메이션을 시작할 준비가 된 것입니다! 과정은 오브젝트 애니메이션과 매우 비슷하지만, ***오브젝트 모드***&#xC5D0;서 하는 대신 ***포즈 모드***.

* 다음으로 이동하여 ***포즈 모드*** 다음 탭이 열려 있는지 확인하세요. ***Dope Sheet*** > ***Action Editor*** 을 클릭해 ***New*** 애니메이션 클립을 추가하세요.
* 원하는 대로 이름을 바꾸고 ***Fake User*** (방패 아이콘)를 활성화하여 애니메이션이 저장되도록 하세요.
* 마우스를 ***Viewport*** 위에 두고 ***하나는*** 모두 선택한 다음 ***I*** 전체 아마추어에 키프레임을 설정하세요.
* 타임라인의 다른 프레임으로 이동한 뒤 원하는 자세가 나오도록 뼈를 원하는 방식으로 조작하세요. 리그를 어떻게 설정했는지에 따라 뼈의 위치, 회전, 스케일을 바꿀 수 있습니다.
* 좋은 결과가 나오면 전체 아마추어에 또 다른 키프레임을 설정하세요. 애니메이션을 반복 재생할 예정이라면 첫 프레임과 마지막 프레임이 같도록 유지하세요.

![](/docs/_remote/aa4ecd75ce5fa54f9c60066e.webp)

애니메이션 과정이 처음이라면, 초보자를 위한 유용한 팁이 담긴 이 튜토리얼을 확인해 보세요:

[![Video Preview](/docs/_remote/629c65271f6a14386ffd42f7.webp)](https://youtu.be/-iWslh4uQIk)

## 스킨드 애니메이션 vs 트랜스폼 애니메이션

**트랜스폼 애니메이션은 애니메이션 표시에 필요한 계산 자원이 적기 때문에 일반적으로 성능 면에서 스킨드 메시 애니메이션보다 더 좋습니다.**

스킨드 메시 애니메이션은 버텍스에 가중치가 부여된 뼈대 스켈레톤이 있는 메시를 사용하는 방식입니다. 스켈레톤이 애니메이션되고, 그에 따라 메시도 애니메이션됩니다. 이 과정은 특히 애니메이션할 버텍스가 많을 때 계산 비용이 상당히 높을 수 있습니다.

반면 트랜스폼 애니메이션은 오브젝트의 트랜스폼 속성(예: 위치, 회전, 스케일)을 직접 애니메이션하는 방식입니다. 보통 메시의 버텍스보다 애니메이션할 트랜스폼 속성이 적기 때문에, 스킨드 메시 애니메이션보다 적은 계산 자원으로 수행할 수 있습니다.

또한 트랜스폼 애니메이션은 키프레임 데이터와 같은 더 압축된 형식으로 미리 계산되어 저장될 수 있어, 실시간으로 복잡한 계산을 하지 않고도 빠르게 접근하고 재생할 수 있는 경우가 많습니다.

전반적으로 스킨드 메시 애니메이션은 더 세부적이고 사실적인 애니메이션을 만들 수 있지만, 비디오 게임과 같은 성능이 중요한 애플리케이션에서는 트랜스폼 애니메이션이 자주 선호됩니다.

## 여러 애니메이션 클립 생성 및 내보내기

모델에 원하는 만큼 많은 애니메이션 클립을 둘 수 있습니다. 애니메이션을 생성, 찾아보기, 삭제하는 방법은 [이 섹션에서](https://www.notion.so/5e962e5d54a24bcb9b906748007eb4cc).

아래 영상에서는 하나의 GLB 파일로 Blender에서 여러 애니메이션을 내보내는 방법을 배웁니다.

[![Video Preview](/docs/_remote/eb23ea78b0ffa9a0778e60d6.webp)](https://www.youtube.com/watch?v=YxAB4bujO_w&ab_channel=Decentraland)

## 많은 애니메이션을 하나로 내보내기

여러 오브젝트를 리깅하고 그 애니메이션을 하나의 애니메이션 클립에 합칠 수 있는 스켈레탈 애니메이션과 달리, 오브젝트 애니메이션에서는 그렇게 할 수 없습니다. 애니메이션은 따로 내보내지며, 한 번에 하나의 오브젝트만 재생됩니다. 이에 대한 해결책은 여러 애니메이션을 하나로 합쳐 모든 오브젝트가 동시에 움직이는 애니메이션 클립을 만드는 것입니다.

이를 위해 ***비선형 애니메이션 트랙*** 에 각 오브젝트의 애니메이션을 아래와 같이 내리세요.

![](/docs/_remote/03413441d7a33b6e5298b18f.webp)

*각 오브젝트의 애니메이션을 NLA 트랙으로 내리세요.*

모든 애니메이션이 NLA 에디터에 나열되면 모든 오브젝트를 선택하고 **File > Export > glTF2.0**으로 이동하세요. Include를 펼치고 Selected Objects를 체크하세요. Animation을 펼친 다음 다시 Animation을 펼치고 **Group by NLA Track**의 체크를 해제하세요. Merged Animation Name에서 애니메이션 클립 이름을 바꾼 뒤 **Export glTF 2.0**.

![](/docs/_remote/9ba538ff6074491ba9311c8c.webp)

*을 누르면 여러 애니메이션을 하나로 내보낼 수 있습니다.*

이 방법은 각 오브젝트에 애니메이션이 하나씩만 있을 때만 작동한다는 점을 기억하세요. 오브젝트에 여러 애니메이션 클립이 있다면 따로 내보내는 것이 가장 좋습니다.


> [!NOTE]
> **💡 팁**: 직접 애니메이션을 만들지 않고, 일반적인 애니메이션을 다운로드해 모델에 적용할 수도 있습니다. 예를 들어 인간형 특징을 가진 3D 캐릭터의 경우 [Mixamo](https://www.mixamo.com/#/).


## 샘플링

때때로 애니메이션 수나 애니메이션+모델의 조합 때문에 애니메이션 파일이 너무 무거워질 수 있습니다.

샘플링은 애니메이션을 최적화하는 좋은 방법입니다. 샘플링 비율은 애니메이션에서 키프레임이 얼마나 자주 베이킹될지를 결정합니다. 예를 들어 샘플링 비율이 2로 설정되면 두 프레임마다 키프레임이 생성됩니다. 샘플링 비율이 3이면 세 프레임마다 키프레임이 베이킹되는 식입니다. 샘플링 비율이 높을수록 파일은 가벼워집니다.

하지만 단점은 일부 중요한 키프레임이 사라지기 때문에 애니메이션이 점점 덜 부드러워진다는 점입니다(키프레임이 애니메이션 전체에 고르지 않게 분배되기 때문입니다). 또한 **샘플링은 애니메이션의 프레임 수를 샘플링 속도로 나누는 것이 아닙니다**.

일반적으로  **2 또는 3의 샘플링 비율** 이면 충분합니다. 이 수치는 품질을 저하시키지 않으면서 애니메이션을 최적화할 수 있습니다.

다음 항목을 찾을 수 있습니다 ***샘플링 비율*** 을 내보내기 설정의 ***Animation***.

![](/docs/_remote/942221d28977584ca4c825da.webp)


> [!NOTE]
> **💡 팁** 아래에서 찾을 수 있습니다. 애니메이션 프레임 수를 샘플링 속도로 나눌 수 있다면 좋은 것입니다! 이는 마지막 프레임이 베이크되어 애니메이션의 끝에서 시작으로 이어지는 전환이 보존된다는 뜻입니다.


## 애니메이션 구현

이 문서는 3D 모델에 애니메이션을 추가하는 방법을 다룹니다. [**애니메이션 처리**](/docs/creator/sdk7/3d-essentials/3d-model-animations/) 를 참고하여 씬에서 애니메이션을 활성화하고 처리하는 방법을 확인하세요.

## 애니메이션 모범 사례

* 아마추어는 가능한 한 단순하게 유지하고, 애니메이션할 계획인 모델 부위에만 뼈를 만드세요. 뼈는 성능에 영향을 줄 수 있으므로 실제로 필요한 것만 추가해야 합니다.
* 애니메이션이 씬에서 반복 재생될 예정이라면, 더 나은 전환을 위해 최종 포즈가 시작 포즈와 완전히 같아야 합니다.
* 캐릭터가 실제로 아무것도 하지 않더라도 절대 애니메이션이 없는 상태로 두지 마세요. 캐릭터가 가만히 있을 때를 위한 "idle" 애니메이션을 만드세요. idle에는 호흡 같은 미세한 움직임과 가끔 주변을 둘러보는 동작을 포함할 수 있습니다.
* 손, 손가락, 머리, 목처럼 뼈의 포즈를 비워 두지 마세요. 디테일은 좋은 애니메이션에서 매우 중요하며, 뻣뻣한 움직임은 신뢰감을 떨어뜨릴 뿐입니다.
* 극도로 필요한 경우가 아니라면 키프레임을 너무 많이 넣지 마세요. 키프레임을 많이 추가할수록 보간이 나빠질 가능성과 애니메이션이 더 무거워질 가능성이 높아집니다.
* 모든 뼈(변형용 뼈 포함)를 선택하고 애니메이션의 첫 프레임과 마지막 프레임에 키프레임을 설정하세요. 이렇게 하면 정보가 없는 뼈가 생겨 하나의 애니메이션이 다른 애니메이션에 의도치 않게 영향을 주는 일을 방지할 수 있습니다.
* 내보낼 때 파일에는 아마추어가 하나만 있어야 합니다. 애니메이션을 가져오면 아마추어도 함께 가져와집니다. 모든 애니메이션은 같은 기본 아마추어로 수행되어야 하므로, 필요 없는 것은 모두 삭제하세요.
* 뼈와 애니메이션 이름은 항상 바꾸세요. 모든 것을 정리해 두세요!
* 내보낼 때 *glTF* 모델을 확인하여 모든 오브젝트와 애니메이션이 내보내지고 있는지 확인하세요. 일부 익스포터는 기본적으로 ***현재 선택된 항목만*** 내보냅니다.
* 모델을 내보낸 후 [Babylon Sandbox](https://sandbox.babylonjs.com/) 에서 확인하고, 모든 애니메이션이 제대로 작동하며 예상한 이름으로 되어 있는지 점검하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/3d/animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
