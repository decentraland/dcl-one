# 콜라이더

Decentraland로 가져온 3D 모델에 콜라이더를 추가하는 방법을 알아봅니다.

3D 모델과 씬의 사용자 간 충돌을 활성화하려면 모델에 콜라이더를 부여해야 합니다. 콜라이더가 없으면 플레이어는 모델이 없는 것처럼 통과할 수 있습니다. 콜라이더는 오브젝트가 플레이어의 포인터 이벤트에 반응하거나 이를 차단하고, 레이캐스트와 교차하거나 이를 차단하게 하는 데도 필요합니다. 성능상의 이유로 콜라이더는 보이는 지오메트리보다 정점 수가 적은 단순한 보이지 않는 지오메트리로 별도로 모델링되는 경우가 많습니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 도움이 되는 여러 편리한 기능이 포함되어 있습니다.


모델은 콜라이더로 사용할 보이지 않는 지오메트리를 사용할 수도 있고, (SDK7부터는) 보이는 지오메트리가 콜라이더로 작동하도록 대신 할당할 수도 있습니다. 다음을 참조하세요 [**3D 모델의 콜라이더**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) . 또한 서로 다른 지오메트리에 서로 다른 충돌 레이어를 할당하여 플레이어 물리, 포인터 이벤트 및 기타 사용자 지정 레이어를 별도로 제어할 수 있습니다. 다음을 참조하세요 [**3D 모델의 콜라이더**](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers) .

콜라이더는 모델과 엔티티가 서로 상호작용하는 방식에는 영향을 주지 않으며, 항상 서로 겹칠 수 있습니다. 콜라이더는 모델이 플레이어의 아바타와 상호작용하는 방식에만 영향을 줍니다.

Decentraland 씬에서 오브젝트를 콜라이더로 인식하려면 특정 방식으로 이름을 지정하기만 하면 됩니다. 오브젝트 이름 끝에는 접미사 “\_collider”가 포함되어야 합니다.

예를 들어 가로등 기둥용 콜라이더를 만들려면 기둥 몸통을 둘러싼 단순한 상자 오브젝트를 만들 수 있습니다. 씬의 플레이어는 이 상자를 볼 수 없지만, 이것이 이동 경로를 막습니다.

![](/docs/_remote/872cedc3afd092432696701c.webp)

이 경우 상자의 이름을 “LampPost*collider”로 지정하고 나무와 상자를 하나의 \_\_gltf 모델로 함께 내보낼 수 있습니다. \*\_\_collider*\* 태그는 상자 오브젝트가 콜라이더 컬렉션에 속함을 Decentraland 월드 엔진에 알리고, 콜라이더 메시를 보이지 않게 만듭니다.

플레이어가 씬에서 가로등 기둥 모델을 볼 때마다 복잡한 가로등 모델을 보게 됩니다. 하지만 그 안으로 걸어 들어가면 나무가 아니라 상자와 충돌하게 됩니다.

## 플레이어 공간 제약

플레이어가 걸어 다니도록 설계된 모델을 만들 때는 다음 기준 치수를 염두에 두어야 합니다:

* 플레이어는 점프하지 않고 약 40cm 높이의 단차까지 걸어 올라갈 수 있습니다. 45cm 이상의 단차는 점프해야 합니다. 편안한 계단을 위해 각 단은 높이 25cm 이하, 깊이 최소 30cm로 유지하세요.
* 플레이어는 점프하지 않고 최대 46도 경사의 비탈을 걸어 올라갈 수 있습니다. 더 가파른 표면에서는 아바타가 미끄러집니다. 쉽게 걸을 수 있도록 경사로는 30도 이하로 유지하세요. 계단 위의 경사로 콜라이더는 예외입니다. 이는 계단의 경사를 따르며, 높이 25cm, 깊이 30cm인 단의 경우 약 40도입니다(다음 참조 [계단에 콜라이더 추가](#add-a-collider-to-a-staircase)).
* 플레이어는 최대 1.8m 높이까지 점프할 수 있습니다
* 아바타의 키는 약 1.9m이지만, 머리에 모자나 다른 웨어러블을 착용할 수 있다는 점을 고려하세요.
* 아바타의 콜라이더는 폭 0.6m(반지름 0.3m)의 캡슐입니다. 약 1.85m보다 낮은 보나 천장은 아바타를 막습니다. 플레이어가 아래를 지나는 모든 곳에 최소 2m의 여유 높이를 확보하세요. 실내에서는 카메라를 위해 더 많은 공간을 남겨두세요. 다음을 참조하세요 [실생활보다 더 크게 실내를 설계하세요](/docs/creator/content-creator-ko/sdk7/designing-the-experience/design-games/#build-interiors-larger-than-real-life).

플레이어가 씬에서 나무 모델을 볼 때마다 복잡한 나무 모델을 보게 됩니다. 하지만 나무 안으로 걸어 들어가면 나무가 아니라 상자와 충돌하게 됩니다.

### 계단에 콜라이더 추가

계단은 콜라이더 오브젝트의 매우 일반적인 사용 사례입니다. 플레이어가 계단을 오르려면 플레이어가 밟을 수 있는 해당 \_collider 오브젝트가 있어야 합니다.

계단 콜라이더에는 경사로 오브젝트를 사용하는 것이 좋습니다. 이렇게 하면 오르내릴 때 훨씬 더 나은 경험을 제공합니다. 계단을 오를 때 각 단을 하나하나 “점프”해야 하는 대신 부드럽게 올라가거나 내려가는 것처럼 보입니다.

경사로 오브젝트를 사용하면 불필요한 지오메트리 생성도 피할 수 있어 더 복잡한 다른 모델을 위한 공간을 절약할 수 있습니다. 콜라이더 지오메트리도 다음을 계산할 때 고려된다는 점을 유의하세요 [**씬 제한 사항**](/docs/creator/sdk7/optimizing/scene-limitations/)

1. 원래 계단의 크기와 비율을 닮은 경사로 모양의 새 오브젝트를 만드세요.

![](/docs/_remote/8162fc04d84389328f79ddb3.webp)

2. 경사로 오브젝트의 이름을 S와 유사하게 지정하세요*tair\_collider*. 이름은 \_로 끝나야 합니다*collider*.
3. 경사로 오브젝트가 계단과 같은 공간을 차지하도록 계단 위에 겹치게 배치하세요.

![](/docs/_remote/fab0dd4c3439215495a72aa2.webp)

4. 두 오브젝트를 하나의 *glTF* 모델로 함께 내보내세요.

이제 플레이어가 씬의 계단을 볼 때는 더 정교한 계단 모델을 보지만, 계단을 오를 때는 경사로와 충돌합니다.

## 콜라이더 최적화

* 다음을 우선적으로 사용하세요 **상자**, **구체** 그리고 **캡슐** 충돌 메시를 사용하세요. 로딩 시와 런타임에서 성능이 더 좋습니다.
* 복잡한 충돌 메시는 여러 개의 프리미티브 충돌 메시로 근사하는 것을 고려하세요(**상자**/**구체**/**캡슐**)

![](/docs/_remote/195d7d652ad646f33e815b49.webp)

*예: 링용 콜라이더는 여러 캡슐 또는 상자 콜라이더로 구성할 수 있습니다*

* 충돌 메시는 관련된 오브젝트의 계층 구조 안에 배치하세요. 예를 들어 Spaceship\_collider는 계층 구조 관점에서 Spaceship 부모 내부의 자식이 될 수 있습니다

### 볼록 콜라이더와 오목 콜라이더

임의 메시 콜라이더를 사용하는 것이 유일한 방법이라면 충돌 메시를 볼록하게(오목하지 않게) 만드세요. 이렇게 하면 끊김, 콜라이더 내부에 끼임 또는 이를 통과하는 현상과 같은 이상한 물리 상황을 방지할 수 있습니다. 오목 메시는 여러 개의 볼록 메시로 모방할 수 있는 경우가 많습니다.

![](/docs/_remote/f40bda0817152df8a3cd5005.webp)

## 충돌 이름 지정

**의미 있는 이름을 사용하세요. 이름은 에셋이 사용되는 위치 또는 오브젝트의 어떤 부분과 관련되는지에 대한 맥락을 제공해야 합니다.**

에셋 이름 지정에는 다음을 혼합하여 사용합니다 `PascalCase` 그리고 `snake_case`, 이를 다음과 같이 부를 수 있습니다 `PascalSnake_case`. 기본 규칙은 새 단어 또는 구분자 뒤의 단어(`_`)가 다음으로 시작한다는 것입니다 **대문자.**

충돌 메시는 Explorer에서 콜라이더를 생성하기 위한 참조로 사용되는 메시입니다

* 위의 메시와 동일한 원칙을 따르지만 다음을 가집니다 `_collider` 끝의 접미사 **예시:** 🟢 **권장** 이름 - `Tram_Wheel_LeftFront_collider` 🔴 **피하세요** 이름 - `Primitive.001_Collider`, `collider (1)`, `Cylinder.007_collider`,


> [!WARNING]
> **🔥최적화 팁🔥**
> 
> 콜라이더를 상자, 구체 또는 캡슐 형태로 근사할 수 있다면 이 3가지 변형 중 하나인 \_box, \_sphere 또는 \_capsule을 추가 접미사로 사용하세요. 충돌 감지에 임의 메시 대신 이 3가지 형태를 사용하면 씬 성능과 로딩 시간이 향상됩니다 💪🚀
> 
> 예시: 🟢 Teater\_MainWall\_North\_collider\_box, Fountain\_Statue\_collider\_sphere


## 콜라이더 모범 사례

* **콜라이더를 만들 때는 항상 가능한 한 가장 적은 수의 삼각형을 사용하세요.** 복잡한 오브젝트를 복사하여 콜라이더로 사용하는 것은 피하세요. 단순한 콜라이더는 좋은 사용자 경험을 보장하고 씬을 삼각형 제한 내에 유지합니다.
* **콜라이더 오브젝트에는 머티리얼이 없어야 합니다**, 씬의 플레이어는 이를 절대 볼 수 없기 때문입니다. 콜라이더는 플레이어에게 보이지 않습니다.
* **모든 콜라이더 오브젝트 이름은 \_로 끝나야 합니다*****collider*****.** 예를 들어, T*ree\_collider*.
* **\_plane을 콜라이더로 사용하는 경우**\_\*\*는 한 방향에서만 차단합니다.\*\* 예를 들어 벽처럼 콜라이더가 양쪽에서 차단하게 하려면 법선이 서로 반대 방향을 향하는 두 개의 평면을 만들어야 합니다.
* 콜라이더 오브젝트를 복제할 때는 이름에 주의하세요. 일부 프로그램은 중복을 피하기 위해 이름 끝에 \_를 추가합니다*1* 예를 들어 중복을 피하기 위해 파일 이름 끝에 추가합니다 *tree\_collider\_1*. 이와 같이 이름이 지정된 오브젝트는 Decentraland 월드 엔진에서 콜라이더가 아닌 일반 오브젝트로 해석됩니다.
* 형상을 보이지 않는 것으로 근사하는 `MeshCollider` 컴포넌트를 엔티티에 추가하면 콜라이더 메시 추가를 피할 수 있습니다.
* 다음을 구성하여 콜라이더 메시 추가를 피할 수도 있습니다 `GLTFContainer` 컴포넌트가 보이는 지오메트리를 콜라이더로 사용하도록 설정하세요. 다음을 참조하세요 [**3D 모델의 콜라이더**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) .
* 각각 콜라이더가 필요한 메시 형태가 많은 3D 모델을 만드는 경우 도움이 되는 편리한 애드온이 있습니다. Blender에서는 Blender 내부의 일괄 이름 변경 옵션을 사용하여 \*\*\*\*자동으로 다음을 추가할 수 있습니다 `_collider` 그룹 내 모든 메시 이름에 접미사를 추가합니다. 이 도구에 대한 자세한 내용은 “**에셋 이름 변경에 Blender 사용” 섹션**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/3d/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
