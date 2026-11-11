> 외부 3D 모델에서 지원되는 에셋과 컴포넌트를 알아보고, Decentraland로 가져오기 전에 이를 어떻게 설정하는지 알아봅니다.

# 3D 모델

![](/docs/_remote/62bf81e2e6497cde955d408a.webp)

Decentraland에 3D 모델을 가져올 때는 glTF 형식인지 확인하는 것이 중요합니다. 이러한 모델에는 다양한 지원 기능이 포함될 수 있습니다. 이 섹션에서는 이를 Decentraland와 호환되게 만드는 방법과 모범 사례를 따르는 방법을 다룹니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 유용한 여러 기능이 포함되어 있습니다.


모든 모델, 셰이더, 텍스처는 다음을 준수해야 합니다. [**씬 제한 사항**](/docs/creator/sdk7/optimizing/scene-limitations/).

## **지원되는 3D 모델 형식**

Decentraland의 모든 3D 모델은 glTF 형식이어야 합니다. [**glTF**](https://www.khronos.org/gltf) (GL Transmission Format)은 Khronos의 오픈 프로젝트로, 3D 자산을 위한 공통적이고 확장 가능한 형식을 제공하며, 효율적이고 최신 웹 기술과 높은 상호운용성을 갖습니다.

glTF 모델에는 다음 중 하나의 *.gltf* 또는 *.glb* 확장자가 있습니다. glTF 파일은 사람이 읽을 수 있어 텍스트 편집기에서 열어 JSON 파일처럼 읽을 수 있습니다. 예를 들어 애니메이션이 제대로 연결되어 있는지 확인하고 이름을 점검하는 데 유용합니다. glb 파일은 이진 형식이므로 읽을 수는 없지만 크기가 훨씬 작아 장면 성능에 좋습니다.

작업 중일 때는 *.gltf* 를 사용하는 것을 권장하지만, 업로드할 때는 *.glb* 로 전환하세요.

3D 모델의 다음 요소들은 *glTF* 파일 안에 포함되거나 외부에서 참조될 수 있습니다:

* 텍스처는 내장되거나 외부 이미지 파일에서 참조될 수 있습니다.
* 모델의 지오메트리, 애니메이션 및 기타 버퍼 관련 요소에 대한 이진 데이터는 외부 *.bin* 파일을 추가할 수 있습니다.


> [!WARNING]
> **📔 참고**: 애니메이션 *반드시* 안에 포함될 수 있습니다. *glTF* 파일 안에 포함되어 Decentraland에서 사용할 수 있습니다. 또한 각 모델은 UV 맵을 1개만 가질 수 있습니다.


## **glTF를 사용하는 이유**

우리가 glTF를 사용하는 이유는 다음과 같은 강력한 기능을 제공하기 때문입니다:

* 계층적 객체
* 골격 구조 및 애니메이션
* 머티리얼 및 셰이더를 위한 강력한 파이프라인
* 광원과 카메라 같은 장면 정보(향후 개발에서 사용할 수 있음).

다음과 비교하면 *COLLADA*에서 지원하는 기능은 매우 비슷합니다. 그러나 glTF는 편집기 형식이 아니라 “전송 형식” 제공에 중점을 두기 때문에 웹 기술과의 상호운용성이 더 높습니다.

## **glTF 내보내기 및 변환**

### **Blender에서 glTF로 내보내기**

Blender에는 기본 gltf 내보내기가 있습니다.

.glb 파일에 포함할 모델을 선택한 다음 File - Export - glTF 2.0으로 이동하세요.

![](/docs/_remote/1f95091f3ce5ddb4219f6eea.webp) ![](/docs/_remote/3edef4656ec7f2427b2d9dbe.webp)

*측면 패널에서 장면의 선택한 객체만 내보내고 싶다면 “Selected Objects”를 체크하세요. 또한 Mesh 드롭다운에서 Apply Modifiers(적용된 경우)를 선택한 뒤 Export를 누르세요.*

### **3D Studio Max에서 glTF로 내보내기**

3D Studio Max는 기본적으로 glTF로 내보내기를 지원하지 않지만, 플러그인을 설치하면 사용할 수 있습니다.

1. 다음에서 플러그인을 다운로드하세요. [이 링크](https://github.com/BabylonJS/Exporters/tree/master/3ds%20Max).
2. 다음 안내를 따라 플러그인을 설치하세요. [이 지침](http://doc.babylonjs.com/resources/3dsmax#how-to-install-the-3ds-max-plugin).
3. 다음 안내를 따라 플러그인을 사용해 glTF 파일을 내보내세요. [이 지침](http://doc.babylonjs.com/resources/3dsmax_to_gltf).

### **Maya에서 glTF로 내보내기**

Maya는 기본적으로 glTF로 내보내기를 지원하지 않지만, 플러그인을 설치하면 사용할 수 있습니다.

1. 다음 안내를 따라 플러그인을 설치하세요. [이 지침](http://doc.babylonjs.com/resources/maya).
2. 다음 안내를 따라 플러그인을 사용해 glTF 파일을 내보내세요. [이 지침](http://doc.babylonjs.com/resources/maya_to_gltf#pbr-materials).


> [!WARNING]
> **📔 참고**: 대안으로 [이 다른 플러그인](https://github.com/WonderMediaProductions/Maya2glTF) 도 시도해볼 수 있습니다.


### **Unity에서 glTF로 내보내기**

Unity는 기본적으로 glTF로 내보내기를 지원하지 않지만, 플러그인을 설치하면 사용할 수 있습니다.

다음에서 플러그인을 다운로드하세요. [이 링크](https://github.com/sketchfab/Unity-glTF-Exporter).


> [!WARNING]
> **📔 참고**: 대안으로 [이 다른 플러그인](https://assetstore.unity.com/packages/tools/utilities/collada-exporter-for-unity2017-99793) 도 시도해볼 수 있습니다.


### **SketchUp에서 glTF로 내보내기**

SketchUp은 기본적으로 glTF로 내보내기를 지원하지 않지만, 플러그인을 설치하면 사용할 수 있습니다.

다음에서 플러그인을 다운로드하세요. [이 링크](https://extensions.sketchup.com/en/content/gltf-exporter).

### **FBX를 glTF로 변환**

*.fbx* 는 3D 모델의 매우 널리 쓰이는 표준입니다. 우리 엔진은 이를 지원하지 않지만, *.fbx* 모델을 *.gltf* 형식으로 쉽게 내보낼 수 있습니다.

다음 도구 사용을 권장합니다:

* [**Blender**](https://www.blender.org/download/): .fbx를 glTF로 변환하는 쉬운 방법은 Blender로 .fbx를 씬에 가져온 뒤 거기서 내보내는 것입니다.
* [**Facebook의 CLI 도구**](https://github.com/facebookincubator/FBX2glTF): 가장 강력한 대안이지만 명령줄 사용이 필요합니다.
* [**Blackthread**](https://blackthread.io/gltf-converter): 가장 완전한 웹 기반 도구입니다. CLI보다 덜 강력하지만 훨씬 더 사용하기 쉽습니다.
* [**Modelconverter**](https://modelconverter.com/convert.html): 사용하기 쉬운 또 다른 웹 기반 도구입니다.

## **glTF 모델 미리보기**

장면에 가져오기 전에 glTF 모델의 내용을 빠르고 쉽게 미리 보는 방법은 [**Babylon.js Sandbox**](https://sandbox.babylonjs.com/)를 사용하는 것입니다. glTF 파일(해당되는 경우 그 *.bin* 파일도)을 캔버스에 드래그 앤 드롭하여 모델을 볼 수 있습니다.

샌드박스에서는 모델에 내장된 애니메이션도 미리 볼 수 있으며, 드롭다운 메뉴에서 선택하여 표시할 항목을 고를 수 있습니다.

![](/docs/_remote/34159024016bd56402873a74.webp)

## **glTF 분석**

glTF를 분석하는 좋은 도구 중 하나는 [**glTF Sample Viewer**](https://github.khronos.org/glTF-Sample-Viewer-Release/)를 사용하는 것입니다. 이 도구를 사용하면 채널, 텍스처, 다양한 조명 환경에 대한 반응, 애니메이션 미리보기 등 훨씬 더 많은 것을 확인할 수 있습니다! 단일 모델에 이상적입니다.

![](/docs/_remote/173cb3c41baf0f4c374aea20.webp)

또 다른 유용한 도구는 [**gltf.report/**](https://gltf.report/)입니다. 모델을 웹사이트에 단순히 드래그 앤 드롭하면 glTF 패키지의 내용을 볼 수 있고, 모델의 어떤 부분을 최적화할 수 있는지 평가할 수 있습니다.

이 도구는 여러 모델, 텍스처, 애니메이션이 포함된 큰 파일에 특히 유용할 수 있습니다. 예를 들어 텍스처를 크기별로 정리하면 어떤 텍스처가 가장 많은 리소스를 사용하는지 쉽게 파악할 수 있습니다.

![](/docs/_remote/40647d97e437f4808c189320.webp)

## **glTF 최적화**

다음 도구는 장면 속 플레이어가 더 가볍고 더 빠르게 다운로드할 수 있도록 3D 모델을 최적화하는 몇 가지 기능을 제공합니다.

[glTF 파이프라인](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)

그중에서도 *.gltf* 형식을 *.glb*로 변환합니다. 이는 이진 형식이므로 훨씬 적은 공간을 차지합니다. 또한 텍스처 파일을 3D 모델 외부에 두기 때문에 동일한 텍스처를 여러 모델에 사용할 수 있습니다.


> [!WARNING]
> **📔 참고**: *.glb* 형식은 기본적으로 항상 텍스처를 파일 안에 포함합니다. 엔진은 내장된 두 텍스처를 같은 것으로 인식할 수 없으므로, 같은 해시를 공유하는 외부 파일이어야 합니다.


## 또한 참조

다음 페이지들도 Decentraland의 3D 모델과 관련된 주제를 다룹니다:

* [메시](/docs/creator/content-creator-ko/3d/meshes/)
* [머티리얼](/docs/creator/content-creator-ko/3d/materials/)
* [텍스처](/docs/creator/content-creator-ko/3d/textures/)
* [콜라이더](/docs/creator/content-creator-ko/3d/colliders/)
* [애니메이션](/docs/creator/content-creator-ko/3d/animations/)
* [유용한 자료](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/): 3D 워크플로를 가속화하는 Blender 애드온, 에셋 라이브러리, AI 도구


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/3d/3d-models.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
