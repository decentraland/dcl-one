> 아이템 상태 및 조건부 로직 관리

# 상태 및 조건



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/wm8ZD2kSyKA" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=wm8ZD2kSyKA>">Watch on YouTube</a></p>



## 조건부 논리

트리거에 조건을 추가하여, 해당 조건이 충족될 때만 동작이 발생하도록 할 수 있습니다. 예를 들어, 문을 클릭하면 문이 이미 열려 있지 않은 경우에만 "열기" 동작이 활성화됩니다.

조건을 추가하려면 옆의 세 점 아이콘을 클릭하세요 **트리거 이벤트** 를 누르고 **트리거 조건 추가**.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

하나의 트리거에는 여러 조건을 포함할 수 있습니다.  **+** 아이콘을 눌러 조건을 더 추가하세요. 조건이 두 개 이상일 경우, 다음 옵션 중 하나를 선택할 수 있습니다:

* **모든 조건이 충족되어야 함(AND)**: 트리거는 모든 조건이 참일 때만 발생합니다.
* **어떤 조건이든 하나만 충족되어도 됨(OR)** 조건 중 하나 이상이 참이면 트리거가 발생합니다.

![](/docs/_remote/95e6cb8369815d94d5f26f5a.webp)

### States

해당 **States** 컴포넌트는 여러 스마트 아이템에 포함되어 있습니다. 이는 스마트 아이템이 취할 수 있는 가능한 상태를 나열합니다. 어떤 시점에서든 스마트 아이템은 이러한 상태 중 하나에 있습니다. 예를 들어, 문은 *Open* 또는 *닫힘*. 열기 동작은 상태를 *Open*, 닫기 동작은 상태를 *닫힘*.

상태로 다음과 같은 작업을 할 수 있습니다:

1. 트리거에서 조건을 사용해 엔티티의 상태를 확인하세요. 이렇게 하면 특정 상태가 활성화된 경우에만 동작이 수행됩니다.

![](/docs/_remote/39d3afce89ca9fad61d21f61.webp)

2. 다음을 통해 상태를 변경합니다 **상태 설정** 동작.

![](/docs/_remote/6f9bc751de8405306a6f58e0.webp)

3. 다음을 통해 상태 변화에 반응합니다 **상태 변경 시** 트리거 이벤트.

두 동작을 전환하려면, 상태를 확인하는 조건이 각각 있는 두 개의 트리거를 정의하세요. 예를 들어, 문에는 열기 동작을 활성화하는 트리거가 하나 있고, 이 트리거의 조건은 먼저 문의 상태가 *닫힘*, 그리고 닫기 동작을 활성화하는 다른 트리거가 있으며, 그 조건은 문의 상태가 *Open*. 플레이어가 문을 클릭할 때마다 두 개 중 하나만 활성화됩니다.

![](/docs/_remote/819921eca04a4f69ab10b587.webp)

스마트 아이템에는 원하는 만큼 상태를 추가할 수 있습니다.  **새 상태 추가** 버튼을 눌러 목록에 하나를 더 추가하세요.

![](/docs/_remote/fb464b1832beb2b74eb0d3e9.webp)

상태 중 하나가 기본값으로 선택되며, 씬이 실행될 때 아이템은 항상 이 상태에서 시작합니다. 다른 상태 옆의 세 점을 클릭하고 다음을 선택하여 다른 상태를 기본값으로 지정할 수 있습니다 **기본값으로 설정**.


> [!NOTE]
> **💡 팁**: 아이템 간 상호작용은 단순하게 유지하세요. 예를 들어, 문 애니메이션 재생, 문 소리 재생, 문의 상태 변경이라는 세 가지 동작을 트리거하여 문을 여는 버튼 같은 상황은 피하세요. 대신 버튼이 문의 상태를 변경하도록 하세요. 그런 다음 **상태 변경 시** 트리거를 사용해 상태가 바뀔 때마다 문 자체가 애니메이션과 소리를 재생하도록 하세요.


### Counter

다음을 사용하세요 **Counter** 컴포넌트를 사용해 숫자를 추적하세요. 이 숫자는 플레이어가 씬에서 동작을 수행함에 따라 바뀔 수 있습니다. 조건부 논리에서 카운터의 값을 사용할 수 있습니다.

엔티티에 Counter 컴포넌트가 있으면 다음 동작을 실행할 수 있습니다:

* **카운터 증가**: 카운터 값을 설정 가능한 **수량만큼 증가시킵니다**, 기본값은 1입니다.
* **카운터 감소**: 카운터 값을 설정 가능한 **수량만큼 증가시킵니다**, 기본값은 1입니다.
* **카운터 설정**: 카운터 값을 특정 숫자로 설정합니다. 예를 들어 0으로 되돌릴 수 있습니다.

다음을 사용하세요 **카운터 변경 시** 트리거를 사용해 카운터 값이 바뀔 때마다 동작을 수행하세요. 이 트리거에 조건을 추가해 특정 임계값을 넘은 뒤에만 활성화되도록 하세요.

![](/docs/_remote/c4189a0d754c03116088f69e.webp)

조건에서 카운터 값이 다음인지 확인할 수 있습니다

* 주어진 값보다 큼
* 주어진 값보다 작음
* 주어진 값과 같음


> [!NOTE]
> **💡 팁**: 크거나 같은지 확인하려면 AND 옵션을 사용해 트리거 이벤트에 두 개의 조건을 추가할 수 있습니다.
> 
> 임계값을 넘을 때 동작이 한 번만 발생하고 그 이후의 모든 증가마다 반복되지 않게 하려면, 카운터를 다음과 결합하세요 **상태** 컴포넌트. 원하는 값에 도달할 때마다 상태를 "완료"로 설정하고, 트리거 이벤트에서 이 상태를 확인하는 조건을 추가하세요.


### 또한 참조

* [스마트 항목 - 기본](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)
* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/)
* [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)
* [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
