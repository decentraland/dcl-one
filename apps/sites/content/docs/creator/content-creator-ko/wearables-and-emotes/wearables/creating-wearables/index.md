> Decentraland 웨어러블 제작을 위한 팁과 가이드라인

# 웨어러블 만들기

![](/docs/_remote/a3778599667aedc8c1fc29ed.webp)

## 소개

이 가이드는 Decentraland 웨어러블용 맞춤 3D 모델을 만드는 기본을 소개합니다. Decentraland 아바타 시스템이 어떻게 작동하는지 설명하고, 자신만의 웨어러블을 올바르게 모델링하는 방법을 보여줍니다.

*참고: 이 가이드는 여러분이 이미 기본에서 중급 수준의 3D 모델링 지식을 어느 정도 갖추고 있다고 가정합니다. 3D 모델링이 처음이라면,* [*여기에서 시작하세요*](/docs/creator/3d-modeling/3d-models/)*.*


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블, 이모트를 편집하고 내보내는 데 도움이 되는 유용한 기능들이 여러 가지 포함되어 있습니다.


시작하기 전에, 참고용 메쉬와 텍스처 예제 파일을 다운로드하세요: [**웨어러블 참고 모델**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)

## Decentraland 아바타 시스템

Decentraland "아바타 시스템"은 사용자 지정 웨어러블로 꾸밀 수 있는 서로 다른 신체 구성 요소와 하위 구성 요소들의 광범위한 집합입니다. 이 구성 요소들은 다음과 같습니다:

* 신체 형태
* 머리
  * 머리 형태
  * 눈썹
  * 눈
  * 입
* 상체
* 하체
* 손 착용
* 발
* 액세서리

### **기본 신체 형태**

기본 아바타 예제 파일을 다운로드한 후, Blender 같은 3D 편집기에 모델을 불러오세요.

각 모델에는 아마추어와 연결된 8개의 서로 다른 메쉬가 포함되어 있습니다. 이 메쉬들은 머리, 눈썹, 눈, 입, 상체, 하체, 손, 발을 나타냅니다. 이 예제 모델들을 자신만의 맞춤 웨어러블을 위한 참고 자료이자 출발점으로 사용할 수 있습니다.

현재 신체 형태는 A와 B, 두 가지가 있습니다. 형태 A (`AvatarShape_A.blend`)는 **여성** 신체이며,  `BaseFemale` 표현에 사용됩니다. 형태 B (`AvatarShape_B.blend`)는 **남성** 신체이며,  `BaseMale`. 파일 이름만으로는 이것이 명확하지 않으므로, 제작 전에 반드시 확인하세요. 잘못된 파일로 만든 몸에 밀착되는 웨어러블은 두 표현이 뒤바뀐 상태로 끝나게 됩니다.

![](/docs/_remote/15019e8948fa233ffdf5f7bb.webp)

### **머리**

![](/docs/_remote/1108d14609f042d64adb1c53.webp)

*기본 머리에는 웨어러블로 사용자 지정할 수 있는 여러 메쉬가 부착되어 있습니다. 눈썹, 눈, 입은 얼굴 앞쪽에 렌더링되는 투명 마스크로 작동합니다.*

### **상체**

![](/docs/_remote/9ce8e60a3286071a888c6bd9.webp)

*상체, 즉 아바타의 몸통입니다. 손은 포함되지 않습니다.*

### **하체**

![](/docs/_remote/f12c5382a0abf31077a0dfda.webp)

*하체에는 아바타의 골반과 다리가 포함됩니다.*

### **손**

![](/docs/_remote/c61c01c445051846535b37fd.webp)

*손은 형태 A와 B에서 동일하며, 아바타의 손목에서 시작합니다.*

### **발**

![](/docs/_remote/4e09b05d153e3455e46bc344.webp)

*발에는 발목과 발이 포함됩니다.*


> [!WARNING]
> **중요: 머리, 상체, 하체 사이의 버텍스 "절단선/봉합선"은 수정하지 마세요.**


신체의 각 부분에는 캡이 있어 "방수" 상태를 유지합니다. 이 캡은 잘못된 스킨 웨이팅으로 인해 애니메이션 클리핑 문제가 발생하더라도 보기 흉한 오류를 방지하기 위해 존재합니다. 메쉬를 편집할 때는 이 캡을 제거하지 않는 것이 가장 좋습니다.

![](/docs/_remote/6d31c3917522878384fcbbf7.webp)

## 웨어러블용 3D 모델 만들기

#### **삼각형 수, 재질 및 텍스처 제한**

Decentraland가 모든 플레이어에게 원활하게 실행되도록 하려면, 삼각형, 재질, 텍스처를 너무 많이 사용하지 않고 웨어러블 모델을 만드는 것이 중요합니다. 목표는 너무 많은 디테일을 희생하지 않으면서 3D 모델을 가능한 한 단순하게 유지해 쉽게 렌더링되도록 하는 것입니다.

각 웨어러블 또는 액세서리에 사용할 수 있는 삼각형 수와 텍스처 수에는 제한이 있습니다:

* 모자, 헬멧, 상체, 하체, 발, 머리카락 등의 각 웨어러블 슬롯당 1.5K 삼각형을 초과할 수 없습니다.
* 마스크, 안경류, 귀걸이, 티아라, 머리 윗부분(top\_head), 얼굴 털 등 각 카테고리당 500개의 삼각형을 초과할 수 없습니다.
* 손 액세서리의 경우 예산은 1K tris입니다. 손 착용이 아바타의 기본 손을 가리는 경우 예산은 1.5K tris입니다.
* 웨어러블당 텍스처는 2개 이하(해상도 1024x1024px 이하)여야 합니다. 모든 텍스처는 정사각형이어야 하며 해상도는 72픽셀/인치여야 합니다.
* 재질은 2개 이하(AvatarSkin\_MAT는 제외)
* 피부 웨어러블의 경우 허용되는 tris 수는 5K이고 텍스처는 5개입니다.


> [!WARNING]
> **웨어러블 tris 결합기:**
> 
> *웨어러블이 다른 웨어러블을 가리는 경우, 제작자는 슬롯별 tris를 결합할 수 있습니다. 예를 들어 점프수트를 만들고 싶다면 상체 카테고리를 사용해 하체를 가리도록 만들 수 있습니다. 이 경우 1.5K*2= 3K 삼각형.\*
> 
> 헬멧의 경우, 모든 머리 착용물(머리, 귀걸이, 안경류, 티아라, 모자, 얼굴 털, 머리카락, 머리 윗부분)을 가리면 4K tris, 2개의 재질, 2개의 텍스처까지 도달할 수 있습니다)


#### 웨어러블의 최대 너비, 높이 및 깊이

웨어러블이 다른 플레이어의 화면 가시성을 방해하거나 씬 공간을 과도하게 침범하지 않도록 거리 제한이 있습니다.

웨어러블의 크기는 다음을 초과할 수 없습니다:

> `높이: 2.42m`, `너비: 2.42m`, `깊이: 1.4m`

![](/docs/_remote/0c4ec0aad4c553469f608e3f.webp) ![](/docs/_remote/74d98bf9b871410fa2fac083.webp)

#### **맵**

Decentraland 웨어러블은 현재 다음의 3가지 맵 유형을 지원합니다:

1. **기본 색상**: 이것은 모델의 색상과 디테일이 담긴 주요 텍스처입니다.
2. **발광**: 이 맵은 모델에서 빛나는 부분을 위한 것입니다. 발광 맵은 발광 전용으로 별도의 재질에 들어갑니다.
3. **알파**: 이 맵은 투명도를 처리하기 위한 것입니다. 텍스처의 불투명도 채널을 사용합니다. (*검정과 흰색 텍스처를 사용한 컷아웃에는 Alpha Blend보다 항상 Alpha Clip을 사용하는 것이 좋습니다*)

![](/docs/_remote/d9bad6330bf2adc9dd0f39cd.webp)

Decentraland 참고 클라이언트는 아바타 재질에 Toon Shader를 사용하므로, 일부 맵은 **불필요합니다** 예를 들면:

* **노멀 맵:** 표면 노멀을 RGB 값으로 인코딩하여 저폴리곤 모델의 고해상도 디테일을 시뮬레이션하는 데 사용되는 텍스처입니다.
* **러프니스 맵:** 3D 객체의 표면 거칠기를 정의하는 데 사용되는 텍스처입니다.

이 맵들을 Decentraland에서 사용하려면 한 텍스처로 베이킹하는 것이 해결책입니다. 텍스처 베이킹에 대한 자세한 정보는 여기에서 확인할 수 있습니다: <https://docs.blender.org/manual/en/latest/render/cycles/baking.html>

#### **노멀**

**Decentraland 엔진은 한쪽 면의 노멀만 렌더링합니다.** (즉, 평면은 한쪽 면에서만 보이고 반대쪽 면은 렌더링되지 않습니다) 따라서 노멀과 함께 3D가 완전히 올바른지 확인하려면 두 가지 방법으로 점검할 수 있습니다.

첫 번째 방법은 재질 속성 설정에서 "Backface culling"을 켜는 것입니다. 아래 이미지처럼 뒤집힌 노멀을 찾는 데 유용합니다:

![](/docs/_remote/0c50bdc39625df77edb130ed.webp) ![](/docs/_remote/19bd475725e7d546031c764d.webp)

두 번째 방법은 뷰포트 오버레이 옵션에서 "Face orientation"을 켜서 노멀이 올바른지 확인하는 것입니다. 모델이 파란색으로 바뀌지만 걱정하지 마세요. 파란 면이 올바른 면이고 빨간 면이 수정해야 하는 면입니다. 이 옵션은 여기에서 찾을 수 있습니다:

![](/docs/_remote/05e89b0f38d24052c2543b41.webp)

#### **아마추어**

제공된 아마추어의 사양, 명명 규칙, 계층 구조 또는 변형을 변경하지 않도록 기억하세요. 이 중 어느 하나라도 변경하면 내보낸 후 클라이언트에서 웨어러블이 작동하지 않게 됩니다.

![](/docs/_remote/8d4e69ae0e0d37a5a75dd0ec.webp)

가져온 아마추어에는 어떤 *끝* 또는 *중립* 본이 없어야 합니다. 그렇지 않으면 Builder로 내보낸 뒤 웨어러블이 작동하지 않습니다. 이 문제가 있는 .fbx를 가져오는 경우 ***Ignore Leaf Bones*** 를 가져올 때 켤 수 있습니다.

![](/docs/_remote/9bb431a3abead38036195f21.webp)


> [!NOTE]
> **스프링 본:** 웨어러블에는 동적 물리(예: 흔들리는 머리카락, 늘어지는 귀걸이)를 위해 기본 아마추어를 넘어서는 추가 본이 포함될 수 있습니다. 이 본들의 이름에는 `springbone` 이 포함되어야 합니다. 자세한 내용은 [스프링 본](/docs/creator/wearables-and-emotes/wearables/spring-bones/) 를 참조하세요.


#### **눈썹, 눈, 입**

이 메쉬들은 투명 셰이더와 함께 작동하므로, 원하는 새 눈썹, 눈 또는 입 스타일에 맞는 png 텍스처를 직접 만들고 UV 맵에 올바르게 배치하는 것 외에는 따로 할 일이 없습니다. 이 텍스처는 256x256px여야 하며 투명도를 위한 알파 채널이 있어야 합니다.

다음은 예시 png 텍스처입니다:

![](/docs/_remote/5574fe5585ace1ba7b8fc724.webp) ![](/docs/_remote/65a2cf2f85f2d9c2eceaf72b.webp) ![](/docs/_remote/a7245593b361bbcdb12ba303.webp)

*눈과 눈썹은 같은 메쉬와 UV 맵을 사용합니다.*

![](/docs/_remote/769020e9eaad71f8b782e641.webp) ![](/docs/_remote/2b2a15a2fadd5b4fa790102b.webp)

*입 메쉬와 UV 맵.*

최종 결과를 시각화하려면 (Blender에서) 다음 노드를 사용해야 합니다:

![](/docs/_remote/477f9ad97ab6c5847c84ba3a.webp) ![](/docs/_remote/7205c2f5e0ce6b631e182dbf.webp)

**마스크:** 아바타 에디터에는 플레이어가 아바타를 커스터마이즈할 때 선택할 수 있는 다양한 색상 옵션이 있습니다.

![](/docs/_remote/689357b1313432170c554166.webp)

이 색상 선택은 웨어러블의 특정 마스크에 적용됩니다.

![](/docs/_remote/859d860ec5e8ae3859344474.webp) ![](/docs/_remote/bc69b3aeb4af2645c8f66ddd.webp)

왼쪽 이미지의 검은 영역(눈 마스크)은 오른쪽 텍스처(눈 기본)에서 색이 입혀질 영역을 나타냅니다. 홍채는 항상 회색조여야 한다는 점을 기억하는 것이 중요합니다(홍채가 완전히 검정이면 틴트가 작동하지 않습니다. 반대로 홍채가 완전히 흰색이면 에디터에서 선택한 색으로 완전히 틴트됩니다).

#### **손 착용**

두 종류의 **손 착용** 동일한 카테고리에서 할 수 있습니다.

1. **손 대체:**

![](/docs/_remote/9affc36f5fddb68dcafbd2b3.webp)

양손을 대체하는 손 착용을 만들고 싶다면 아마도 ***손*** 기본 메쉬를 재정의해야 할 것입니다. 그렇게 하면 손 착용 웨어러블의 제한은 1500 tris가 됩니다.

2. **손 액세서리:**

![](/docs/_remote/f9dc813cad31b498851c90f6.webp)

시계, 팔찌, 반지 등과 같은 손 액세서리도 만들 수 있습니다. 이 경우 손은 재정의할 필요가 없으며 항목당 제한은 1000 tris입니다.


> [!WARNING]
> 참고: **손 착용** 카테고리는 손 액세서리, 손에 쥐는 아이템(도구나 소품 등), 그리고 손을 대체하는 아이템을 위한 것입니다. 이들은 모두 적절한 스키닝과 함께 아바타 아마추어를 따라가야 합니다. 이러한 사양을 따르지 않는 아이템을 제출하면 큐레이터 위원회에서 거부될 수 있습니다.


#### **머리카락과 수염**


> [!TIP]
> **팁:** 이제 머리카락과 다른 늘어지는 액세서리는 **스프링 본** 을 사용해 아바타의 움직임과 중력에 반응하여 동적으로 움직일 수 있습니다. 정적인 머리카락 대신 스프링 본 체인을 추가해 포니테일, 땋은 머리, 긴 머리카락이 자연스럽게 흔들리도록 만들 수 있습니다. 자세한 가이드는 [스프링 본](/docs/creator/wearables-and-emotes/wearables/spring-bones/) 를 참조하세요.


맞춤 머리카락 웨어러블을 만들 때 기억해야 할 중요한 두 가지가 있습니다.

첫째, 머리 모양을 따르도록 하세요. 시작점이 필요하다면 예제 파일에 제공된 머리 메쉬를 언제든 참고할 수 있습니다.

![](/docs/_remote/370336fb19510d9325816b5a.webp)

둘째, 사용자가 아바타 에디터에서 머리카락이나 수염의 색을 바꿀 수 있게 하려면 머리카락을 회색조로 칠하고 재질 이름에 "Hair"를 사용해야 합니다(예: "M\_Hair\_Short"). 틴트의 영향을 받지 않는 다른 객체를 포함하고 싶다면 그런 명명 규칙을 추가하지 마세요.

![](/docs/_remote/6c65a6133c0e05d828652d8e.webp) ![](/docs/_remote/cb8f9f3d94259277831a5b96.webp) ![](/docs/_remote/4b874f015973cb5a36ecc963.webp)

*회색의 낮은 톤은 더 어둡게 보이고, 높은 톤은 더 밝게 보이며, 아바타 에디터에서 사용자가 선택한 색상과 곱해집니다.*

### **기본 재질과 텍스처**

아바타 모델에는 세 가지 기본 재질이 있습니다. 하나는 웨어러블 자체에 사용되는 재질, 또 하나는 피부에 사용되는 재질, 그리고 나머지 하나는 눈썹, 눈, 입에 사용되는 재질입니다.

![](/docs/_remote/d83fc01227d8770cbf81986b.webp)

각 기본 메쉬에는 고유한 피부 텍스처가 함께 제공됩니다.

![](/docs/_remote/930d22b040b6df838d2d26f2.webp) ![](/docs/_remote/8dfa78f550fa2b8b6f32dfd0.webp)

피부 텍스처는 회색조로 만들어져 있어 렌더 엔진이 에디터를 통해 사용자의 선호에 따라 아바타의 피부에 틴트를 적용할 수 있습니다. 에디터에서 피부색을 틴트할 수 있으려면 재질 이름이 *AvatarSkin\_MAT*.

![](/docs/_remote/51dce26a37033db0987f4cbe.webp)


> [!WARNING]
> 이어야 합니다. 중요한 점: 반바지나 치마처럼 웨어러블에 의해 노출되는 신체 부위의 UV 매핑은 항상 유지하세요.


\src="../images/wearables-and-emotes/creating-wearables/19\_skin\_uv.png"width="600"/>

웨어러블용 맞춤 텍스처를 만들 수 있습니다! 하지만 각 웨어러블마다 작고 단일한 텍스처 파일 하나를 사용하는 것이 항상 가장 좋습니다. 예제 파일에 제공된 기본 AvatarWearable\_MAT 텍스처를 사용하면 웨어러블의 성능을 보장할 수 있습니다!

![](/docs/_remote/d821fd219afb4cca8fe745d3.webp)


> [!WARNING]
> 얼굴 특징 전용 셰이더가 실행되지 않도록 메쉬 이름에 '\_mouth,' '\_eyebrows,' 또는 '\_eyes'를 포함하지 마세요. 이러한 용어는 얼굴 특징용으로 예약되어 있으며, 부적절하게 사용하면 메쉬에 잘못된 셰이더가 적용됩니다.


✨ 모델에 직접 사용할 텍스처를 만들고 싶다면 더 나은 UV 언래핑을 위해 다음 애드온을 권장합니다:

**UVPacker**

UvPacker는 몇 번의 클릭만으로 UV를 패킹하고 정리하는 데 도움이 되는 무료 애드온입니다. Decentraland는 웨어러블에 최대 1024x의 텍스처를 사용하므로, 이 도구는 더 좋고 더 정돈된 텍스처를 만드는 데 큰 도움이 됩니다.

여기에서 웹사이트에서 직접 다운로드할 수 있습니다:

[**https://www.uv-packer.com/download/**](https://www.uv-packer.com/download/)

**UVToolKit**

UvToolKit은 Blender에서 뛰어난 UV를 만들 수 있도록 UV 설정과 옵션을 확장하는 데 도움이 되는 애드온입니다.

다운로드 링크는 다음과 같습니다: [**https://alexbel.gumroad.com/l/NbMya**](https://alexbel.gumroad.com/l/NbMya)

### **스킨 웨이팅**

스킨 웨이팅은 애니메이션 중 아바타 리깅의 어떤 본이 어떤 웨어러블에 영향을 주는지 결정하는 과정입니다.

새 웨어러블에 스킨 웨이팅을 적용할 때는 염두에 두어야 할 몇 가지 고려 사항이 있습니다.

각 에셋은 전체 스켈레톤에 웨이트가 적용되어야 합니다. 예를 들어 상체 에셋은 스킨 웨이트를 적용하면 다음과 같이 보입니다:

![](/docs/_remote/18e214351b78092a3436da29.webp)

신체 부위의 교차점에서 만나는 웨어러블은 반드시 같은 본에 완전히 웨이트가 적용되어야 합니다. 예를 들어 이 두 개의 초록색 영역에서는 목의 버텍스가 오직 "Neck" 본에만 완전히 웨이트되어야 합니다.

![](/docs/_remote/e5d072fd17af31865e1e1c4a.webp)

#### **핵심 본**

스킨 웨이팅에 사용할 "핵심" 본은 다음과 같습니다:

**머리 본:** 머리카락, 귀걸이, 티아라, 눈, 눈썹, 입, 그리고 머리의 움직임을 따라가야 하는 모든 액세서리에 사용됩니다.

**목 본:** 머리 본과 상체의 교차 버텍스에 사용됩니다.

**엉덩이 본:** 상체와 하체의 교차 버텍스에 사용됩니다.

**오른쪽 다리 및 왼쪽 다리 본:** 하체와 발의 교차 버텍스에 사용됩니다.

**오른쪽 팔뚝 및 왼쪽 팔뚝** 손의 교차 버텍스에 사용됩니다.


> [!WARNING]
> ⚠️ **힌트**
> 
> * 기억하세요, 어떤 본이든 어떤 메쉬의 버텍스에든 영향을 줄 수 있습니다! 예를 들어 키가 큰 부츠용 새 발 메쉬를 만들고, 부츠 윗부분을 "Leg Bones"에 스킨 웨이트할 수 있습니다. 또는 긴 머리카락을 만들어 아바타가 움직일 때 머리카락에 "Shoulder"나 "Spine" 본이 영향을 주도록 할 수도 있습니다.
> * 리깅의 양쪽이 대칭을 이루도록 유지하는 것이 항상 권장되며, 왼쪽과 오른쪽은 비슷한 본 영향을 가져야 합니다.
> * 일반적으로 버텍스는 4개 이상의 본 또는 조인트의 영향을 받을 수 없습니다.
> * 문서에 제공된 것과 정확히 같은 아마추어로 내보내는 것을 잊지 마세요. 만약 다음과 같은 다른 본이 있다면 *"\_end\_bones"* 또는 이와 유사한 것들이 있다면 클라이언트에서 작동하지 않습니다.


#### **강체 부분**

백팩, 제트팩 또는 갑옷 판처럼 구부러지면 안 되는 부분은 해당 부분의 모든 버텍스에 하나의 본만 100% 웨이트를 적용하세요. 예를 들어 `Avatar_Spine2` 를 등에 있는 모든 것에 사용하세요. 그런 다음 그 부분을 웨어러블의 메쉬에 결합하여 웨어러블이 하나의 스키닝된 메쉬로 유지되도록 합니다.

`Avatar_Spine2` 는 두 신체 형태 모두에서 같은 위치에 있으므로, 여기에 웨이트가 적용된 부분은 두 형태에서 같은 위치를 사용할 수 있습니다.

등의 강체 부분은 평평한 부분이 등에서 멀어지는 곡선 때문에 떠 있는 것처럼 보일 수 있습니다. 부품 전체 바닥면 아래의 간격을 측정하고 전면면을 등의 가장 돌출된 지점에서 약 4mm 떨어지게 배치하세요. 또는 부품과 등 사이에 작은 마운트 블록을 추가한 다음, 마운트의 전면 버텍스를 닿는 신체 버텍스처럼 웨이트하여 마운트가 척추와 함께 휘도록 하세요.

#### **웨어러블 내보내기**

웨어러블을 내보낼 때는 제공된 Armature 외부에 다른 본이 없도록 하세요, **단, 스프링 본은 제외합니다** (즉, 이름에 `springbone` 가 들어 있는 본으로, 동적 물리에 사용됩니다). 서로 다른 소프트웨어 간 아마추어를 가져올 때 흔한 문제는 *"\_end"* 또는 *"\_neutral"* 본이 생성되는 것입니다. 내보내기 전에 반드시 제거하세요. 그렇지 않으면 이후 클라이언트에서 웨어러블이 작동하지 않을 가능성이 매우 높습니다.

![](/docs/_remote/c6477cfc83e060c6483735ac.webp)

웨어러블을 내보내려면 오브젝트를 선택한 다음 아마추어를 선택하세요. 카메라, 조명 또는 빈 오브젝트처럼 다른 것은 내보내지 않도록 주의하세요.

단일 메쉬로 구성된 유효한 웨어러블은 내보낸 파일에 64개의 노드를 가집니다: 그 `아마추어`, 62개의 본과 스키닝된 메쉬입니다. 스프링 본은 각각 하나의 노드를 추가합니다.

다음으로, 웨어러블을 *glTF2.0* 형식으로 내보내세요. 스키닝 속성이 있는 웨어러블만 내보내고, 애니메이션이나 쉐이프 키처럼 불필요한 기능은 포함하지 마세요.

![](/docs/_remote/f71be0c8670ef1406a96e0f4.webp)

## 모델링을 위한 좋은 습관

### **메쉬를 T-포즈에서 A-포즈로 변경하기**

웨어러블을 만들 때 최종 결과를 시각화하고, 토폴로지와 웨어러블 위치를 다루기 쉽게 하는 가장 좋은 방법은 모델을 A-포즈로 작업하는 것입니다. 이를 위해 다음의 간단한 단계를 따르세요:

1. 먼저 상체를 선택한 다음 "**편집 모드**"와 "**On cage**"을 아마추어 모디파이어에서 켜야 합니다.

![](/docs/_remote/9dd9e778254d10a239d518c8.webp)

2. 이제 포즈 모드에서 팔을 60° 회전할 수 있습니다.

![](/docs/_remote/41758037910791f5efcffe51.webp)

3. T-포즈 대신 A-포즈에서 메쉬를 편집할 수 있습니다.

![](/docs/_remote/71276e7651ff3507da8ee9fe.webp)

4. 하지만 아마추어 모디파이어에서 "편집 모드"와 "On cage"를 다시 켜기만 하면 A-포즈에서 T-포즈로 쉽게 전환할 수 있다는 점도 기억해 두는 것이 좋습니다.

![](/docs/_remote/a731a1c3b9056b0a3f73203e.webp)

### **관절 변형:**

관절(예: 팔이나 다리)에 관해 웨어러블 토폴로지에서 최상의 결과를 얻으려면 루프를 만들 때 좋은 관행을 따르는 것이 중요합니다. 여기에서 서로 다른 루프 컷에 따른 메쉬 변형의 차이를 볼 수 있습니다:

![](/docs/_remote/c18bb25e36a973c241938296.webp)

모든 것이 올바르게 변형되고 있는지 확인하는 좋은 방법은 다음 예제처럼 웨이트 페인트를 하는 것입니다:

![](/docs/_remote/0e4eb4a84b57c9c5a3f1d079.webp) ![](/docs/_remote/a6d3efa7b12801dd5b29ed8b.webp)

### **치마**

치마/드레스를 모델링할 때 유용한 팁이자 좋은 습관은 주름이 만나는 지점에 추가 루프컷을 넣는 것입니다. 리그의 웨이트를 칠할 때 매우 유용합니다.

![](/docs/_remote/20d43358a97c94b9db32014e.webp)

이 루프컷을 사용하면 버텍스 영향이 훨씬 더 부드럽게 보이며 치마/드레스를 애니메이션할 때 더 나은 결과를 얻을 수 있습니다.

본 영향이 어떻게 되어야 하는지에 대한 예시는 다음과 같습니다:

![](/docs/_remote/3a762bb515429e86217673cd.webp)

### **모자**

모자를 만들 때 좋은 습관은 모자의 기본 메쉬에 머리카락을 추가한 다음 에디터에서 카테고리 *머리카락* 를 숨기는 것입니다. 이렇게 하면 모자가 다른 머리카락과 겹치는 것을 방지하고 예기치 않은 결과를 줄일 수 있습니다.

![](/docs/_remote/52f717e012928949c63883f0.webp)

### **폴리곤 수 추가**

유용한 팁은 모델의 폴리곤 수를 항상 추적하는 것입니다. Blender에서 이를 하려면 뷰포트 오버레이 패널에서 통계를 켜야 합니다.

![](/docs/_remote/444dbdc043e94bf60c83af44.webp)

### 리소스

이 공유 폴더에서 기본 모델, 텍스처, 그리고 완전히 제작된 웨어러블 예제 등 다양한 리소스를 찾을 수 있습니다. 자신만의 것을 만들 때 이 리소스들을 자유롭게 활용하세요.

[**웨어러블 참고 모델**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/wearables/creating-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
