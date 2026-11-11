> 아바타 리그와 다운로드 가능한 파일에 대한 기능.

# 리그 기능

이 문서는 Rig 1.0의 설정, 컨트롤, 기능을 설명합니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 도움이 되는 여러 편리한 기능이 포함되어 있습니다.


### 아마추어 변환

이것은 컨트롤 설정이 적용된 오브젝트 모드의 아마추어 변환입니다. **이것은 어떤 방식으로도 수정하지 마세요**. 리그는 포즈 모드에서만 조작해야 합니다. 원치 않는 편집을 방지하기 위해 변환은 오브젝트 모드에서 잠겨 있습니다.

*Rig 1.0 변환.*


> [!WARNING]
> ⚠️ **경고**: **오브젝트 모드에서 리그를 절대 편집하지 마세요.**


## 뼈 방향

이것은 Rig 1.0의 뼈 방향입니다.

*뼈 방향의 축.*

*포즈를 미러링할 때의 동작.*

### 포즈 미러링

이 리그의 왼쪽과 오른쪽 뼈는 같은 방식으로 배치되어 있지 않아서, 하나의 미러링 방식만으로는 몸 전체에 적용되지 않습니다. 다음부터 시작하세요. **포즈 > 포즈 복사**, 그런 다음 **X 반전하여 붙여넣기**, 그리고 상체는 수동으로 마무리합니다.

**다리와 발은 붙여넣기 후에 처리합니다.** **X 반전하여 붙여넣기** 는 다음에서 올바른 결과를 제공합니다. `CTRL_FK_Avatar_UpLeg`, `CTRL_FK_Avatar_Leg`, `CTRL_FK_Avatar_Foot`, `CTRL_FK_Avatar_ToeBase`, `CTRL_IK_Foot`, `CTRL_IK_Avatar_ToeBase` 그리고 `CTRL_Avatar_Knee`. IK 다리에서는 정확하며, FK 다리에서는 왼쪽과 오른쪽 휴식 뼈의 롤이 약간 다르기 때문에 발에서 약 3cm 정도 차이가 납니다. 정확해야 한다면 FK 발을 눈으로 보며 조금 조정하세요.

**어깨, 팔, 손, 손가락은 한 단계가 더 필요합니다.** **X 반전하여 붙여넣기** 만으로는 다른 손이 원하는 위치에서 40\~70cm 정도 벗어나게 됩니다. 붙여넣은 후에는 이 컨트롤들을 하나씩 확인하고 **X와 Z 값을 반전하고 Y는 유지하세요**:

* `CTRL_Avatar_Shoulder`
* `CTRL_FK_Avatar_Arm`, `CTRL_FK_Avatar_ForeArm`, `CTRL_FK_Avatar_Hand`
* `CTRL_IK_Avatar_Hand`, `CTRL_IK_Avatar_Elbow`
* 전체 30개 `CTRL_Avatar_Hand*` 손가락 컨트롤

쿼터니언이나 오일러 각 중 어떤 방식으로 작업하든 회전에 적용하세요. 부호는 어느 쪽이든 같습니다. IK 손과 팔꿈치 컨트롤에는 위치에도 적용하세요.

예를 들어, 붙여넣은 팔의 값이 X -0.5, Z 0.1이라면 X 0.5, Z -0.1이 됩니다.

이렇게 하면 전체 팔 체인이 정확하게 맞습니다.

**상체가 다른 이유:** 오른쪽 다리의 뼈 프레임은 왼쪽의 단순한 거울상이기 때문에 **X 반전하여 붙여넣기** 가 가정하는 방식입니다. 오른쪽 팔의 프레임은 로컬 **Z** 축이 X 대신 반전되어 있어, 반전 붙여넣기를 하면 잘못된 방향으로 회전한 상태로 나옵니다. X와 Z를 반전하면 정확히 이것이 보정됩니다.

#### 대신 값을 직접 설정하기

붙여넣는 대신 원시 왼쪽 값으로 오른쪽 값을 직접 설정한다면, 다음 부호 반전을 사용하세요:

| 컨트롤 | 회전 `(w, x, y, z)` | 위치           |
| --- | ----------------- | ------------ |
| 상체  | `(w, -x, -y, z)`  | `(x, y, -z)` |
| 다리  | `(w, x, -y, -z)`  | `(-x, y, z)` |

대략적인 방법입니다. 팔은 약 1mm 이내로 맞지만, FK 다리는 5\~10cm 정도 차이가 납니다. **X 반전하여 붙여넣기** 또한 왼쪽과 오른쪽 휴식 뼈의 작은 롤 차이도 보정하지만, 단순한 부호 반전은 그렇지 않습니다. 위의 붙여넣기 후 수정 방법을 권장합니다.


> [!WARNING]
> **📔 참고**: 시작 포즈가 처음부터 완벽하게 대칭은 아니므로, 한쪽에서 다른 쪽으로 포즈를 미러링하면 그 작은 비대칭이 덮어써집니다. 키프레임을 찍기 전에 결과를 눈으로 확인하세요.


### 팔 회전하기

상완 컨트롤 `CTRL_FK_Avatar_Arm`은 로컬 X축이 기울어져 있습니다. 이 축을 중심으로 회전하면 팔이 가슴을 가로지르게 되는데, 팔을 들어 올리려 했는데 팔이 교차되는 가장 흔한 이유입니다.

* 하려면 **방향을 맞추려면** 세계 축을 기준으로 회전하세요. 세계 X는 팔을 앞으로 들어 올리고, 세계 Z는 안팎으로 움직입니다.
* 하려면 **비트는** 팔을 비틀려면, 손바닥이 위나 아래를 향하고 팔꿈치가 안쪽이나 바깥쪽을 가리키도록 뼈 자체의 로컬 Y축을 기준으로 회전하세요.

### 각 관절이 구부러지는 축

팔꿈치와 무릎은 로컬 X축으로 구부러집니다. 손목과 손가락은 그렇지 않습니다. 로컬 Z축으로 굽습니다. 손가락을 로컬 X축으로 회전하면 옆으로 움직이는데, 완성된 이모트에서 손가락이 잘못 접히는 가장 흔한 이유입니다.

| Control                                                | 이 축으로 굽히세요            | 다른 축                               |
| ------------------------------------------------------ | --------------------- | ---------------------------------- |
| 팔꿈치 `CTRL_FK_Avatar_ForeArm`, 무릎 `CTRL_FK_Avatar_Leg`  | 로컬 X                  | Y와 Z는 잠겨 있음                        |
| 손목 `CTRL_FK_Avatar_Hand`                               | 로컬 Z는 손을 손바닥 쪽으로 굽힘   | 로컬 X는 옆으로 기울임                      |
| 손가락 `CTRL_Avatar_Hand{Index,Middle,Ring,Pinky}{1,2,3}` | 로컬 Z는 손가락을 손바닥 쪽으로 굽힘 | 로컬 X는 손바닥 평면에서 옆으로 움직입니다. Y는 잠겨 있음 |
| 엄지 기저부 `CTRL_Avatar_HandThumb1`                        | 로컬 X는 엄지를 손바닥 가로로 움직임 | Y는 잠겨 있음                           |
| 엄지 `CTRL_Avatar_HandThumb2`, `CTRL_Avatar_HandThumb3`  | 로컬 Z는 엄지를 굽힘          | Y는 잠겨 있음                           |

손가락 굽힘은 양손 모두 같은 방향으로 갑니다. 팔꿈치 굽힘, 손목 기울기, 그리고 엄지의 손바닥 가로 움직임은 왼쪽과 오른쪽에서 반대 방향이므로, 한쪽에서 맞는 값은 다른 쪽에서는 부호를 바꿔야 합니다.

### 뼈 컬렉션

사고를 방지하고 컨트롤을 더 쉽게 식별할 수 있도록, 이 리그는 다음에서 접근할 수 있는 뼈 컬렉션으로 구성되어 있습니다. *데이터 속성* Blender의 탭에서 볼 수 있습니다. 이 컬렉션들의 표시 여부는 *눈 아이콘* 을 클릭하여 켜고 끌 수 있습니다. 기본적으로 DON'T TOUCH 컬렉션을 제외한 모든 항목이 표시됩니다.

아마추어 데이터 속성 탭.

뼈를 컬렉션으로 분리한 방식은 다음과 같습니다:

* Global/Switch: 루트와 척추 같은 전역 컨트롤과 어깨를 포함합니다. 사용자 지정 속성이 있는 컨트롤도 이 컬렉션에 들어 있습니다.
* FK Upper: 상체 FK 설정 컨트롤 전부.
* FK Lower: 하체 FK 설정 컨트롤 전부.
* IK Upper: 상체 IK 설정 컨트롤 전부.
* IK Lower: 하체 IK 설정 컨트롤 전부.
* Fingers: 양손 손가락용 컨트롤.
* Deformation Bones: 변형 뼈가 저장되는 곳입니다.


> [!WARNING]
> **⚠️ 주의!**
> 
> DON'T TOUCH 컬렉션에는 IK 및 기타 리그 제약 설정이 들어 있으며 숨겨진 상태로 유지해야 합니다. 이 뼈들을 편집하면 리그의 기능이 손상될 수 있습니다.


## 컨트롤과 그룹화

컨트롤은 기본 스켈레톤을 움직이는 비변형 뼈입니다. 범주에 따라 다른 색을 가집니다:

* 노란색: 전역 컨트롤 및 사용자 지정 속성이 있는 컨트롤
* 초록색: 힙(척추 계층에서 식별하기 더 쉬움)
* 파란색: FK 동작이 있는 컨트롤
* 빨간색: IK 컨트롤
* 분홍색: 왼쪽 컨트롤
* 주황색: 오른쪽 컨트롤

*모든 컨트롤과 그 색상.*

## 사용자 지정 속성과 설정

### FK/IK 블렌드

팔은 보통 FK로, 다리는 IK로 설정되지만, 경우에 따라 다른 설정이 필요할 수 있습니다. 손이 푸시업이나 등반 중처럼 특정 위치를 유지해야 한다면 IK가 가장 좋습니다. 다리는 공중에 있거나, 수영하거나, 구를 때 FK가 가장 잘 맞습니다. 애니메이션에서 더 많은 유연성과 자유도를 위해, 이 리그에는 UpperBody 컨트롤의 FK/IK 블렌드가 있으며, 0은 완전 FK, 1은 완전 IK입니다. 그 사이의 다른 값은 두 방식의 혼합입니다.

![양팔과 양다리의 FK/IK 블렌드.](/docs/_remote/efb0c0633809e34cc3499adb.webp)

*양팔과 양다리의 FK/IK 블렌드.*

![FK > IK 전환 작동 방식.](/docs/_remote/b7bd249527c142b1c626e967.webp)

*FK > IK 전환 작동 방식.*

간단한 규칙으로 대부분을 처리할 수 있습니다. 팔다리가 어떤 것과 접촉하고 있으면 IK를 유지하고, 접촉이 끊겨 자유롭게 흔들리면 FK로 전환하세요. 앉은 포즈의 경우:

* **양발이 바닥에 닿아 있음**: 다리는 IK로 유지하세요. 다음으로 발을 움직이고 `CTRL_IK_Foot`, 그런 다음 `CTRL_Avatar_UpperBody` 를 내려서 몸을 낮춥니다. 발은 제자리에 고정됩니다. 무릎은 `CTRL_Avatar_Knee`.
* **한 다리를 다른 다리 위로 교차**: 들어 올린 다리만 FK로 전환하세요.
* **발이 늘어져 있음**: 양다리를 모두 FK로 전환하세요.

다음을 사용하세요 `CTRL_Avatar_UpperBody` 를 사용해 몸 전체를 위아래로 움직입니다. 이것이 무게 중심입니다. `CTRL_Avatar_Hips` 는 골반과 다리만 움직여 상체를 늘려 버리므로, 앉을 때는 올바른 컨트롤이 아닙니다.


> [!WARNING]
> **📔 참고**: 클립 중간에서 팔다리를 FK와 IK 사이로 전환하지 마세요. 두 체인이 그 프레임에서 정확히 일치하지 않으면 팔다리가 튑니다. 꼭 전환해야 한다면 FK > IK 속성 자체에 키프레임을 찍어 변화가 원하는 지점에서 일어나게 하세요.


#### IK로 얼굴이나 대상을 터치하기

손이 얼굴, 소품 또는 다른 손 같은 무언가를 터치하게 하려면 그 팔을 IK로 전환하세요. FK 관절만으로는 손끝을 대상에 안정적으로 맞출 수 없습니다. IK 손 컨트롤은 손목 위치에 있다는 점을 기억하세요. 따라서 컨트롤을 대상 위에 두면 손목이 그곳으로 가고 손끝은 아닙니다. 손끝이 원하는 위치에 닿을 때까지 컨트롤을 움직이세요.

FK > IK 속성은 키프레임 사이에서 서서히 블렌딩됩니다. 전환하는 키프레임에서는 FK 팔을 IK 포즈에 가깝게 맞추세요. 그렇지 않으면 옆에 늘어진 FK 팔이 블렌드가 진행되는 동안 가슴을 가로질러 움직입니다.

아바타가 기본 대기 포즈로 서 있을 때 측정한, 기본 아바타 머리의 치수를 사용해 얼굴에 손을 위치시키세요:

| 기준점             | 바닥에서의 높이                 | 다음 기준에 대한 상대 위치 `Avatar_Head` 뼈          |
| --------------- | ------------------------ | ---------------------------------------- |
| `Avatar_Head` 뼈 | 1.72 m                   | 이것은 두개골 바닥이 아니라 얼굴의 중앙입니다                |
| 코끝              | 1.66 m (남성 몸체에서는 1.64 m) | 앞으로 0.19 m, 아래로 0.06 m (남성 몸체에서는 0.08 m) |
| 턱               | 1.60 m                   | 아래로 0.12 m                               |
| 머리 꼭대기          | 1.90 m                   | 위로 0.18 m                                |

머리의 너비는 약 0.30 m입니다. 머리 뼈보다 0.12 m 위를 향한 손은 이마에 닿습니다. 머리가 회전되어 있다면, 이 오프셋을 머리의 회전된 좌표계에 적용하세요.

### 회전 FK 블렌드 분리

UpperBody 컨트롤의 또 다른 사용자 지정 속성은 회전 분리로, 뼈가 부모의 회전을 상속할지 여부를 선택할 수 있게 해줍니다(FK일 때). 0에서는 뼈가 회전을 상속하지 않고, 1에서는 부모의 동작을 완전히 따릅니다. 그 사이의 다른 값은 두 방식의 혼합입니다. 이 도구가 흥미로운 이유는 FK 뼈가 위치를 유지하게 만들어, 약간 IK처럼 동작하기 때문입니다.

![팔용 회전 분리 속성.](/docs/_remote/627c80a7f1680d2c4d02c6d4.webp)

*팔용 회전 분리 속성.*

![팔의 IsoRot 속성이 작동하는 방식.](/docs/_remote/517ca0821f2dd011b2c186f1.webp)

*팔의 IsoRot 속성이 작동하는 방식.*

Head 컨트롤에도 이 속성이 있습니다. 예를 들어 걷기 사이클에서 매우 유용한데, 몸통이 비틀려도 머리는 회전을 유지해 항상 정면을 보게 하기 때문입니다. 이 옵션이 없으면 애니메이터는 몸통이 비틀릴 때마다 머리가 곧고 정면을 보도록 직접 회전시켜야 합니다.

![머리용 회전 분리 속성.](/docs/_remote/4afc891b59733db2550998de.webp)

*머리용 회전 분리 속성.*

![머리의 IsoRot 속성이 작동하는 방식.](/docs/_remote/4c87d322918f5fad671253d0.webp)

*머리의 IsoRot 속성이 작동하는 방식.*


> [!WARNING]
> ⚠️ **경고**: 이전 버전의 Blender에서는 모든 컨트롤을 선택하고 키프레임을 찍어도 이러한 사용자 지정 속성은 자동으로 키프레임되지 않습니다. 만든 포즈/동작을 잃지 않도록 각 속성에 키프레임을 수동으로 삽입하세요. Blender 4.4에서는 I를 누르면 모든 속성과 사용자 지정 속성에 키프레임이 설정됩니다.


![이전 버전의 Blender에서는 모든 컨트롤과 사용자 지정 속성에 키프레임을 꼭 찍으세요!](/docs/_remote/a66e6f888ab4d3128c2395ee.webp)

*이전 버전의 Blender에서는 모든 컨트롤과 사용자 지정 속성에 키프레임을 꼭 찍으세요!*

*Blender 4.4에서는 I를 눌러 Location, Rotation, Scale 및 Custom Properties에 자동으로 키프레임을 설정하세요..*

사용자 지정 속성에 키프레임을 찍는 또 다른 방법은 ***키잉*** 을 타임라인 탭의 ***활성 키잉 세트*** 에서 선택하고, 아래 GIF에 보이는 것처럼 Location, Rotation, Scale 및 Custom Properties를 선택하는 것입니다. 그러면 I를 누를 때마다 팝업 메뉴 없이 키프레임이 생성됩니다. 일부 애니메이터는 메뉴를 선호하기 때문에 기본적으로 이 옵션은 활성화되어 있지 않습니다. 하지만 가장 잘 맞는 방법을 자유롭게 선택하세요.

![키잉 옵션으로 키프레임 찍기.](/docs/_remote/296a3a5045569233102e0836.webp)

*키잉 옵션으로 키프레임 찍기.*

### IK 발 설정 반전

FK 발을 애니메이션하는 것은 매우 간단합니다. 컨트롤 중 하나를 잡아 회전시키기만 하면 됩니다. 발용 컨트롤과 발가락용 컨트롤이 따로 있기 때문에 애니메이터는 움직임을 완전히 제어할 수 있습니다. 그러나 IK에서는 그렇게 단순하지 않습니다. 발은 바닥에 붙어 있어야 하면서도 발볼과 뒤꿈치, 좌우 방향으로 회전할 수 있어야 합니다.

이 리그는 IK 시스템의 장점을 잃지 않으면서도 애니메이터가 발 움직임을 자유롭게 할 수 있도록 설정되어 있습니다. 이는 네 개의 컨트롤로 구성됩니다:

* Foot roll: 이 컨트롤은 발을 앞뒤와 좌우로 회전시킵니다. 뒤꿈치나 발볼 쪽으로 너무 많이 굽는 것을 방지하기 위해 한계를 설정하여 발 리그가 깨지지 않도록 했습니다. 이 한계에 도달하면 발은 회전을 멈춥니다.

![Foot roll: X로 회전하면 앞뒤로 움직이고, Z로 회전하면 좌우로 움직입니다.](/docs/_remote/28fab31b129fde23a5c5f90d.webp)

*Foot roll: X로 회전하면 앞뒤로 움직이고, Z로 회전하면 좌우로 움직입니다.*

* Toe tip roll: 발가락 끝에서 발을 회전시킵니다. 앞으로만 회전합니다.

![toe tip roll은 양의 X로만 회전합니다.](/docs/_remote/68e7725b2019cbf56dd0f028.webp)

*toe tip roll은 양의 X로만 회전합니다.*

* Toes control: 발볼에서 발가락을 회전시킵니다.

![발가락은 어떤 방향으로도 회전할 수 있습니다.](/docs/_remote/9c173333785f1d51960159db.webp)

*발가락은 어떤 방향으로도 회전할 수 있습니다.*

* Foot control: 이것은 발 전체를 움직이는 전역 컨트롤입니다. 다른 모든 발 컨트롤의 부모이므로, 어떤 변환도 유지하면서 잡고 회전시킬 수 있습니다.

![발 컨트롤이 작동하는 방식.](/docs/_remote/aa913f0950d1fe1e5d55b393.webp)

*발 컨트롤이 작동하는 방식.*

### 잠긴 변환

일부 컨트롤에는 변환 매개변수 옆에 잠금 아이콘이 있을 수 있으며, 이는 해당 값이 변경될 수 없다는 뜻입니다. 이는 특정 방식으로만 동작해야 하는 컨트롤에 적용되어 원치 않는 변환을 방지하기 위한 것입니다. 예를 들어 팔꿈치와 무릎은 하나의 축, 이 경우 X축으로만 회전하도록 되어 있으므로 다른 축은 잠겨 있습니다. 잠긴 속성을 가진 다른 예로는 IK 팔꿈치와 무릎, 손가락, 발 롤, 발가락 끝이 있습니다.

이 잠금은 유지하는 것이 좋지만, 더 자유로운 움직임이 필요하다면 잠금 아이콘을 클릭하여 해제하면 됩니다.

잠금은 Blender의 인터랙티브 도구에만 적용됩니다. 뼈의 회전을 직접 쓰는 Python 스크립트는 이를 무시하므로, 스크립트로 리그를 포즈할 경우 각 관절의 자유 축에 대한 회전은 직접 유지하세요.

![컨트롤의 잠긴 변환.](/docs/_remote/52bbc9e002370ab201046aeb.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/emotes/rig-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
