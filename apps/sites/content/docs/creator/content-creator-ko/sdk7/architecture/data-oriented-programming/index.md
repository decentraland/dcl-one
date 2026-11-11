> 데이터 지향 프로그래밍은 성능을 최대한 활용하는 강력한 프로그래밍 접근 방식입니다.

# 데이터 지향 프로그래밍

데이터 지향 프로그래밍은 성능을 크게 향상시키는 강력한 프로그래밍 접근 방식입니다. 이는 *데이터를* 중심 요소로 다루는 데 초점을 맞추며, 그 밖의 모든 것은 해당 데이터를 접근하거나 수정하기 위해 이를 중심으로 구성됩니다. 이 접근 방식은 플레이어 간에 동기화해야 하는 데이터를 더 쉽게, 더 빠르게 접근할 수 있게 해주므로 멀티플레이어 친화적이기도 합니다.

데이터 지향 프로그래밍은 씬의 모든 것을 다양한 시스템 전반에서 복사되고 변형되어야 하는 데이터로 생각하도록 유도합니다. 이 접근 방식의 주요 이점은 메모리에서 데이터를 읽어오는 속도를 최적화하는 데 있으며, 이는 현대의 애플리케이션과 게임을 실행할 때 종종 가장 큰 병목이 됩니다.

이처럼 눈에 띄는 성능 향상 때문에, 지난 몇 년 동안 게임 제작 업계의 많은 부분이 이 접근 방식을 채택하는 방향으로 이동해 왔습니다.

Decentraland의 SDK는 씬을 JavaScript로 실행하며, 이 언어의 한 가지 단점은 메모리 할당을 제어할 수 없다는 점입니다. 하지만 Decentraland를 실행하는 엔진은 C#을 사용하며, 이 언어는 데이터 지향 원칙을 따르면 많은 이점을 얻습니다. SDK와 엔진은 서로 메시지를 끊임없이 주고받습니다. 이 통신을 최대한 효율적으로 만들려면 양쪽의 데이터 구조를 가능한 한 비슷하게 유지하여, 이 데이터를 계속해서 재구성해야 하는 일을 피하는 것이 합리적입니다.

## 어떻게 보이는지

데이터 지향 프로그래밍은 많은 개발자들이 현재 익숙한 객체 지향 프로그래밍과는 다릅니다. 객체 지향 프로그래밍에서는 코드가 현실의 개념인 객체를 모방하려는 추상화를 따라 구성됩니다. 이러한 각 객체는 데이터와 기능을 모두 가질 수 있습니다. 이 접근 방식을 사용하는 애플리케이션은 개념적으로는 계획하기 쉬운 경우가 많지만, 실행 효율은 더 떨어집니다.

데이터 지향 프로그래밍에서는 데이터가 객체를 중심으로 구조화되지 않고, 접근하기 쉽도록 최적화된 방식으로 구조화됩니다. 데이터가 나타내는 현실의 개념은 그 데이터를 변경하는 여러 흐름에서 아무 역할도 하지 않습니다.

Decentraland SDK가 기반으로 하는 엔티티 컴포넌트 시스템(ECS) 모델은 데이터 지향 프로그래밍 접근 방식과 매우 잘 맞습니다. 각 컴포넌트는 구조화된 데이터 모음의 일부입니다. 컴포넌트는 참조를 통해 엔티티에 속하지만, 데이터는 엔티티를 중심으로 구조화되지 않고, 유사한 컴포넌트들의 모음으로 구조화됩니다. 예를 들어, 모든 `Transform` 씬의 컴포넌트는 동일합니다. 이러한 트랜스폼 중 하나는 주 건물의 모델에 속할 수도 있고, 다른 하나는 테이블의 자식인 유리잔에 속할 수도 있습니다. 그다음 씬의 시스템은 이러한 `Transform` 컴포넌트를 아무런 구분 없이 하나씩 처리합니다. 모든 `Transform` 컴포넌트는 같은 필드를 가지며 같은 검사를 거칩니다.

![](/docs/_remote/29703f638d6018eac3f30fbf.webp)

열려 있거나 닫혀 있을 수 있는 문이 열두 개 있는 씬을 상상해 보세요. 이 모든 문의 상태를 불리언 값을 담는 단순한 "isOpen" 컴포넌트로 표현할 수 있습니다. "isOpen"이 참이면 문은 열려 있어야 하고, 거짓이면 문은 닫혀 있어야 합니다. 플레이어가 문을 클릭하면 상태가 토글되어야 하며, 다른 플레이어도 그 토글을 볼 수 있어야 합니다. 씬이 문 상태의 변경을 처리하고 이를 다른 플레이어와 동기화하는 동안에는, 씬은 실제로 "isOpen"이 무엇을 나타내는지에는 별로 신경 쓰지 않습니다. 컴포넌트 전체는 다른 플레이어와 동기화해야 하는 불리언들의 모음일 뿐입니다. 그런 다음 씬의 별도 시스템이 각 "isOpen"의 상태를 해당 문의 회전에 정기적으로 맞춰 주는 일을 처리할 수 있습니다.

데이터 지향 프로그래밍이 반드시 더 어려운 것은 아니지만, 배우고 채택해야 하는 다른 접근 방식입니다. 이 접근 방식에 익숙하지 않은 개발자라면 익숙해지는 데 시간이 좀 필요할 수 있으며, 더 잘 감을 잡을 수 있도록 예제를 탐색하고 직접 다뤄 보기를 권장합니다.

## 왜 효과적인가

데이터 지향 프로그래밍이 왜 그렇게 큰 차이를 만드는지 이해하려면 하드웨어를 살펴봐야 합니다.

프로세서가 메모리에서 데이터를 가져와야 할 때, 원하는 값 옆의 메모리에 우연히 기록된 데이터까지 포함해 데이터의 한 덩어리 전체를 캐시에 가져옵니다. 코드가 이미 캐시에 있는 데이터를 더 많이 활용할수록, 코드의 실행 속도는 더 빨라집니다.

기계의 메모리가 말 그대로 창고라고 상상해 보세요. 데이터는 여러 개의 쌓인 상자들에 저장되어 있습니다. 특정한 데이터가 필요할 때마다, 그 데이터가 있는 상자를 가져오기 위해 지게차를 불러야 하고, 검수를 위해 그것을 앞쪽 데스크로 옮겨 와야 합니다. 그 앞쪽 데스크에는 한 번에 몇 개의 상자밖에 놓을 수 없으므로, 많은 데이터를 주변에 두고 있을 수는 없습니다.

지게차가 창고 뒤쪽으로 가서 상자를 가져오는 데는 오랜 시간이 걸립니다. 상자들 속에 있는 원하는 데이터가 여기저기 흩어져 있다면, 지게차에게 여러 번 왕복을 시켜야 한다는 뜻입니다. 대부분의 시간 동안에는 앞쪽 데스크 옆에서 다음에 요청한 상자가 도착하기를 기다리며 손 놓고 서 있게 됩니다.

이 상자들을 영리하게 쌓고, 같은 시점에 필요할 가능성이 높은 데이터끼리 대부분 함께 묶이도록 정리하면 그런 낭비되는 시간을 많이 줄일 수 있습니다. 데이터를 현명하게 묶으면, 다음에 필요한 것이 이미 앞쪽 데스크에 있는 상자 중 하나에 들어 있다는 사실을 자주 발견하게 됩니다. 그러면 지게차 운전자를 번거롭게 하지 않고 바로 작업을 이어갈 수 있습니다.

예를 들어, 문이 열려 있는지 닫혀 있는지 확인하기 위해 문의 상태를 가져와야 하는 씬이 있다면, 하드웨어는 그 문의 상태를 나타내는 특정 불리언 값만 가져오는 것이 아니라, 관련이 있을 수도 있고 없을 수도 있는 다른 많은 데이터도 함께 가져옵니다.

씬에 열두 개의 문의 열림/닫힘 상태를 매 프레임마다 업데이트해야 하는 시스템이 있다고 가정해 봅시다. 코드가 객체 지향 프로그래밍 방식으로 구성되어 있다면, 서로 관련된 정보의 각 조각이 어떻게 묶여 있는지 알 수 없습니다. 어쩌면 우리 창고의 한 "상자"에는 문 A의 "isOpen" 상태가 들어 있고, 동시에 문 A의 텍스처와 열릴 때 재생되는 오디오도 들어 있을 수 있습니다. 열두 개의 문 각각에 대해 새로운 데이터 "상자"를 가져오러 가야 할지도 모릅니다. 그리고 물론 이 전체 과정은 매 프레임마다 다시 일어나야 합니다. 그러니 문들의 상태가 전혀 바뀌지 않았더라도, 초당 360번(30 x 12)의 비유적인 창고 뒤쪽 왕복이 발생하는 셈입니다.

반면 코드가 데이터 지향 방식으로 작성되었다면, 그 12개의 불리언은 모두 같은 상자에 들어 있을 가능성이 매우 높습니다. 이는 이 모든 불리언이 한 번에 메모리에 기록된 단일 배열의 일부이기 때문입니다. 메모리에 어떻게 들어갈지를 명시적으로 조직하는 일은 없으므로, 최악의 경우에는 배열이 두 개의 상자로 나뉠 수도 있습니다. 하지만 그 최악의 경우에도 2번의 왕복은 12번보다는 훨씬 낫습니다.

매 프레임마다 모든 문의 상태를 확인하는 일은 많은 작업처럼 들리지만, 모든 데이터가 이미 메모리 캐시에 있다면 실제로는 매우 빠릅니다. 데이터가 깔끔하게 정리되어 있다면, 씬은 이런 과정들을 많은 엔티티에 대해 수행하면서도 여전히 매우 빠르게 실행될 수 있습니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/data-oriented-programming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
