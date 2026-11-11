> Decentraland 씬의 모바일 제한 및 성능 목표, 그리고 Creator Hub에서 이를 미리 보고 측정하는 방법입니다.

# 성능

모바일 기기는 데스크톱이나 웹보다 리소스 제약이 더 큽니다. 모바일 씬 제한과 이를 미리 보는 방법을 이해하면 가장 넓은 사용자층에서 원활하게 실행되는 씬을 배포하는 데 도움이 됩니다.

## 모바일 씬 제한

아래 표는 모바일에서 실행할 때 적용되는 제한을 보여줍니다. 한도에 도달하면 **소프트 한도** 는 성능 패널에 경고를 표시하고, 한도에 도달하면 **하드 한도** 씬 로딩이 차단됩니다.

| 메트릭           | 소프트 한도    | 하드 한도     |
| ------------- | --------- | --------- |
| 삼각형           | 1,000,000 | 1,200,000 |
| 엔티티           | 4,800     | 6,000     |
| 메시(바디)        | 2,400     | 3,000     |
| 지오메트리         | 1,000     | 2,000     |
| 머티리얼          | 400       | 500       |
| 텍스처           | 400       | 500       |
| 콜라이더          | 1,200     | 1,500     |
| VFX 이미터       | 8         | 10        |
| 실시간 파티클       | 25,000    | 50,000    |
| 광원            | 8         | 16        |
| 그림자 조명        | 2         | 4         |
| 콘텐츠 크기        | 120 MB    | 150 MB    |
| 외부 콘텐츠        | 40 MB     | 50 MB     |
| 메모리(프로세스 RSS) | 1,638 MB  | 2,048 MB  |
| 드로우 콜         | 1,000     | 2,000     |
| 성능(높을수록 좋음)   | 90%       | 85%       |

## 조명 및 파티클

조명 항목은 **제작된** 씬 콘텐츠를 집계하며, 엔진이 최종적으로 실행하는 내용을 기준으로 하지 않습니다. 각 그래픽 프로필은 가장 가까운 광원에 대한 예산만 활성화하며(매우 낮음은 0, 높음은 최대 8), 그림자를 완전히 비활성화할 수도 있습니다. 따라서 20개의 조명을 제작한 씬은 한 번에 최대 8개만 빛을 내더라도 그 씬 비용을 그대로 부담합니다. 제한에 맞춰 제작하고 런타임 예산에 의존하지 마세요.

파티클은 런타임에서도 추가로 제한됩니다: 이미터당 5,000개, 씬당 50,000개입니다. 이 한도를 초과해 제작된 항목은 조용히 상한값으로 고정되므로, 이를 초과해도 씬 예산만 낭비됩니다.

## 모바일 성능 미리보기

씬의 모바일 성능을 바로 확인할 수 있습니다 **Creator Hub에서** 실제 기기에 배포하지 않고도.

1. 다음에서 씬을 엽니다 **Creator Hub에서**.
2. 미리보기 > 모바일용 QR 코드 표시를 선택하세요
3. 다음을 엽니다 **씬 제한 미리보기** 휴대폰의 오른쪽 상단 아이콘(빨간 통계가 표시된 모니터)에서.

![Creator Hub의 모바일 씬 제한 패널](/docs/_remote/86e4b4fb53b120e0cdcd5a75.webp)


> [!WARNING]
> **높음 그래픽에서 성능을 90% 이상으로 유지하세요**
> 
> 해당 **성능** 값은 활성 그래픽 프로필이 설정한 FPS 예산의 백분율입니다. 점수 **100%** 는 씬이 전체 FPS 목표에 도달했음을 의미합니다.
> 
> 항상 다음과 같은 중급 사양 기기에서 테스트하세요: **Samsung Galaxy A54** 그리고 **높음 그래픽 프로필에서 90% 이상의 성능 점수를 목표로 하세요**. 이는 대부분의 모바일 플레이어에게 원활한 경험을 보장합니다. 다음을 참조하세요: [하드웨어 요구 사항](/docs/creator/content-creator-ko/build-for-mobile/mobile-client/hardware-requirements/) 참조 기기용
> 
> 그래픽 프로필은 다음에서 변경할 수 있습니다: **인게임 메뉴 → 설정 → 그래픽 → 동적 그래픽을 끔으로 설정 → 프로필 전환**.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/build-for-mobile/develop/optimize-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
