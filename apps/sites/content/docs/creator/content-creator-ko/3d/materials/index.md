# 재질

Decentraland로 가져온 3D 모델에서 지원되는 재질 속성과 텍스처를 알아봅니다.

재질은 다음에 내장됩니다 *.gltf* 또는 *.glb* 파일을 추가할 수 있습니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 유용한 여러 기능이 포함되어 있습니다.


이 문서는 3D 모델에 가져온 재질을 참조합니다. 프리미티브 형태에 적용할 코드로 정의된 재질은 다음을 참조하세요. [**머티리얼**](https://docs.decentraland.org/creator/development-guide/materials/) .


> [!WARNING]
> **📔 참고**: 현재는 이것이 프리미티브 형태가 아닌 한, 씬 코드에서 3D 모델의 재질을 동적으로 변경할 수 없습니다.


## 셰이더 지원

Decentraland로 가져오는 모든 모델에서 셰이더를 사용할 수 있는 것은 아닙니다. 다음 중 하나를 사용해야 합니다:

* **표준 재질:** 모든 셰이더가 지원됩니다. 예: diffuse, specular, transparency 등.
* **PBR(물리 기반 렌더링) 재질**: 이 셰이더는 diffuse, roughness, metalness, emission 같은 속성을 포함하므로 더 유연하며, 재질이 빛과 상호작용하는 방식을 설정할 수 있습니다.

아래 이미지는 같은 색상과 텍스처로 생성된 두 개의 동일한 모델을 보여줍니다. 왼쪽 모델은 모든 *PBR* 재질을 사용하며, 그중 일부는 *금속성*, *투명도*, 그리고 *발광성*을 포함합니다. 오른쪽 모델은 모든 *표준* 재질을 사용하며, 그중 일부는 *투명도* 및 *발광성*.

![](/docs/_remote/129845688fefc65d479ded1d.webp)

### 현재 Decentraland 엔진에서 작동하는 PBR 속성

> * 기본 색상
> * 금속성
> * 거칠기
> * 스페큘러
> * 발광
> * 발광 강도
> * 알파
> * 노멀

![](/docs/_remote/9edb0c21919b4803373b3885.webp)

이러한 속성이 월드에서 어떻게 동작하는지 확인하려면 다음 [테스트 월드](decentraland://?realm=TestEnvironment.dcl.eth) 로 이동하여 다양한 오브젝트와 재질이 빛 및 월드 환경과 어떻게 상호작용하는지 확인할 수 있습니다.

![](/docs/_remote/1abdb0c1779701d95d2637dc.webp)

### 기본/확산 색상

오브젝트 표면의 기본 색상을 정의합니다. 이 자체로는 조명의 영향을 받지 않기 때문에, roughness, metallic, specular 등과 같은 다른 노드와 함께 결합됩니다.

### 금속성

메탈릭 셰이더는 금속 표면의 외형을 시뮬레이션하는 데 사용되는 렌더링 기법의 한 유형을 말합니다. 메탈릭 셰이더는 금속의 물리적 특성과 빛과의 상호작용을 고려하여 금속 특유의 반짝이고 반사되는 품질을 만들어냅니다.

![](/docs/_remote/b601bec72f14836b99d6f451.webp)

### 거칠기

거칠기 재질은 빛이 재질 표면과 어떻게 상호작용하는지에 대한 사실적인 시뮬레이션과 관련이 있습니다. 일반적으로 거칠기 맵은 모델 표면에 "매끄러움" 또는 "거칠기"의 범위를 부여하는 데 사용됩니다. 이러한 유형의 정보를 제공하기 위해 일반적으로 그레이스케일 값의 텍스처 맵이 사용됩니다.

![](/docs/_remote/c4e81d0781e9fcd132dbed8f.webp)

이 속성은 비금속 재질 모델과 금속 재질 모델 사이를 혼합합니다. 값이 1.0이면 확산 반사나 투과 없이 기본 색상으로 물들어진 완전한 스페큘러 반사를 제공합니다. 0.0이면 재질은 확산 또는 투과 기본층과 그 위의 스페큘러 반사층으로 구성됩니다.

### 스페큘러

물리 기반 렌더링(PBR) 셰이더에서 스페큘러 속성은 빛이 표면의 반사성과 광택 측면에서 어떻게 상호작용하는지를 의미합니다. 스페큘러 반사는 표면에서 거울처럼 반사되는 빛을 말합니다. PBR에서는 이 속성을 사용해 표면이 얼마나 거울처럼 빛을 반사하는지 제어합니다. 금속과 같은 재질은 일반적으로 높은 스페큘러 반사를 가지며 선명하고 밝은 하이라이트를 만드는 반면, 플라스틱 같은 비금속 재질은 더 낮은 스페큘러 반사를 가져 더 넓고 부드러운 하이라이트를 만듭니다.

![](/docs/_remote/92855ba60b2140bd9d3977ad.webp)

### 알파

재질을 *투명하게*설정할 수 있습니다. 투명 재질은 그 *알파*이렇게 하려면 재질의 투명도 속성을 활성화한 다음 그 *알파* 을 원하는 값으로 설정하세요. 알파가 1이면 재질은 완전히 불투명해지고, 알파가 0이면 보이지 않게 됩니다.

아래 이미지는 표준 재질로 생성된 두 개의 동일한 모델을 보여줍니다. 왼쪽 것은 불투명 재질만 사용하고, 오른쪽 것은 일부 부분에 투명 재질과 불투명 재질을 모두 사용합니다.


> [!WARNING]
> 💡 투명 재질을 사용하는 것은 확산 재질보다 항상 성능 면에서 더 비용이 큽니다. 투명 재질은 항상 가능한 한 적게 사용하도록 하세요.


![](/docs/_remote/69e874b1001f67ad59d3ac60.webp)

주요 투명도 모드는 두 가지가 있습니다: *알파 클립* 및 *알파 블렌드*입니다. 주요 차이점은 다음과 같습니다:

* **알파 클립:** 알파 클립은 그레이스케일 값의 클립 임계값을 기준으로 값을 0 또는 1의 절대값으로 렌더링합니다. 이전 색상은 표면 색상으로 덮어써지지만, 알파 값이 클립 임계값보다 클 때만 그렇습니다.

![](/docs/_remote/6e885f9bbcbfca598d148a89.webp)

* **알파 블렌드:** 알파 블렌드는 0과 1 사이의 값을 보간합니다. 알파 블렌딩을 사용하면 이전 색상 위에 표면 색상을 겹쳐서 표시할 수 있습니다.

![](/docs/_remote/293aba2fd75ef42086bd5c31.webp)


> [!WARNING]
> **🔥최적화 팁🔥** 중간 정도의 투명도가 꼭 필요하지 않다면, **렌더링 성능상 항상 \_Alpha Clip***\*\* 대신 \*\****알파 블렌드**\_**.**



> [!WARNING]
> **⚠️ 정렬 문제**
> 
> 게임에서 투명 블렌드 모드를 사용할 때는 색상 블렌딩이 이루어지는 순서를 고려하는 것이 매우 중요합니다. 최종 출력 색상은 블렌딩 순서에 의해 크게 영향을 받을 수 있기 때문입니다. 현재 엔진은 오브젝트 단위 정렬만 지원하므로, 오브젝트 원점에 따라 모든 투명 표면을 자동으로 정렬합니다. 그러나 픽셀 단위 정렬과 삼각형 단위 정렬은 현재 지원되지 않습니다.
> 
> 정렬과 관련된 문제를 피하려면, 같은 메시에서 알파 클립과 알파 블렌드를 함께 사용하는 오브젝트는 피하는 것이 좋습니다. 이렇게 하면 예상치 못한 블렌딩 아티팩트를 방지하고 게임이 최상의 모습으로 보이게 할 수 있습니다.


### 발광

또한 재질을 *발광성으로*설정할 수 있습니다. 발광 재질은 스스로 빛을 냅니다. 렌더링될 때 실제로 씬의 가까운 오브젝트들을 비추는 것은 아니며, 그저 주변에 흐릿한 빛이 있는 것처럼 보일 뿐입니다.

아래 이미지는 표준 재질로 생성된 두 개의 동일한 모델을 보여줍니다. 오른쪽 것은 일부 표면에 빛나는 발광 재질을 가지고 있습니다.

![](/docs/_remote/6a611f35617386705cb8bcce.webp) ![](/docs/_remote/4deead5b2aeaa81746397cb9.webp)

*Blender에서 재질을 발광성으로 만들려면, 단순히 재질에 `emission` 셰이더를 추가하면 됩니다.*

### 발광 강도

방출되는 빛의 세기입니다. 값이 1.0이면 이미지의 오브젝트가 Emission Color와 정확히 같은 색상을 갖게 되며, 즉 ‘그림자 없음(shadeless)’ 상태가 됩니다.

![](/docs/_remote/5392ea458ab888ee365b3678.webp)

*다음에서* [*테스트 월드*](decentraland://?realm=TestEnvironment.dcl.eth) *월드에서 발광 강도가 어떻게 동작하는지*

### 노멀

PBR 셰이더의 "normal" 노드는 재질의 표면 노멀을 제어하는 데 사용되는 기본 구성 요소입니다. 노멀은 특정 지점에서 표면에 수직인 방향을 정의하는 벡터이며, 빛이 표면과 어떻게 상호작용하는지 결정하는 데 매우 중요한 역할을 합니다.

![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

## 버텍스 페인팅

3D 모델의 버텍스 페인팅은 현재 Decentraland 엔진에서 지원되지 않습니다.

## 재질 제한

파라셀당 재질 제한은 다음과 같습니다:

> log2(n+1) x 20 씬의 재질 수. 모델의 일부로 가져온 재질도 포함됩니다.

각 재질은 오브젝트당 하나의 드로우 콜을 의미하므로, 재질 수를 가능한 한 최소로 유지하고 텍스처 아틀라스와 같은 기법을 사용해 재질을 최대한 재사용하는 것이 매우 중요합니다. 이것은 또한 씬의 에셋들 사이에 일관된 스타일을 갖게 하여 씬에 도움이 됩니다.

## 재질 명명

정돈되고 건강한 아트 파이프라인을 위해 재질 이름을 적절하게 지정하는 것을 권장합니다. 이를 위한 한 가지 방법은 이 명명 규칙을 사용하는 것입니다.

```
<Object>_<Classification>_<Sub-Classification(optional)>_<_MAT>
```

예를 들어, 봄용으로 발광하고 빛나는 나무와 겨울용으로 차갑고 금속성인 다른 나무 2개를 만들었다고 해봅시다. 재질 이름은 다음과 같이 지을 수 있습니다: *"TreeSpring\_Emissive\_MAT"* 그리고 또 하나는 *"TreeWinter\_Metallic\_MAT"*

결론적으로,

* 🟢 **다음을 사용하는 것을 권장합니다** 오브젝트와 분류로 시작하는 이름: *"Wood\_Oak\_MAT"*, *"SciFiFence\_Metallic\_MAT"*, 등.
* 🔴 **피해야 할 것** 다음과 같은 이름 사용 *"Material009"*, *"material1"*&#xC740; 씬과 모델을 추적하고 분석하기 매우 어렵게 만듭니다.

## glTF 재질 덮어쓰기

다음의 재질을 덮어쓸 수 있습니다. *glTF* 모델의 [GltfNodeModifiers](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 컴포넌트를 사용하여 씬 코드에서 그렇게 할 수 있습니다. 다음을 참조하세요. [glTF 재질 수정](/docs/creator/content-creator-ko/sdk7/3d/materials/#modify-gltf-materials) 자세한 내용은.

## 재질 모범 사례

* 씬에 같은 텍스처를 사용하는 여러 모델이 포함되어 있다면, 텍스처를 3D 모델 안에 내장하지 말고 외부 파일로 참조하세요.

내장된 텍스처는 각 모델마다 복제되어 씬의 크기를 증가시킵니다. *.glb* 파일은 기본적으로 텍스처가 내장되어 있지만, [**glTF 파이프라인**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline) 을 사용해 외부로 추출할 수 있습니다.

> 참고: 내장되지 않을 텍스처용 파일을 참조한 후에는, 그 파일이 이동되거나 이름이 변경되지 않도록 주의하세요. 그렇지 않으면 파일에 대한 참조가 끊깁니다. 또한 해당 파일은 씬 폴더 안에 있어야 씬과 함께 업로드됩니다.

* 재질의 투명도를 설정할 때는 항상 *알파 클립* 대신 *알파 블렌드*를 사용하도록 하세요. 단, 유리처럼 부분적으로 투명한 재질이 특별히 필요할 때는 예외입니다. 이렇게 하면 엔진이 잘못된 모델을 다른 모델 앞에 렌더링하는 문제를 피할 수 있습니다.
* 경험칙으로, 항상 *백페이스 컬링* 을 재질에 설정하는 것을 기억하세요. 이렇게 하면 엔진이 모델의 보이는 면만 렌더링하므로 씬의 성능이 더 좋아집니다. 단, *백페이스 컬링* 모델을 양면으로 렌더링해야 하는 경우에만 해제하세요(예: 3D 평면으로 만든 나뭇잎 묶음).

![](/docs/_remote/83df362d7fc9ab62e2ae130d.webp)

* Decentraland [**기본 텍스처를 사용하세요**](https://github.com/decentraland/builder-assets/tree/master/textures) . 이는 플레이어에게 미리 로드되어 있어, 에셋 렌더링이 훨씬 빨라집니다.
* 읽어보세요 [**이 문서**](https://www.khronos.org/blog/art-pipeline-for-gltf) 는 glTF 모델에서 PBR 텍스처를 사용하는 전체 아트 파이프라인에 대한 자세한 개요를 제공합니다.
* Blender에서 glTF 호환 재질을 만드는 방법에 대한 자세한 참조는 [**Blender 문서**](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html) .
* 에서 무료의 고품질 PBR 텍스처를 찾아보세요. [**cgbookcase**](https://cgbookcase.com/) .


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/3d/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
