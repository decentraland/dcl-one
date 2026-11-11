> 내 씬에 얼마나 많은 항목을 넣을 수 있나요?

# 씬 제한 사항

메타버스에서 성능을 개선하기 위해, 모든 씬이 따라야 하는 일련의 제한을 마련했습니다. 이 제한은 각 파셀 기준입니다. 따라서 씬이 클수록 이 제한값도 더 높게 설정됩니다.

다음을 사용할 때 [Creator Hub](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/), 씬의 3D 모델이 사용하는 리소스에 대한 통계와 씬의 제한을 확인할 수 있습니다.

![](/docs/_remote/ab87e37107598ee524831639.webp)

이 메뉴를 확장하여 세부 정보를 볼 수 있습니다.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 팁**: 모든 파셀 수별 세부 숫자에 대한 참조 표는 다음을 참조하세요:
> 
> [참조 표](https://docs.google.com/spreadsheets/d/1BTm0C20PqdQDAN7vOQ6FpnkVncPecJt-EwTSNHzrsmg/edit#gid=0)


## 씬 제한 규칙

다음은 씬이 동시에 렌더링할 수 있는 요소의 최대 개수입니다:

> *n* 는 씬이 차지하는 파셀 수를 나타냅니다.

* **삼각형:** `n x 10000` 씬 내 모든 모델의 총 삼각형 수.
* **엔티티:** `n x 200` 씬 내 엔티티 수.
* **메시:** `n x 300` 씬 내 메시 수입니다.
* **재질:** `log2(n+1) x 20` 씬 내 재질 수입니다. 모델의 일부로 가져온 재질도 포함됩니다.
* **텍스처:** `log2(n+1) x 10` 씬 내 텍스처 수입니다. 모델의 일부로 가져온 텍스처도 포함됩니다.
* **높이:** `330미터` 차지하는 파셀 수와 관계없이 씬의 최대 높이입니다.

  > 경고: 200미터보다 높은 곳에 배치된 엔티티는 멀티플레이어 동기화 문제를 겪을 수 있습니다. 플레이어가 상호작용하는 콘텐츠나 플레이어 간에 동기화되는 콘텐츠는 이 높이 아래에 두세요.

  > 중요: 현재 씬에서 렌더링되고 있는 엔티티만 이 제한에 대해 계산됩니다. 씬이 여러 3D 모델 간에 전환되는 경우, 전체 합계가 아니라 특정 시점에 렌더링된 모델이 중요합니다. 플레이어 아바타와 플레이어가 씬 외부에서 가져온 아이템도 이 제한 계산에 포함되지 않습니다.
* **총 파일 크기:** Genesis City에서 -`파셀당 15MB - 최대 300MB`. 월드의 경우 다음을 참조하세요 [월드 크기](#world-size). 콘텐츠 서버에 업로드된 파일의 총 크기입니다. 3D 모델과 오디오가 포함됩니다. node.js 패키지와 같이 업로드되지 않는 파일은 포함되지 않습니다. 배포를 확인하기 전에 게시될 파일의 전체 목록과 크기를 확인할 수 있습니다.
* **파일 수:** `파셀당 200개 파일` 업로드된 파일의 총 개수입니다. 3D 모델과 오디오가 포함됩니다. node.js 패키지와 같이 업로드되지 않는 파일은 포함되지 않습니다.
* **최대 파일 크기** `파일당 50MB` 씬의 어떤 종류의 개별 파일도 50MB를 초과할 수 없습니다. 작은 씬은 총 파일 크기 제한을 초과할 수 없으므로 더 엄격하게 제한됩니다(예: 단일 파셀 씬의 총 크기는 15MB로 제한됨).


> [!WARNING]
> **📔 참고**: 파일 크기 및 파일 수 제한은 배포 시 강제 적용되며, 이를 초과하면 배포가 차단됩니다. 다른 제한(삼각형, 엔티티, 바디, 재질, 텍스처, 높이)은 소프트 제한으로, Creator Hub에서는 경고로 표시되지만 이를 초과해도 일반적으로 치명적인 실패보다는 성능 저하로 이어집니다. 강력한 권장 사항으로 받아들이세요.



> [!NOTE]
> **💡 팁**: 씬 프로젝트 폴더의 모든 파일이 파일 크기 제한에 포함되는 것은 아니며, 서버에 업로드되는 파일만 포함됩니다.  *node\_modules* 폴더의 모든 내용은 매우 크지만 업로드되지 않는 종속 항목이므로 포함되지 않습니다. 같은 이유로 `/src` 폴더의 파일도 소스 코드는 업로드되지 않으므로 포함되지 않습니다.
> 
> 업로드에서 제외하고 싶은 다른 파일이나 폴더는 프로젝트의 `.dclignore` 파일에 나열할 수 있습니다. 다음에 유의하세요 `.dclignore` 는 로컬 미리보기에도 적용되므로, 씬이 실제로 불러오는 항목은 아무것도 나열하지 마세요. 다음을 참조하세요 [dclignore 파일](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-files/#the-dclignore-file).


## 최적화하기

다음을 참조하세요 [성능 최적화](/docs/creator/content-creator-ko/sdk7/optimizing/performance-optimization/) 에서 씬을 이러한 제한 아래로 유지하고 플레이어에게 더 부드럽게 실행되도록 하는 팁을 확인하세요.


> [!NOTE]
> **📱 모바일**: 이 페이지의 하드 제한은 모든 클라이언트에 적용되지만, 모바일 기기는 데스크톱보다 여유가 적습니다. 타깃이  [모바일 앱](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/)일 때는 이 제한보다 충분히 낮게 유지하고, 삼각형 수를 줄이고, 재질 수를 줄이고, 텍스처를 더 작게 사용하는 것이 좋습니다.


## 씬 경계

미리보기를 실행할 때 파셀 경계 밖에 있는 콘텐츠는 렌더링 시 빨간색으로 강조 표시됩니다. 콘텐츠의 일부라도 이 경계 밖에 있으면, 플레이어가 씬을 방문할 때 해당 부분은 렌더링되지 않습니다.

큰 물체의 끝부분이 경계를 벗어나면, 그 끝부분은 물체에서 잘려 나갑니다.

단일 파셀 씬의 크기는 16미터 x 16미터입니다. 씬이 여러 파셀로 구성된 경우, 크기는 파셀의 배치에 따라 달라집니다.

엔티티를 지하에 배치하여 숨기거나 일부만 드러나게 할 수 있습니다. 씬에는 기본 지면 높이보다 아래로 내려가는 터널을 만들 수 없으며, 플레이어는 다음보다 아래로 이동할 수 없습니다 `y = 0` 높이.

## 셰이더 제한

decentraland에서 사용되는 3D 모델은 지원되는 셰이더와 재질을 사용해야 합니다. 다음을 참조하세요 [3D 모델 재질](/docs/creator/content-creator-ko/3d/materials/) 에서 지원되는 셰이더 목록을 확인하세요.

## 메쉬 압축


> [!WARNING]
> **📱 Draco 압축 메쉬는** [**모바일 클라이언트에서**](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/overview/) **자산 최적화가 필요합니다.** 다음을 사용하는 glTF/GLB 모델은 [Draco](https://google.github.io/draco/) 메쉬 압축 모델은 자산 최적화 파이프라인을 거친 후에만 Decentraland 모바일 앱에 로드됩니다. Draco 압축 모델이 모바일에서 로드되지 않는 경우, 최적화되었는지 확인하세요 — 또는 모델을 **사용하지 않고**  Draco 압축 없이 내보내어 이 단계를 완전히 건너뛰세요. 다른 형태의 glTF 최적화(양자화, 텍스처 압축, 메쉬 단순화)는 영향을 받지 않습니다.


## 조명

각 플레이어는 Explorer UI에서 자신의 스카이박스 설정을 변경할 수 있지만, 씬의 조명 조건을 기본 설정에서 모든 플레이어에 대해 변경할 수는 없습니다.

## 텍스처 크기 제한

텍스처 크기는 다음 숫자와 일치하는 너비와 높이 값(픽셀 단위)을 사용해야 합니다:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> 이 수열은 2의 거듭제곱으로 이루어져 있습니다: `f(x) = 2 ^ x` . 1024는 텍스처 크기에 허용되는 최대값이며, 이보다 큰 텍스처는 자산 번들 변환 과정에서 축소됩니다. 이는 다른 렌더링 엔진에서도 흔히 요구되는 사항으로, 그래픽 프로세서의 내부 최적화 때문입니다.

너비와 높이가 같은 숫자일 필요는 없지만, 둘 다 이 수열에 속해야 합니다.

**텍스처의 권장 크기는 1024x1024입니다**, 우리는 이것이 국내 네트워크를 통해 전송하기에 최적의 크기이며 적절한 로딩/품질 경험을 제공한다고 확인했습니다.

다른 유효한 크기의 예:

```
32x32
64x32
512x256
512x512
1024x1024
```


> [!WARNING]
> **📔 참고**: 텍스처는 임의 크기도 때로는 작동하지만, 종종 버그가 있는 상태로 렌더링되며 더 불안정합니다. 모든 텍스처가 이러한 크기와 일치하도록 강력히 권장합니다.


## 월드 크기

Decentraland [월드](/docs/creator/content-creator-ko/sdk7/publishing/publishing-options/#decentraland-worlds) 는 단일 씬으로 로드되므로 서로 다른 제한이 있습니다.

* Decentraland NAMEs에 게시된 월드는 같은 지갑이 소유한 모든 월드가 공유하는 저장소 예산에서 할당됩니다. 보유한 각 NAME은 `100MB`를 제공하며, 각 LAND 파셀은 추가로 `100MB`, 그리고 보유한 MANA 2,000개마다 추가로 `100MB`. 이 예산은 원하는 대로 월드들 사이에 배분할 수 있습니다.
* ENS 도메인에 게시된 월드에는 `36MB` 의 확장할 수 없는 제한이 있습니다.

예산의 사용량과 남은 양은 다음에서 확인할 수 있습니다 **관리** 섹션에서, 또는 **월드** 탭에서 [빌더](https://decentraland.org/builder/worlds). 예산을 초과한 경우(예: 자산을 판매하거나 이전한 후) 월드에 접근할 수 없게 되기 전에 공간을 확보할 수 있는 48시간이 주어집니다.

다음을 참조하세요 [월드 크기 제한](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/kinds-of-project/#size-limits) 에서 자세한 내용을 확인하세요.

콘텐츠 서버에 업로드된 파일의 총 크기입니다. 3D 모델과 오디오가 포함됩니다. node.js 패키지와 같이 업로드되지 않는 파일은 포함되지 않습니다. 배포를 확인하기 전에 게시될 파일의 전체 목록과 크기를 확인할 수 있습니다.

월드의 다른 모든 제한은 삼각형, 재질 등을 포함하여 파셀 기준입니다. 월드에 파셀을 더 추가하는 것은 무료이므로, 씬에 최대 300x300 파셀(총 90,000개)을 추가할 수 있으며, 해당 파셀 수에 해당하는 제한을 적용받습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/optimizing/scene-limitations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
