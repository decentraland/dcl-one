> 복합 파일에서 런타임에 엔티티와 컴포넌트의 트리를 생성합니다

# 복합체

하나는 *컴포지트* 는 트리 구조를 설명하는 파일입니다 [엔티티와 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/). 코드를 작성해 각 엔티티를 하나씩 만드는 대신, 그 구조를 컴포지트 파일에 저장하고 한 번의 호출로 전체를 생성할 수 있습니다.

이미 모르는 사이에 컴포지트를 사용하고 있을 수도 있습니다:

* 모든 씬에는 `main.composite` 파일이 있습니다. 이 파일은 [씬 편집기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)에서 시각적으로 추가하고 설정한 모든 내용을 보관합니다. 이 파일은 씬이 시작될 때 자동으로 로드됩니다.
* [Custom Items](/docs/creator/content-creator-ko/scene-editor/interactivity/custom-items/) 은(는) 다음과 같이 저장됩니다 `composite.json` 파일입니다. 각 Custom Item은 씬 전체에서 재사용할 수 있는 컴포지트입니다.

이 페이지에서는 런타임에 코드로 컴포지트를 생성하는 방법을 다룹니다. 예를 들어 씬의 로직의 일부로 Custom Item을 동적으로 생성하는 경우입니다.

## 컴포지트 생성

컴포지트를 생성하는 데는 두 단계가 필요합니다: **로드** 하고, 그다음 **생성합니다** . 로드는 비동기이고 생성은 동기이므로, 패턴은 항상 로드 후 생성입니다:

```ts
import { engine, Composite, getCompositeProvider } from "@dcl/sdk/ecs";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  // 1. 파일에서 컴포지트를 로드합니다
  const resource = await provider.loadComposite(src);

  // 2. 생성: 그 안의 모든 엔티티와 컴포넌트를 만듭니다
  const barrel = Composite.instance(engine, resource, provider);
  return barrel;
}
```

`provider.loadComposite(src)` 는 컴포지트 파일을 읽어 메모리에 로드합니다. 다음으로 빌드된 일반적인 씬에서는 `@dcl/sdk`로, 이미 컴포지트 프로바이더가 설정되어 있으며, 다음으로 접근할 수 있습니다 `getCompositeProvider()` 함수 `@dcl/sdk/ecs`.

`Composite.instance()` 는 그다음 컴포지트에 설명된 모든 엔티티를 모든 컴포넌트와 함께 생성하고, 스폰된 트리의 **루트 엔티티** 를 반환합니다. 나중에 컴포넌트를 읽거나 변경할 때, 예를 들어 스폰된 아이템의 위치를 바꾸거나 제거할 때 이 반환된 엔티티를 사용하세요.


> [!NOTE]
> **💡 팁**: 코드를 작성하지 않고 같은 작업을 하려면 다음을 사용하세요 **엔티티 생성** 작업을 씬 편집기에서 사용하세요. 다음을 참고하세요 [엔티티 생성에 관하여](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities).



> [!WARNING]
> **📔 참고**: 컴포지트를 스폰하기 전에 먼저 로드해야 합니다. `Composite.instance()` 는 동기식이며 이미 메모리에 있는 컴포지트만 생성할 수 있습니다. 오직 `main.composite` 는 씬에 번들로 포함되어 처음부터 사용할 수 있습니다. 그 외의 다른 컴포지트 파일은 다음으로 로드해야 합니다 `loadComposite()` 먼저. 컴포지트가 이미 사용 가능한지 동기적으로 확인하려면 다음을 사용하세요 `provider.getCompositeOrNull(src)`.
> 
> `loadComposite()` 는 멱등적입니다. 각 컴포지트를 해당 `src` 문자열로 키를 지정하므로, 같은 경로로 다시 호출해도 파일을 다시 로드하지 않고 이미 로드된 컴포지트를 반환합니다. 같은 파일을 두 번 로드할 걱정 없이 매번 생성 전에 안전하게 호출할 수 있습니다.


### 기존 엔티티에 생성하기

기본적으로 `Composite.instance()` 는 생성된 트리를 담을 새 엔티티를 만듭니다. 다음을 전달하세요 `rootEntity` 이미 가지고 있는 엔티티에 생성하려면 옵션에:

```ts
// 래퍼 엔티티 없이 씬 루트에 바로 컴포지트를 생성합니다
const barrel = Composite.instance(engine, resource, provider, {
  rootEntity: engine.RootEntity,
})
```

## 생성된 컴포지트의 위치 지정

생성된 컴포지트를 특정 위치, 회전 또는 크기에 배치하려면 `Transform` 컴포넌트를 다음이 반환한 루트 엔티티에 설정하세요 `Composite.instance()`.

```ts
import { engine, Composite, getCompositeProvider, Transform } from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  const resource = await provider.loadComposite(src);

  // 특정 위치에 컴포지트를 생성합니다
  const barrel = Composite.instance(engine, resource, provider);
  Transform.createOrReplace(barrel, {
    position: Vector3.create(8, 0, 8),
  });
}
```


> [!WARNING]
> **📔 참고**: `Transform.createOrReplace()` 는 루트 엔티티의 기존 Transform 컴포넌트를 대체합니다. 루트 엔티티만 영향을 받으며, 자식 엔티티는 루트에 대한 상대적 위치를 유지합니다.


## 현재 제한 사항: 중첩 컴포지트

생성은 자체 포함된 컴포지트와 Custom Item에서 작동합니다. 컴포지트는 자신의 엔티티 중 하나에서 다른 컴포지트를 참조할 수 있지만 `loadComposite()` 합니다 **아니며** 중첩 참조 안으로 재귀하지는 않습니다: 전달한 파일만 로드합니다.

생성할 때 `Composite.instance()` 이미 메모리에 있는 중첩 참조로 재귀합니다. 중첩 컴포지트가 로드되어 있지 않으면( `main.composite`를 제외한 다른 모든 경우), 해당 분기는 경고를 기록하고 건너뜁니다. 중첩 참조가 있는 컴포지트를 생성하려면 `loadComposite()` 를 각 참조 파일마다 먼저 호출하세요 — 아니면 더 간단히, 생성하는 컴포지트를 자체 포함형으로 유지하세요.

## 관련 페이지

* [엔티티 및 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/) — 컴포지트가 설명하는 빌딩 블록입니다.
* [Custom Items](/docs/creator/content-creator-ko/scene-editor/interactivity/custom-items/) — 컴포지트로 저장되는 재사용 가능한 아이템입니다.
* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/#about-spawning-entities) — Spawn Entity 액션을 사용해 코드 없이 컴포지트를 생성합니다.
* [씬 파일](/docs/creator/content-creator-ko/sdk7/kinds-of-projects/scene-files/) — `main.composite` 가 씬 프로젝트에 있는 위치


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/architecture/composites.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
