> Decentraland에서 텍스처가 작동하는 방식을 알아봅니다

# 텍스처

텍스처는 장면에 원하는 룩앤필을 구현하기 위한 3D 아트 파이프라인의 핵심 요소입니다. 이 섹션에서는 텍스처를 만드는 데 필요한 모든 정보, 제한 사항, Blender의 노드, 그리고 최상의 성능을 내도록 최적화하는 방법을 확인할 수 있습니다!


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 유용한 여러 기능이 포함되어 있습니다.


## 제한 사항

### 텍스처 크기 제한

현재 Decentraland Explorer는 장면이 콘텐츠 서버에 업로드된 후 Asset Bundle Converter를 사용하여 최적화 목적으로 텍스처를 최대 1024px까지 압축합니다. 에셋을 만들 때 이 제한을 반드시 고려하세요!

또 하나 중요한 점은 텍스처는 항상 2의 거듭제곱 크기여야 하며, 이 규격을 따르지 않는 텍스처는 장면 렌더링 시 문제를 일으킬 수 있습니다.

텍스처 크기는 다음 숫자와 일치하는 너비와 높이 값(픽셀)을 사용해야 합니다:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> 이 수열은 2의 거듭제곱으로 이루어져 있습니다: `f(x) = 2 ^ x` . **1024는 텍스처 크기로 허용하는 최대값입니다.** 이는 다른 렌더링 엔진에서도 흔히 요구되는 사항이며, 그래픽 프로세서의 내부 최적화 때문입니다.

너비와 높이가 같은 숫자일 필요는 없지만, 둘 다 이 수열에 속해야 합니다.

**텍스처의 권장 크기는 1024x1024입니다.**, 이것이 국내 네트워크를 통해 전송하기에 가장 적합하고 적절한 로딩/품질 경험을 제공하는 최적의 크기임을 확인했습니다.

다른 유효한 크기 예시:

```
32x32px
64x32px
512x256px
512x512px
1024x1024px
```

반면, 파셀당 텍스처 수에는 제한이 있습니다:

```
log2(n+1) x 10 파셀당 텍스처 수입니다. 모델의 일부로 가져온 텍스처도 포함됩니다.
```

## UV 매핑

UV 매핑은 3D 모델의 면을 나중에 각종 맵을 에셋에 적용할 때 사용할 2D 좌표로 펼치는 과정입니다. 이는 제작 과정의 핵심 단계입니다. 모델을 올바르게 언래핑하는 것은 모델의 해상도를 최대한 활용하는 데 중요한 요소이며, 맵을 수정하기 쉽게 정리해 줍니다.

UV 언래핑에 대해 더 자세히 알고 싶다면 Blender Foundation에서 만든 이 멋진 영상을 볼 수 있습니다:

[![Video Preview](/docs/_remote/b25b76a5b0e4c1d0cec817e8.webp)](https://youtu.be/Y7M-B6xnaEM?si=qMnWTMsXxC-vxZAH)

또 다른 훌륭한 중급 수준의 UV 언래핑 튜토리얼로, 더 복잡한 모델을 어떻게 언래핑하는지 설명하는 \[Blender Guru] (<https://www.youtube.com/@blenderguru)의> 영상도 있습니다:

[![Video Preview](/docs/_remote/2b79a21fed4d0ba2abc23bfa.webp)](https://youtu.be/scPSP_U858k?si=Uw0xHbv9jtqVstDS)

## 맵

다음에서 [재질 섹션](/docs/creator/3d-modeling/materials/) 이전에는 Decentraland가 PBR 셰이더와 어떻게 동작하는지 설명했습니다. 이 섹션에서는 각 텍스처 맵이 셰이더와, 나중에 월드로 내보내질 3D 오브젝트의 룩앤필을 어떻게 바꾸는지 보여드리겠습니다.

## 디퓨즈 맵

이는 오브젝트 표면의 기본 색상입니다. 모델 간에 균형 잡힌 색상 팔레트를 유지하는 것은 경험 전반의 일관된 룩앤필을 만드는 데 핵심입니다.

![](/docs/_remote/3a9f1ec299edbbb69e0531e0.webp)

필요할 때 사용할 수 있는 멋진 무료 팔레트 생성기를 소개합니다!

* **Coolors:** <https://coolors.co/>

또는 멋진 영화에서 제공하는 팔레트 영감을 참고할 수도 있습니다:

* **Movies in Color:** <https://moviesincolor.com/>

## 메탈릭 맵

메탈릭 맵에서 회색조 맵은 오브젝트의 금속성 정도를 나타냅니다. 흰색은 완전한 금속성, 검은색은 비금속성을 의미합니다. 다음 예시에서는 빛이 모델에 어떻게 영향을 주고 환경과 어떻게 상호작용하는지 볼 수 있습니다.

![](/docs/_remote/d7d680ed47a91de7cef0901a.webp)

*이미지에서 텍스처의 어두운 부분이 모델에 어떤 영향을 주는지 볼 수 있습니다. 어두운 줄무늬는 불투명하고, 흰색 줄무늬는 메탈릭 셰이더 속성에 따라 동작하며, 회색은 그 둘의 중간입니다.*

## 러프니스 맵

러프니스 맵에서는 어두운 영역이 더 매끄러운 표면에 해당하고, 밝은 영역은 더 거친 표면에 해당합니다. 이 회색조 표현은 렌더링 엔진이 표면의 서로 다른 지점에서 빛을 어떻게 산란하거나 반사할지 결정하는 데 사용됩니다.

![](/docs/_remote/ab1077956ce52917b01f28a4.webp) ![](/docs/_remote/cf4b607b56f34ffc9787473a.webp)

## 투명 맵

### 알파 클립

Blender의 Alpha Clip은 텍스처와 함께 사용할 때, 텍스처의 알파 채널을 사용해 재질의 어느 부분을 보이게 할지 결정합니다. 지정된 임계값보다 높은 알파 값을 가진 픽셀은 표시되고, 그보다 낮은 픽셀은 버려져 텍스처의 투명도 정보에 기반한 컷아웃 효과가 만들어집니다.

다음 예시에서는 알파 클립이 적용된 재질을 사용했으며, 2개의 텍스처, 즉 디퓨즈 색상과 흑백 알파 텍스처를 사용해 색상을 알파 채널에 연결했습니다.

![](/docs/_remote/a6eb0c3607b3ac754dd99f4a.webp) ![](/docs/_remote/7efa7539bc36fae5c645b2f7.webp)

![](/docs/_remote/85b8e0c6b3f5f217698536fc.webp)

*그 결과 검은색으로 칠해진 영역은 버려지고 흰색 영역은 렌더링되는 것을 볼 수 있습니다*

### 알파 블렌드

*알파 블렌드* 영역별로 중간 값을 선택할 수 있게 해줍니다.

Blender의 Alpha Blend는 텍스처와 함께 사용할 때, 알파 채널을 기준으로 텍스처의 투명 영역과 불투명 영역을 블렌딩합니다. 이를 통해 텍스처의 보이는 부분과 투과되는 부분이 부드럽게 결합되어, 배경이나 장면의 다른 오브젝트와 자연스럽게 상호작용할 수 있습니다.

![](/docs/_remote/46cdac0bd41b0f225bd498e4.webp)

*Alpha Clip은 렌더 값이 0 또는 1(특정 임계값에서)인 반면, Alpha Blend는 0과 1 사이의 값을 보간합니다. 위 예시에서 알파 블렌드 재질은 완전한 그라데이션 전환을 보여주고, 알파 클립은 클립 임계값으로 지정된 텍스처의 일부를 제외합니다.*


> [!WARNING]
> **🔥최적화 팁🔥**
> 
> * 투명 텍스처(RGB+A)는 투명도에 회색조 값을 사용하는 것보다 성능 면에서 항상 더 비용이 큽니다.
> * 투명도는 장면을 렌더링할 때 항상 비용이 큰 작업입니다. 투명도는 가능한 한 최소로 유지하고, Alpha Blend는 꼭 필요할 때만 사용하세요. 그렇지 않다면 Alpha Blend보다 Alpha Clip이 더 선호됩니다.


## 이미시브 맵

이미시브 맵은 3D 장면에서 표면의 자체 발광 또는 방출되는 빛을 제어하는 데 사용되는 텍스처 맵 유형입니다. 이는 특정 3D 모델의 어느 부분이 외부 광원과 무관하게 얼마나 많은 빛(및 빛의 색상)을 방출하는지 결정하는 셰이더의 구성 요소입니다. 이미시브 맵은 보통 스스로 빛을 내는 것처럼 보이는 재질이나 오브젝트를 시뮬레이션할 때 사용됩니다.

![](/docs/_remote/e4c3fc1cd832dcc33edb3f1c.webp)

*이 예시에서는 글로우 후처리를 사용하는 환경에서, 이미시브 강도와 결합된 이미시브 맵이 월드에서 어떻게 동작하는지 대략적으로 테스트하기 위해 사용된 것을 볼 수 있습니다.*

## 노멀 맵

노멀 맵은 3D 그래픽에서 미세한 표면 디테일을 시뮬레이션하고, 모델의 실제 기하 구조를 변경하지 않으면서 복잡한 형상이 있는 것처럼 보이게 만드는 데 사용되는 텍스처 유형입니다. 보통 요철, 홈, 기타 표면 불규칙성을 추가해 로우 폴리 모델의 사실감을 높이는 데 사용됩니다. 또한 복잡한 기하 구조 대신 노멀 맵 레이어에서 많은 디테일을 제공할 수 있으므로 오브젝트 자체를 더 가볍게 유지할 수 있습니다.

![](/docs/_remote/76188ce5daf39c2ac1f11691.webp) ![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

다음의 *셰이더 에디터*를 사용해 재질에 노멀 맵을 추가하려면, ***노멀 맵*** 노드를 텍스처와 *Principled BSDF* 셰이더 사이에 연결해야 합니다.

![](/docs/_remote/813d6e575903a5c3a19e8e2e.webp)


> [!WARNING]
> **⚠️중요⚠️:** 장면 렌더링 시 문제가 생길 수 있으므로, 하나의 텍스처를 알베도와 노멀에 동시에 사용하지 마세요.


## 텍스처 최적화

텍스처를 최적화하는 과정은 익스플로러에서 장면을 렌더링할 때 많은 이점을 제공합니다. 또한 장면의 스타일을 일관되게 유지하고 디자인 과정에서 더 유연하게 만드는 좋은 방법이기도 합니다. 이러한 이점 중 일부는 다음과 같습니다:

* 크기와 압축이 최적화된 텍스처를 사용하면 장면이 훨씬 더 부드럽고 빠르게 실행되어, 다운로드가 더 빨라지고 렌더링도 쉬워집니다(특히 인터넷 연결이 느린 플레이어에게 더 유리합니다).
* 경험을 렌더링하는 데 필요한 메모리와 처리 성능을 줄여 플레이어에게 더 나은 사용자 경험을 제공합니다.
* 커뮤니티 콘텐츠 서버의 저장 공간을 절약합니다.
* 다음의 **텍스처 아틀라스** 및/또는 **트림 시트** 를 사용하면 장면 제작을 반복할 때 더 큰 유연성을 확보하고, 오브젝트 간 스타일 일관성도 유지할 수 있습니다. 이 기법을 사용하면 각 모델마다 개별적으로 작업하는 대신 텍스처를 쉽게 교체하고 색상이나 패턴을 조정할 수 있습니다.
* 모델 간에 텍스처를 공유하면 장면당 텍스처 수를 줄일 수 있어, 게임 내 드로우 콜 수를 크게 감소시킵니다. glb(임베디드 텍스처 포함)를 사용 중이라면, 아래 가이드라인에서 텍스처를 추출하고 모델이 동일한 텍스처를 사용하도록 지정하는 데 도움이 되는 추출 도구를 찾을 수 있습니다.

## glTF 모델 간 공유 텍스처

장면 최적화를 위한 현명하고 일반적인 방법은 장면 전체의 모델 간에 텍스처와 재질을 공유하는 것입니다. 이렇게 하면 드로우 콜이 크게 줄어들고 Decentraland 장면이 훨씬 더 부드럽게 실행됩니다.

다음 도구는 [**glTF 파이프라인**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)**,** 를 기반으로 하며, 3D 모델을 더 가볍게 만들고 장면의 플레이어가 더 빨리 다운로드할 수 있도록 하는 몇 가지 최적화를 제공합니다.

**Mac:** [MAC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/texture_extractor.sh)

**PC:** [PC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/glb_texture_extract.bat)

.gltf 형식을 .glb로 변환합니다. .glb는 바이너리 형식이므로 훨씬 더 적은 용량을 차지합니다. 또한 텍스처 파일을 3D 모델 바깥에 배치하므로, 같은 텍스처를 여러 모델에서 사용할 수 있습니다.

> 📔 참고: .glb 형식은 기본적으로 파일 안에 텍스처가 임베드되어 있습니다. 엔진은 임베드된 두 텍스처를 같은 것으로 인식하지 못하므로, 동일한 해시를 공유하는 외부 파일이어야 합니다.

### GLB 텍스처 추출기 사용 방법

이 예시 장면에는 Blender로 만든 간단한 SF 장면이 있습니다.

![](/docs/_remote/09c63fb13acc8a4914118992.webp)

이 장면에는 정적 모델을 위한 기본 환경이 포함되어 있지만, 나중에 코드로 이동시켜 플레이어와 상호작용하게 하려는 드로이드와 우주선이라는 두 개의 다른 에셋도 있습니다. 이 경우 4개의 텍스처를 사용했습니다(바닥용 1개, 대부분의 에셋용 아틀라스 색상 맵 1개, 이미시브 1개, 패널용 UI SF 텍스처 1개).

![](/docs/_remote/1167ef79495fce28d3f04994.webp) ![](/docs/_remote/a8815581bf426ea4c90234ea.webp)

이 에셋들을 모두 models 폴더로 내보내면 환경 정적 장면, 우주선, 드로이드의 3개 모델이 생깁니다.

![](/docs/_remote/ba80d47093d413ede575a657.webp)

하지만 문제가 있습니다. .glb 파일에는 텍스처가 임베드되어 있으므로, 텍스처를 재사용하는 서로 다른 에셋을 내보내면 폴더에 새로운 에셋이 추가될 때마다 텍스처가 중복됩니다. 중복된 텍스처를 피하려면 이 유용한 도구를 사용할 수 있습니다.


> [!WARNING]
> ⚠️ 중요 참고: 도구를 사용하기 전에 만일의 사태에 대비해 모델을 백업하세요!!!


**Mac에서**

모든 에셋을 models 폴더로 내보냈다면 스크립트 파일을 그 폴더로 드래그할 수 있습니다.

![](/docs/_remote/e18bbaa902da38ea04efdabb.webp)

1 - 폴더를 터미널로 드래그하여 터미널에서 폴더를 엽니다.

![](/docs/_remote/b110a995c9095984ab0e64a6.webp)

2 - texture-extractor.sh를 터미널로 드래그합니다.

![](/docs/_remote/21793b724c96391277db0f59.webp)

3 - 명령을 실행합니다. 모든 에셋을 처리하는 데 몇 초가 걸릴 수 있습니다. 완료되면 다음과 같은 결과가 표시됩니다:

![](/docs/_remote/98233ac951185a1a694e7d05.webp)

4 - models 폴더로 이동하면 "out"이라는 새 폴더가 생겨 있고, 그 안에서 추출된 텍스처가 있는 새 에셋을 볼 수 있습니다.

![](/docs/_remote/fb16a7c2d30d3624207c138e.webp)

5 - 모든 에셋을 새 파일로 교체하세요! 또한 작업이 끝나면 "texture-extractor.sh"와 비어 있는 "out" 폴더를 삭제하세요.

![](/docs/_remote/98e2e796967f27259ce9c137.webp)

**Windows에서**

모든 에셋을 models 폴더로 내보냈다면 스크립트 파일을 그 폴더로 드래그할 수 있습니다.

![](/docs/_remote/7a5b21bf0d0ceb24563c52c3.webp)

1 - glb\_texture\_extract.bat을 더블 클릭하여 파일을 추출합니다. Windows에서 인식되지 않은 앱이라고 경고하면, 다음으로 이동하세요 **추가 정보** 을 사용하고, 그다음 **그래도 실행**.

![](/docs/_remote/c0f9074745dc366687e508e5.webp) ![](/docs/_remote/e286200a0e0ba4a6327ffc69.webp)

2 - 스크립트가 out이라는 폴더를 생성하며, 그 안에서 추출된 텍스처가 있는 모든 새 .glb 파일을 찾을 수 있습니다.

![](/docs/_remote/c4d9063db0fa909ac05097d1.webp)

5 - 모든 에셋을 새 파일로 교체하세요. 완료되면 "texture-extractor.bat" 스크립트와 비어 있는 "out" 폴더를 삭제하세요.

모든 단계를 따르면 이제 장면이 훨씬 더 빨라지고 에셋이 동일한 텍스처를 공유하게 됩니다! 여러 에셋과 큰 장면을 다룰 때는 개선 효과가 매우 뚜렷합니다! 이 도구를 사용하면 상당한 양의 메가바이트 정보를 절약할 수 있습니다!


> [!WARNING]
> ⚠️ 이 단계를 완료한 후에는 모든 텍스처가 가이드라인을 따르는지 확인하는 것이 매우 중요합니다. 텍스처가 가이드라인을 따르지 않거나 너무 무거우면, 배포하기 전에 더 가볍게 최적화하세요. 성능이 좋은 장면에 권장되는 텍스처 크기는 1024x1024입니다. 또한 해상도도 확인하세요. 72 DPI를 초과하는 이미지는 성능에 영향을 주며, 이미지가 더 좋아 보이게 만들지도 않습니다.


## 텍스처 아틀라스/ 트림 시트

텍스처 아틀라스는 여러 개의 더 작은 이미지를 하나로 묶어 담은 단일 이미지 파일입니다. 각 메시마다 하나의 텍스처를 사용하는 대신, 여러 메시가 더 큰 텍스처를 공유합니다.

에셋을 만들기 전에 텍스처 아틀라스를 만들 수 있으며, 이는 에셋이 텍스처 아틀라스에 맞춰 UV 언래핑된다는 뜻입니다. 텍스처를 만들 때는 초기에 계획이 필요합니다.

또는 에셋이 완성된 후 페인팅 소프트웨어에서 텍스처를 병합해 텍스처 아틀라스를 만들 수도 있습니다. 하지만 이 경우 UV 아일랜드도 텍스처에 맞춰 다시 배치해야 합니다.

아래 이미지는 하나의 텍스처 세트를 사용하는 여러 3D 오브젝트를 보여줍니다:

![](/docs/_remote/ed4a55e913cda2f92bcb60db.webp) ![](/docs/_remote/d4e59128a0be5c8f7414ad23.webp)

아틀라스를 만드는 또 다른 방법은 트림 시트를 사용하는 것입니다. 트림 시트는 3D 모델링과 게임 개발에서 널리 사용되는 일반적인 기법으로, 대형 스튜디오와 게임 업계에서 시각적 일관성과 효율적인 아트 파이프라인을 보장하기 위해 사용합니다. 트림 시트는 여러 개의 작은 디테일이나 요소를 담고 있어 모델의 서로 다른 부분에 적용할 수 있는 하나의 이미지 또는 텍스처를 사용하는 것입니다. 이 기법은 큰 장면을 만들 때 매우 유용하며, 아트 파이프라인의 첫 단계에서 고려해야 합니다.

![](/docs/_remote/6e219acc9852ed30d1a9b768.webp)

*디퓨즈, 노멀, 이미시브를 사용하는 트림 시트의 콜라주 예시입니다.*

훌륭한 [튜토리얼이 있습니다](https://www.artstation.com/blogs/jennifermcgarry/yd4Q/jenns-guide-to-trim-sheets) 로 [Jennifer McGarry](https://www.artstation.com/jennifermcgarry/blog) 가 Blender를 사용한 트림 시트 활용법을 설명합니다!

## 텍스처 이름 지정

**텍스처를 올바르게 이름 짓는 것은 매우 중요합니다.** 텍스처를 올바르게 이름 짓는다면 다음과 같습니다:

* 아트 파이프라인을 더 효율적이고 유연하며 체계적으로 만들고, 필요할 때 쉽게 대상 지정 및 수정할 수 있게 합니다.
* 텍스처 추출기나 다른 도구를 사용할 때 같은 이름의 텍스처가 겹치는 것을 방지합니다.
* Asset Bundle을 사용할 때 텍스처가 겹치는 문제를 방지합니다.
* 장면을 분석할 때 문제 지점을 더 효율적으로 찾을 수 있는 방법입니다.

**텍스처를 올바르게 이름 짓는 방법:**

* **에셋 이름** 텍스처가 무엇인지 명확하게 나타내야 합니다.
* 텍스처 이름은 다음 접두사로 시작해야 합니다 `T_`.
* 텍스처 이름은 텍스처 유형을 정의하는 다음 접미사로 끝나야 합니다:
  * `_D` - **디퓨즈/ 컬러 맵**
  * `_A` - **알파 텍스처**
  * `_MT` - **금속성**
  * `_R` - **거칠기**
  * `_N` **- 노멀 맵**
  * `_E` - **이미시브**

예: 벽용 벽돌 텍스처의 디퓨즈 맵이라면, 이름 `T_BrickWall_D` 가 적절할 수 있습니다. 같은 에셋의 노멀 맵이라면 이름은 `T_BrickWall_N`

**예시:**

* 🟢 **다음을 사용하는 것을 권장합니다** 텍스처 이름을 다음과 같이 시작: `T_Parquet_D`, `T_Floor_R`, `T_Pipes_MT`,
* 🔴 **피해야 할 것** 텍스처 이름을 다음과 같이 시작: `Image_`, `sprite_`,`Untitled`

## 선택 사항 도구

에셋을 만들 때 작업을 더 빠르고 효율적으로 만들어 주는 수많은 애드온과 외부 도구가 있습니다. 무료인 것도 있고 구매해야 하는 것도 있으며, 몇 가지를 들자면:

### UVTools

* **UV Packer (무료)**:<https://www.uv-packer.com/blender/>
* **Zen UV:** <https://blendermarket.com/products/zen-uv>
* **Uvpackmaster 3:** <https://blendermarket.com/products/uvpackmaster>

### 이미지 압축기

텍스처를 더 가볍게 만들기 위해 사용할 수 있는 온라인 이미지 압축기가 여러 개 있습니다. 몇 가지를 들자면:

* **CompressPNG:** <https://compresspng.com/>
* **TinyPNG:** <https://tinypng.com/>
* **FreeConvert:** <https://www.freeconvert.com/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/3d/textures.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
