> Decentraland 이모트를 만드는 팁과 가이드라인.

# 이모트 만들기

이 문서에서는 파일 사양, 블렌더에서의 애니메이션 기본, 이모트를 올바르게 내보내는 방법, 그리고 빌더에 가져오는 방법을 다룹니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블, 이모트를 편집하고 내보내는 데 도움이 되는 여러 유용한 기능이 포함되어 있습니다.


#### 애니메이션 사양 표

| 프레임 속도      | 30 fps             |
| ----------- | ------------------ |
| 최대 길이       | 10초(300프레임)        |
| 파일당 애니메이션 수 | 1                  |
| 내보내기 형식     | .glb               |
| 샘플링 속도      | 기본값 1(필요하면 2 또는 3) |
| 최대 파일 크기    | 3 MB               |
| 최대 애니메이션 거리 | 1미터(앞/뒤, 왼쪽/오른쪽)   |
| 최대 애니메이션 높이 | 4미터                |

애니메이션 사양에 대한 더 자세한 설명은 [**아래에서**](#the-animation-specifications).

### **자료**

이 문서는 Rig 1.0의 설정, 컨트롤, 기능을 설명합니다.

[Decentraland Blender Rig](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Avatar_File.blend)


> [!NOTE]
> Maya를 사용한다면 이 [Maya Rig](https://github.com/decentraland/docs/tree/main/creator/images/emotes/DCL_Maya_Rig.ma) 과 [피커를](https://github.com/decentraland/docs/tree/main/creator/images/emotes/emoteAvatar.pkr) 제공한 [SparkleStudios](https://www.sparkles.studio/) ❤️.


## **시작하기 전에**

### **프레임 속도**

시작하기 전에 프레임 속도를 확인하는 것이 중요합니다. Decentraland의 애니메이션은 반드시 30 fps의 프레임 속도를 가져야 합니다. 리그 파일은 기본적으로 24 fps로 설정되어 있으므로, 애니메이션을 만들기 전에 30으로 변경하세요(잘못된 프레임 속도는 애니메이션 속도에 영향을 줍니다). 해당 옵션은 아래와 같이 Format 아래의 Output Properties(프린터 아이콘)에서 찾을 수 있습니다:

![시작하기 전에 프레임 속도가 30 fps로 설정되어 있는지 확인하세요.](/docs/_remote/53650a16525c3ac92e8eef4c.webp)

시작하기 전에 프레임 속도가 30 fps로 설정되어 있는지 확인하세요.

### **포즈 모드**

블렌더에서 리그는 세 가지 다른 모드로 볼 수 있습니다: Object Mode, Edit Mode, 그리고 Pose Mode입니다. 애니메이션은 Pose Mode에서만 만들 수 있습니다(이 모드에서는 컨트롤에 색이 표시됩니다). 리그를 선택한 상태에서 오른쪽 상단의 드롭다운 메뉴에서 해당 옵션을 찾을 수 있습니다.

![Pose Mode로 전환하기.](/docs/_remote/2d50e8d239676da8c48b0736.webp)

Pose Mode로 전환하기.

### **애니메이션용 인터페이스**

리그 파일에는 뷰포트(정면 및 측면 뷰)용 두 개의 창 외에도 아래쪽에 세 개의 창이 더 있습니다:  ***그래프 에디터***, ***도프 시트***, 그리고  ***타임라인***.

* ***그래프 에디터***: 이 편집기에서는 선택한 컨트롤의 각 변환 속성에 대한 애니메이션 곡선을 편집할 수 있습니다. 이 곡선은 보간이 어떻게 계산되는지 보여주며, 애니메이션에서 원하는 효과를 얻기 위해 편집할 수 있습니다. 여기와 도프 시트 모두에서  ***선택한 것만 표시*** 도구가 활성화되어 있어, 선택한 컨트롤과 관련된 채널만 포함됩니다. 화살표 아이콘을 클릭하면 이 기능을 켜고 끌 수 있습니다.
* ***도프 시트***: 여기에서 키프레임을 편집할 수 있습니다. 새 애니메이션을 만들거나 이미 만들어진 여러 애니메이션을 살펴보는 곳도 여기입니다. 애니메이션에 접근하려면  ***액션 에디터*** 를 선택해야 한다는 점을 기억하세요. 이 옵션은  *도프 시트* 아이콘
* ***타임라인***: 이것은 타임라인과 재생 컨트롤이 있는 곳입니다. 여기서는  ***자동 키잉*** 이 켜져 있어, 컨트롤을 조작할 때마다 자동으로 키프레임이 만들어집니다. 재생 컨트롤 옆의 점을 클릭하면 이 기능을 언제든지 끌 수 있습니다.

이 작업 공간만 있으면 애니메이션을 시작하는 데 필요한 모든 준비가 끝납니다!

![](/docs/_remote/4dfc916aa7bf8bcc00b1659f.webp)

이것들은 아래쪽 창입니다. 위쪽 창은 ***그래프 에디터,*** 가운데 창은 ***도프 시트,*** 그리고 아래쪽 창은 ***타임라인입니다.*** 위쪽의 빨간 화살표는 ***선택한 것만 표시*** 도구를 보여주고, 아래쪽 화살표는 ***자동 키잉***.


> [!NOTE]
> **💡 힌트!**
> 
> 블렌더는 매우 사용자 정의가 가능하므로, 지금이야말로 창을 추가, 조정 또는 제거하면서 자신에게 가장 잘 맞는 레이아웃을 설정하기 좋은 때입니다. 애니메이터마다 선호도가 다르니, 원하는 대로 레이아웃을 편집해도 좋습니다!


## 시작하기

#### **시작 포즈**

제공된 리그 파일에는 이미 액션인  ***Starting\_Pose***&#xAC00; 들어 있습니다. 모든 아바타 액션은 아이들 포즈에서 시작하므로, **그 포즈에서 애니메이션을 시작하고 마지막 프레임에서도 다시 사용하는 것을 적극 권장합니다**. 이렇게 하면 Idle에서 Emote로의 전환이 더 자연스럽고 애니메이션도 더 부드러워집니다.


> [!NOTE]
> **💡 힌트!**
> 
> 루프 애니메이션을 만들고 싶다면 시작 포즈에서 애니메이션을 시작할 필요는 없습니다. 애니메이션에서 더 적절한 포즈를 자유롭게 사용하세요!\*\*


루프가 매끄럽게 재생되려면 첫 번째와 마지막 키프레임이 반드시 동일해야 합니다. 예를 들어 30 fps에서 4초짜리 루프는 1프레임과 121프레임에 같은 포즈가 있어야 합니다. 조금이라도 다르면 클립이 반복될 때마다 플레이어에게 눈에 띄는 튐 현상이 보입니다.

#### 애니메이션 영역 <a href="#ground-reference-and-animation-area" id="ground-reference-and-animation-area"></a>

제공된 아바타 파일에는 다음이라는 컬렉션이 있습니다: ***Animation\_Area\_Reference***. 이 컬렉션은 애니메이션 제한 범위를 시각적으로 설정하는 데 도움이 되는 세 개의 오브젝트로 구성되어 있습니다: ***Animation\_Area\_Reference***, ***Ground\_Reference*** 과 ***Root\_Animation\_Area***.

* Animation\_Area\_Reference는 4x4 큐브로, 바운딩 박스입니다. 아바타 메시와 소품은 전체 애니메이션 동안 이 영역 안에 있어야 합니다. 소품은 큐브 영역 안에만 있다면 자유롭게 이동할 수 있습니다.
* Ground\_Reference는 애니메이션 중 땅을 뚫고 들어가는지 확인하는 데 도움이 되는 평면입니다. 또한 중요한 정보가 하나 더 들어 있습니다: 지름이 2미터인 더 작은 원은 아바타 루트를 이동할 수 있는 영역을 정의합니다. 즉, 지면에 있을 때 아바타는 앞, 뒤, 왼쪽, 오른쪽 각각 1미터씩만 이동할 수 있습니다.
* Root\_Animation\_Area는 아바타 루트/무게 중심(CTRL\_Avatar\_UpperBody 또는 Avatar\_Hips)을 이동할 수 있는 영역을 정의하는 원통입니다. 이 원통의 높이는 4미터이므로, 메시가 바운딩 박스 밖으로 나가지 않는 한 루트는 최상단까지 올라갈 수 있습니다. 아바타의 다리, 팔 및 기타 신체 부위는 원통 밖에 있을 수 있지만, Animation\_Area\_Reference 밖으로 나가서는 안 됩니다.

![애니메이션 영역 참조.](/docs/_remote/8ef56331d4f29f23223dc576.webp)

요약하면, 루트가 원통 안에 있는 한 아바타는 오른쪽, 왼쪽, 앞, 뒤로 움직일 수 있으며, 이는 네 방향 각각 1미터를 의미합니다.

![아바타 이동.](/docs/_remote/7949b480b39476fbab88c60f.webp)

높이에 관해서는, 아바타가 바운딩 박스 안에 있는 한 최대 4미터까지 올라갈 수 있습니다. 아래에는 아바타 높이를 올바르게 처리한 사례(전신이 바운딩 박스 안에 있음)와 잘못 처리한 사례(메시가 바운딩 박스 밖에 있음)가 있습니다.

| ![](/docs/_remote/cbee2350215541a459589276.webp) | ![](/docs/_remote/4774d078d1fff2f442dda8f4.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

| ![](/docs/_remote/d51139e66aeafc6d74965579.webp) | ![](/docs/_remote/93bbdf9a30346096af762dea.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

![](/docs/_remote/da31c95fa83a05f64398f245.webp)

애니메이션 영역 경계 안에 있는 이모트 예시입니다.

| ![](/docs/_remote/989fc61d99ce5203d6197a0e.webp) | ![](/docs/_remote/aa48c499a9817ab4725cfe89.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |


> [!NOTE]
> **💡 주의!**
> 
> 이 경계를 넘으면 게임플레이 문제를 일으킬 수 있으니 주의하세요.


## **애니메이션 만들기**

blend 파일에는 편집할 수 있도록 준비된 애니메이션 클립이 있습니다: *StartingPose\_Avatar*. 필요에 따라 이 애니메이션 클립을 복제하고 이름을 바꿀 수 있습니다. 처음부터 새로 만들 필요는 없습니다!

에서 *액션 찾아보기* 섹션에서 간단히 ***새 액션 만들기*** 버튼을 클릭하여 현재 애니메이션을 복제하세요. 클립 이름을 바꾸려면 텍스트를 클릭하고 다른 내용을 입력하면 됩니다.

Blender 4.4에서는 *슬롯 액션*, 이전 버전의  *액션 찾아보기* 섹션 오른쪽에 있는 아이콘을 도입했습니다. 소품 없이 이모트를 만드는 경우에는 이를 따로 건드릴 필요가 없으므로 그대로 두면 됩니다. 아바타를 애니메이션할 때는 슬롯 액션이 Avatar\_Animation인지 확인하세요.

![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

기존 애니메이션을 복제하거나 ***액션 연결 해제*** 를 클릭한 다음 ***새로 만들기***.

### **애니메이션 찾아보기 및 삭제하기**

블렌더에서는 같은 파일 안에 여러 애니메이션 트랙을 둘 수 있습니다. Browse Action 드롭다운 메뉴를 클릭하여 이를 살펴볼 수 있습니다. F(Fake User)가 있는 모든 애니메이션은 저장됩니다. 애니메이션을 삭제하려면 키보드에서 Shift를 누른 채 X를 클릭하세요. 그렇게 하면 애니메이션 옆에 0이 표시되며, 이는 블렌더를 닫거나 파일을 다시 열 때 삭제된다는 뜻입니다.

![](/docs/_remote/cf72bdeab774263920c04bb9.webp)

애니메이션 찾아보기: F가 있는 것은 저장되고, 0이 있는 것은 삭제됩니다.

블렌더를 다시 불러오지 않고 애니메이션을 삭제하는 또 다른 방법은 Display Mode를 View Layer에서 Blender File로 바꾸는 것입니다. Actions를 펼친 다음 삭제하고 싶은 애니메이션을 마우스 오른쪽 버튼으로 클릭하고 Delete를 선택하세요.

![](/docs/_remote/3e2be89fb22f3ee63cc39ef1.webp)

Outliner의 Display Mode 아래 Blender File에서 애니메이션을 직접 삭제할 수 있습니다.


> [!NOTE]
> **💡 힌트!**
> 
> 항상 같은 애니메이션 트랙만 편집하지 마세요. 큰 변경을 하기 전에 애니메이션을 복제해 두세요. 그러면 무언가를 삭제하거나 바꾼 것을 후회할 때 사용할 백업 버전이 남습니다. 지금까지의 진행 상황을 추적하는 좋은 방법이기도 합니다!


![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

애니메이션 클립 복제하기.

### **이름 짓기**

**애니메이션 이름은 대문자로 시작해야 하며, 이름이 두 단어 이상이라면 단어는 \_로 구분해야 합니다.** 공백이나 특수 문자는 사용하지 마세요. 이름 짓기 예시는 다음과 같습니다:

* Snowfall
* Rainbow\_Dance
* Throw\_Money
* Talk\_To\_Hand

### **이모트 오버라이드**

이모트 오버라이드는 변형 본에 어떤 매개변수의 키프레임이 설정되지 않았을 때 발생합니다. 키프레임이 없으면 그 본은 어디에 있어야 하는지, 얼마나 회전하고 스케일되어야 하는지에 대한 정보를 가지지 못해 해당 채널이 비어 있게 됩니다. 그 결과, 월드에서 이모트를 재생한 뒤 이전 이모트가 아직 재생 중일 때 새 이모트를 트리거하면 위치, 회전, 스케일 정보가 이전 이모트에 의해 덮어써져 두 이모트가 섞이게 됩니다. 의도한 것이 아니라면 이는 애니메이션에 영향을 주며, 때로는 재미있는 결과가 나오기도 하지만, 때로는 이모트를 완전히 망가뜨리기도 합니다. 아래는 이모트 오버라이드의 예시입니다.

![](/docs/_remote/4d3cf50ba093eedb38f43aee.webp)

이를 방지하려면 뼈가 들어 있는 모든 레이어를 선택하세요(찾을 수 있는 위치는 ***오브젝트 데이터 속성*** > ***스켈레톤*** > ***레이어***)입니다. 그런 다음 ***포즈 모드***, 타임라인 커서를 애니메이션의 첫 프레임에 두고 ***뷰포트 표시***&#xC5D0;서 ***A*** 를 눌러 모두 선택하세요.  ***그래프 에디터***&#xC5D0;서 ***눈*** 아이콘을 아마추어 채널 옆에서 두 번 클릭하여 모든 채널을 보이게 하세요. 모든 뼈를 선택한 상태에서 ***I*** 를 눌러 키프레임을 설정하세요. 마지막 프레임도 동일하게 하세요.

**변형 본을 선택하는지 확인하세요. 이것은 특히 중요합니다!** 변형 본은 가장 아래쪽 마지막 레이어에서 찾을 수 있으며,  ***뷰포트***.

![](/docs/_remote/359c9a9a72aea9590225e2c7.webp) 에서 초록색 본으로 표시됩니다. 첫 프레임과 마지막 프레임의 모든 본에 키프레임을 설정하면 이모트 오버라이드를 방지할 수 있습니다.

## **애니메이션 사양**

### **애니메이션 길이**

애니메이션의 최대 길이는 **10초** 또는 **300프레임**입니다. 첫 프레임과 마지막 프레임의 모든 컨트롤 속성에 키프레임을 설정하는 것을 잊지 마세요.


> [!WARNING]
> ⚠️ 그래프 에디터에서 가시성이 꺼져 있는 채널은 키프레임이 설정되지 않으며, 삭제되거나 액션 에디터에 표시되지도 않습니다. 의도적으로 그렇게 한 것이 아니라면 가시성에 특히 주의하세요.


![](/docs/_remote/91ee3dde0d4a300d0eb09a66.webp)

키프레임을 찍기 전에 채널을 보이게 하세요!

### **애니메이션 수**

일반 이모트(소품 없음)인 경우, 내보낸 파일에는 애니메이션이 하나만 포함될 수 있습니다. Emotes 2.0의 경우 아바타용 클립 하나와 소품용 클립 하나를 둘 수 있습니다. 작업 중 애니메이션을 복제했다면 내보내기 전에 모두 삭제하세요. 최종 버전만 남기세요. 여러 애니메이션이 필요해서 동작하는 시퀀스 이모트(동작 시작, 동작 루프, 동작 종료)는 아직 지원되지 않습니다.

### **형식**

애니메이션은 .**GLB** 형식으로 내보내야 합니다. 파일에는 변형 스켈레톤과 애니메이션만 포함될 수 있습니다. **메시, 컨트롤 및 기타 다른 오브젝트는 내보내면 안 됩니다**. 더 자세한 내보내기 방법은 [**아래에서**](#exporting).

### **샘플링**

제약은 내보낼 수 없으므로, 애니메이션 클립을 내보내는 유일한 방법은 베이킹하는 것입니다. 즉, 변형 본의 위치, 회전, 스케일이 애니메이션의 모든 프레임에 키프레임으로 기록됩니다. 클립이 300프레임까지 길어지면 내보낸 뒤 300개의 키프레임을 갖게 되며, 키프레임이 많을수록 파일도 더 무거워집니다.

샘플링은 애니메이션을 최적화하는 좋은 방법입니다. 샘플링 속도는 애니메이션에서 키프레임을 얼마나 자주 베이크할지 정합니다. 예를 들어 샘플링 속도를 2로 설정하면 두 프레임마다 키프레임이 만들어집니다. 샘플링 속도 3이면 세 프레임마다 키프레임이 베이크되고, 이런 식으로 이어집니다. 샘플링 속도가 높을수록 파일은 더 가벼워집니다.

하지만 단점은 중요한 키프레임 일부가 사라지므로 애니메이션이 점점 덜 부드러워진다는 것입니다(키프레임이 애니메이션 전체에 고르게 분포되지 않기 때문입니다). 또한 다음도 중요한 점입니다. **샘플링은 애니메이션의 프레임 수를 샘플링 속도로 나누는 것이 아닙니다**.

보통은 **2 또는 3의 샘플링 속도** 면 충분합니다. 이 숫자들은 품질을 해치지 않으면서 애니메이션을 최적화할 수 있습니다.


> [!NOTE]
> **💡 힌트!**
> 
> 애니메이션의 프레임 수가 샘플링 속도로 나누어떨어진다면 좋습니다! 이는 마지막 프레임도 베이크되어 애니메이션의 끝에서 시작으로 이어지는 전환이 보존된다는 뜻입니다.


### **파일 크기**

최대 파일 크기는 **3 MB**입니다. 내보낸 후 파일이 이를 초과하면 메시가 실수로 내보내지지 않았는지, 애니메이션이 10초를 넘지 않았는지 확인해 보세요. 그래도 3 MB를 넘는다면 샘플링 속도를 조정해 보세요. 값이 높을수록 최적화가 더 잘됩니다.

이모트에 추가 3D 모델이 포함되어 있다면, 이 모델의 텍스처 크기는 1024픽셀을 초과할 수 없습니다.

## **내보내기**

아마추어와 애니메이션만 내보내고 싶으므로, 아래와 같이 내보내기 전에 메시와 아마추어 외의 모든 오브젝트의 가시성을 꺼 주세요:

![](/docs/_remote/3b011135dba6d9de9e62a101.webp)

내보내기 전에 메시 가시성을 꺼 주세요!

파일에는 내보낼 하나의 액션만 포함되어야 합니다. 리그에는 기본적으로 하나의 `Starting_Pose` 액션이 포함되어 있으므로 [삭제하고](#browsing-and-deleting-animations) 다른 남아 있는 액션도 함께 삭제하세요. 그렇지 않으면 내보낸 `.glb` 파일에는 하나의 애니메이션 대신 두 개의 클립이 들어가게 됩니다 [는 Decentraland의 기대와 다릅니다. 변경 가능한 사본의](#number-of-animations) .blend `파일을 정리하기 전에 저장해 두세요.` 내보내려면&#x20;

파일 *내보내기* > *glTF2.0 (.glb, .gltf)* > *으로 이동하세요. 내보내기 설정에서는 Include를 펼친 다음 Limit to에서 Visible Objects를 켜세요. 그런 다음 Data 탭을 펼치고 Armature를 펼친 뒤 Export Deformation Bones Only를 활성화하세요.*

![](/docs/_remote/3142125b130d9e9715dddd3d.webp)

애니메이션을 샘플링해야 한다면 Animation 탭을 펼치고 Sampling Animations를 확장한 뒤 원하는 샘플 수를 선택하세요.

| ![](/docs/_remote/446d202239ea82992946aa43.webp) | ![](/docs/_remote/eef0447fdcd598e5153a2669.webp) |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

애니메이션 내보내기는 이것으로 끝입니다!

| ![](/docs/_remote/b5ee46c688eb78bff045e6a1.webp) | ![](/docs/_remote/6813e1cd152caee20fe515fb.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

참조 자료

## 아직 어디서 시작해야 할지 모르겠거나 참고 자료나 영감이 필요하다면, 도움이 될 만한 애니메이션 클립들을 소개합니다. 공부 자료로도 좋습니다!

아직 어디서 시작해야 할지 모르겠거나 참고 자료나 영감이 필요하다면, 도움이 될 만한 애니메이션 클립들이 여기 있습니다. 좋은 학습 자료가 될 수 있습니다!

[Idle.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/idle.glb)

[Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/jump.glb)

[Walk.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/walk.glb)

[Run.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/run.glb)

[Pose\_Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_jump.glb)

[Pose\_Spin.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_spin.glb)

[Spotlight.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/spotlight.glb)

[Fashionista.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/fashionista.glb)

[Chic.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/chic.glb)

[Flag\_Emote.glb](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote.glb)

[Flag\_Emote.blend](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote_Final.blend)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/emotes/creating-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
