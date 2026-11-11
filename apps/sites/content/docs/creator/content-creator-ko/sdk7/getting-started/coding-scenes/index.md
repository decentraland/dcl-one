> 이 세트는 Decentraland의 클라이언트와 SDK에서 일이 어떻게 작동하는지 이해하는 데 도움이 됩니다.

# 코딩 필수 요소

## 개발 도구

매우 높은 수준에서 보면, Decentraland의 **소프트웨어 개발 키트** (SDK)를 사용하면 다음을 할 수 있습니다:

* 기본 *프로젝트* Decentraland 씬을 포함하는 프로젝트를 생성합니다. 여기에는 콘텐츠를 렌더링하고 실행하는 데 필요한 모든 에셋이 포함됩니다.
* 웹 브라우저에서 로컬로, 완전히 오프라인 상태로, 그리고 이더리움 트랜잭션을 수행하거나 LAND를 소유할 필요 없이 씬의 콘텐츠를 빌드, 테스트, 미리 볼 수 있습니다.
* Decentraland API를 사용해 TypeScript 코드를 작성하여 씬에 상호작용적이고 동적인 동작을 추가합니다.
* 씬의 콘텐츠를 콘텐츠 서버에 업로드합니다.
* 업로드한 콘텐츠의 URL에 LAND 토큰을 연결합니다.

SDK에는 다음이 포함됩니다:

* **크리에이터 허브**: 그 밖에도 간편한 드래그 앤 드롭 인터페이스로 씬을 만들 수 있게 해 주는 독립 실행형 애플리케이션입니다. 미리 보기 실행, 디버그, 코드 편집, 게시를 할 수 있습니다. [자세히 보기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)
* **Decentraland ECS**: 상호작용형 경험을 만들 수 있게 해 주는 도우미 메서드 프레임워크를 담은 TypeScript 패키지입니다. 이를 사용해 씬의 객체를 만들고 조작할 수 있으며, 플레이어나 다른 애플리케이션 간의 인월드 거래를 원활하게 할 수도 있습니다. ( [최신 ECS 레퍼런스](https://github.com/decentraland/ecs-reference/blob/master/docs-latest/decentraland-ecs.md))
* **장면 예제**: 다음에서 영감을 얻고 코딩 모범 사례를 확인하세요 [씬 예제](/docs/creator/content-creator-ko/tutorials-and-examples/examples/).

기타 기존 도구:

* **웹 에디터**: 간단한 씬을 만들고 게시할 수 있는 웹 기반 도구입니다.

## 요구 사항

씬을 로컬에서 개발하려면 LAND 토큰을 소유할 필요가 없습니다. 씬의 개발과 테스트는 이더리움 네트워크(LAND와 Decentraland Name의 소유권을 확립하는 데 Decentraland가 사용하는 시스템)나 콘텐츠 서버에 씬을 배포할 필요 없이 완전히 오프라인으로 진행할 수 있습니다.

다음이 필요합니다:

* **크리에이터 허브**: 그 밖에도 간편한 드래그 앤 드롭 인터페이스로 씬을 만들 수 있게 해 주는 독립 실행형 애플리케이션입니다. 미리 보기 실행, 디버그, 코드 편집, 게시를 할 수 있습니다. [자세히 보기](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/).

씬의 코드를 편집할 계획이라면 다음 중 하나를 설치해야 합니다:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> **Visual Studio Code**: 다운로드하세요 [여기](https://code.visualstudio.com/). 훨씬 더 빠르고 오류를 적게 내면서 코드를 작성하는 데 도움이 됩니다. 소스 코드 편집기는 구문 오류를 표시하고, 입력하는 동안 자동 완성을 제공하며, 현재 맥락에 따라 스마트 제안까지 보여줍니다. 또한 코드에서 객체를 클릭하면 해당 클래스의 전체 정의와 지원하는 속성을 볼 수 있습니다.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> **Cursor AI**: 다운로드하세요 [여기](https://www.cursor.com/). AI와 통합된 강력한 코드 편집기입니다. 코드 작성을 돕기 위해 다양한 AI 모델을 선택할 수 있으며, 모두 무료입니다. AI 비서는 입력하는 동안 자동 완성만 해 주는 것이 아니라, 큰 코드베이스를 리팩터링하거나, 문서를 작성하거나, 그 밖의 작업도 요청할 수 있습니다.


> [!NOTE]
> **💡 팁**: Cursor, OpenDCL, Claude Code 같은 AI 비서를 사용해 일반 언어 설명만으로 전체 씬을 만들 수 있습니다 — TypeScript 경험은 필요하지 않습니다. 다음을 참조하세요 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 시작하려면.


## 지원되는 언어와 문법

Decentraland는 [TypeScript(.ts)](https://www.typescriptlang.org/docs/handbook/jsx.html) 를 씬 작성의 기본 언어로 사용합니다.

TypeScript는 JavaScript의 상위 집합이므로, JavaScript에 익숙하다면 거의 같다고 느낄 것입니다. 하지만 TypeScript에는 타입 선언이 포함되어 있습니다. 타입 선언 덕분에 자동 완성, 더 나은 디버깅 힌트 같은 기능을 사용할 수 있으며, 개발 시간을 단축하고 더 견고한 코드베이스를 만들 수 있습니다. 이러한 기능은 모두 긍정적인 개발자 경험의 핵심 요소입니다.

씬이 빌드되면, 작성한 TypeScript 코드는 더 가볍게 만들기 위해 압축된 JavaScript로 컴파일됩니다. TypeScript의 원본 소스 코드는 서버에 업로드되지 않으며, 컴파일된 JavaScript 버전만 업로드됩니다.

### 다른 언어

TypeScript 대신 다른 도구나 언어를 사용해 JavaScript로 컴파일할 수 있습니다. 단, 컴파일된 스크립트가 씬의 `main` 필드에 설정된 경로와 일치하는 단일 JavaScript 파일 안에 포함되어 있어야 합니다. `scene.json` 파일의 *bin/index.js*). 제공되는 모든 타입 선언은 TypeScript로 작성되며, 다른 언어와 트랜스파일러는 공식적으로 지원되지 않습니다.

## 씬

씬에 배포하는 콘텐츠를 **씬**라고 합니다. 씬은 3D 콘텐츠를 렌더링하는 상호작용형 프로그램으로, 게임일 수도 있고, 상호작용형 경험일 수도 있고, 미술관일 수도 있으며, 원하는 무엇이든 될 수 있습니다!

씬은 Decentraland의 가상 LAND에 배포됩니다. LAND는 이더리움 스마트 계약에서 관리되는 희소한 비대체성 자산입니다. 단일 **구획**, 즉 16미터 x 16미터 크기의 LAND 구역이나 인접한 여러 구획에 배포할 수 있습니다.

플레이어가 Decentraland를 방문하면, 맵을 따라 이동하면서 각 씬의 콘텐츠를 다운로드하고 렌더링합니다. 씬에서 멀어지면 해당 씬은 언로드됩니다.

CLI에서 미리 보기를 실행하여 자신의 컴퓨터에서 씬을 로컬로 실행할 수도 있습니다.

## 엔티티와 컴포넌트

Decentraland의 3차원 씬은 [엔티티-컴포넌트-시스템](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) 아키텍처를 기반으로 하며, 씬의 모든 것은 *엔티티*엔티티입니다 *구성 요소*, 각 컴포넌트는 자신이 속한 엔티티에 특정 속성을 부여합니다. 문 엔티티에는 적어도 Transform 컴포넌트(위치, 회전 및 크기를 설정함)와 도형을 제공하는 또 다른 컴포넌트가 있을 가능성이 큽니다. 컴포넌트는 데이터를 저장하는 장소일 뿐이며, 그 자체로 어떤 동작도 수행하지 않습니다.

![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

```ts
export function main() {
	// 엔티티 생성
	const door = engine.addEntity()

	// 트랜스폼 컴포넌트를 통해 엔티티에 위치 부여
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// GltfContainer 컴포넌트를 통해 엔티티에 보이는 형태 부여
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```

엔티티는 다른 엔티티 안에 중첩되어 트리 구조를 이룰 수 있습니다. 웹 개발에 익숙하다면 엔티티를 DOM 트리의 요소로, 컴포넌트를 각 요소의 속성으로 생각하면 이해하기 쉬울 수 있습니다.

![](/docs/_remote/28a71287061591ece4e52409.webp)

엔티티는 추상적인 개념입니다. 엔티티는 단지 식별자일 뿐이며, 서로 다른 컴포넌트를 묶는 참조로 사용됩니다.

참조 [엔티티와 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/) 이 두 개념과 Decentraland 씬에서 어떻게 사용되는지에 대한 자세한 설명은

### 커스텀 컴포넌트

기본 컴포넌트 집합(예: `Transform`, `컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도`, `머티리얼` , 등)은 엔진에 의해 해석되며, 엔티티의 외형, 위치, 소리 방출 여부 등에 직접적인 영향을 미칩니다.

또한 *사용자 정의 컴포넌트* 를 정의하여 씬의 메커니즘에 유용할 수 있는 데이터를 저장할 수 있습니다. 엔진은 이러한 컴포넌트의 값이 무엇을 의미하는지 해석하는 방법을 알지 못하므로, 씬이 렌더링되는 방식에 직접적인 영향은 없습니다. 하지만 씬 코드에 로직을 작성해 이러한 값을 감시하고 이에 반응할 수 있습니다. 예를 들어 문의 열림/닫힘 상태를 추적하기 위해 사용자 정의 "doorState" 컴포넌트를 정의할 수 있습니다. 이 경우 컴포넌트는 이 상태를 추적하는 값을 저장하는 장소에 불과합니다. 씬에서 문이 열리고 닫히는 것을 보려면, 이 값들을 사용해 문의 회전에 영향을 주는 로직을 별도로 구현해야 하며, 엔진이 해석할 수 있는 `Transform` 컴포넌트의 값이어야 합니다.

참조 [사용자 지정 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 를 참조하세요.

### 이름으로 엔티티 가져오기

Creator Hub의 Scene Editor에서 드래그 앤 드롭으로 추가한 엔티티에도 코드로 접근해 추가 편집과 동작 추가를 할 수 있습니다.

다음을 사용하세요 `engine.getEntityOrNullByName()` 를 사용해 엔티티를 가져오며, Scene Editor UI에서 엔티티에 할당된 이름을 전달합니다. 각 엔티티는 고유한 이름을 가져야 합니다.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```

그런 다음 새 컴포넌트를 추가하거나, 기존 컴포넌트를 수정하거나, 복제하거나, 삭제하는 등 원하는 작업을 할 수 있습니다.

참조 [이름으로 엔티티 가져오기](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/#get-an-entity-by-name) 를 참조하세요.

엔티티가 [스마트 아이템](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)라면, 해당 엔티티의 **동작** 를 호출하거나 그 **트리거** 를 코드로 구독할 수도 있습니다. 다음을 참조하세요 [참조 항목](/docs/creator/content-creator-ko/scene-editor/extend-with-code/reference-items/).

## 시스템

엔티티와 컴포넌트는 씬의 객체에 대한 정보를 저장하는 장소입니다. *시스템* 는 컴포넌트에 저장된 정보를 시간에 따라 변경하는 함수를 담고 있습니다.

시스템은 게임 로직을 구현하는 곳으로, 게임 루프의 매 틱마다 주기적으로 업데이트되거나 확인되어야 하는 동작을 수행합니다.

시스템은 [*업데이트 패턴*](http://gameprogrammingpatterns.com/update-method.html).

```ts
// 기본 시스템
function mySystem() {
	console.log('my system is running')
}

engine.addSystem(mySystem)

// dt가 있는 시스템
function mySystemDT(dt: number) {
	console.log('마지막 프레임 이후 경과 시간:  ', dt)
}

engine.addSystem(mySystemDT)
```

하나의 씬에는 동시에 0개 또는 여러 개의 시스템이 실행될 수 있습니다. 시스템은 씬의 지속 시간 중 서로 다른 시점에 켜거나 끌 수 있습니다. 일반적으로는 독립적인 동작을 별도의 시스템에 분리해 두는 것이 좋습니다.

참조 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 시스템이 씬에서 어떻게 사용되는지에 대한 자세한 내용은

### 게임 루프

해당 [게임 루프](http://gameprogrammingpatterns.com/game-loop.html) 는 Decentraland 씬 코드의 핵심입니다. 정기적인 간격으로 코드의 일부를 순환 실행하며 다음을 수행합니다:

* 플레이어 입력을 감지
* 씬 업데이트
* 씬 다시 렌더링

대부분의 전통적인 소프트웨어 프로그램에서는 모든 이벤트가 플레이어의 행동에 의해 직접 트리거됩니다. 플레이어가 버튼을 클릭하거나 메뉴를 열기 전까지는 프로그램 상태의 어떤 것도 바뀌지 않습니다.

하지만 상호작용형 환경과 게임은 다릅니다. 씬의 모든 변경이 반드시 플레이어의 행동에 의해 발생하는 것은 아닙니다. 씬에는 스스로 움직이는 애니메이션 객체나 자체 AI를 가진 비플레이어 캐릭터가 있을 수도 있습니다. 일부 플레이어 행동은 완료되기까지 여러 틱이 필요할 수도 있습니다. 예를 들어 문을 여는 데 1초가 걸린다면, 문이 움직이는 동안 회전 값은 약 30번에 걸쳐 점진적으로 업데이트되어야 합니다.

루프를 한 번 반복하는 각 과정을 *틱*이라고 합니다. Decentraland 씬은 가능한 경우 초당 30틱으로 렌더링됩니다. 기기가 각 틱을 렌더링하는 데 어려움을 겪으면 업데이트 빈도가 낮아질 수 있습니다.

각 틱마다 씬이 업데이트된 다음, 업데이트된 값을 바탕으로 다시 렌더링됩니다.

Decentraland 씬에는 명시적으로 선언된 게임 루프가 없지만, 씬의 [시스템](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 이 게임 루프를 구성합니다.

씬의 컴파일과 렌더링은 백엔드에서 수행되므로, 씬을 개발하는 동안 직접 처리할 필요가 없습니다.

## 컴포넌트 쿼리하기

할 수 있습니다 [컴포넌트를 조회](/docs/creator/content-creator-ko/sdk7/architecture/querying-components/) 메서드를 사용하여 `engine.getEntitiesWith(...components)` 특정 컴포넌트를 가진 씬의 모든 엔티티를 추적할 수 있습니다.

대개는 [system](/docs/creator/content-creator-ko/sdk7/architecture/systems/) 안에서 컴포넌트를 쿼리한 뒤, 반환된 각 엔티티를 순회하면서 각각에 같은 작업 집합을 수행하는 것이 합리적입니다.

게임 루프의 매 틱마다 씬의 모든 엔티티를 반복 처리하려고 하면 성능에 상당한 부담이 될 수 있습니다. 쿼리에서 반환된 엔티티만 참조하면 관련 있는 것만 다루고 있다는 점을 보장할 수 있습니다.

```ts
// 시스템 정의
function boxHeightSystem(dt: number) {
	// MeshRenderer와 Transform 컴포넌트가 모두 포함된 엔티티를 쿼리
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.get(entity)
		console.log('박스의 높이:  ', transform.position.y)
	}
}

// 엔진에 시스템 추가
engine.addSystem(boxHeightSystem)
```

## 씬 생명주기

직접 느슨한 코드 줄을 `index.ts`에 작성하기 시작하면, 코드에 중요한 맥락이 일부 빠져 있을 수 있습니다. 예를 들어 플레이어 엔티티에 대한 정보나 Creator Hub에서 드래그 앤 드롭으로 추가된 엔티티에 대한 정보가 없을 것입니다. 코드를 읽는 시점에는 그런 것들이 아직 로드되지 않았습니다.

그런 상황을 피하려면, 씬의 초기 로딩 코드는 항상 `main()` 함수를 사용해( `index.ts` 파일의) 엔트리포인트로 작성하는 것이 좋습니다. 이 함수는 씬의 초기 맥락이 모두 로드된 후에만 실행되며, 여기에는 Scene Editor UI를 통해 추가된 모든 항목이 포함됩니다.

다음 경우에는 `main()` 함수 밖에 코드를 작성할 수 있습니다:

* 코드가 다음에 의해 간접적으로 호출되는 경우 `main()`
* 코드가 시스템을 정의하거나 엔진에 시스템을 추가하는 경우
* 코드가 [비동기 함수](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/)


> [!WARNING]
> **📔 참고**: async 함수나 시스템 안의 코드는 처음 실행될 때 이미 씬의 모든 것이 제대로 초기화된 상태입니다.
> 
> [사용자 정의 컴포넌트](/docs/creator/content-creator-ko/sdk7/architecture/custom-components/) 정의는 예외이며, 이는 반드시 `main()` 함수 바깥, 별도 파일에 작성해야 합니다. 이들은 `main()` 가 실행되기 전에 해석되어야 합니다.


## 가변성

컴포넌트의 가변 버전 또는 불변(읽기 전용) 버전 중 하나를 선택할 수 있습니다. 컴포넌트의 `.get()` 함수는 컴포넌트의 불변 버전을 반환합니다. 값은 읽을 수 있지만, 그 속성들을 변경할 수는 없습니다.

해당 `.getMutable()` 함수는 값을 변경할 수 있는 컴포넌트 표현을 반환합니다. 컴포넌트에 변경을 가할 계획일 때만 가변 버전을 사용하세요. 불변 버전을 다루면 성능이 크게 향상됩니다.

```ts
// 불변 버전 가져오기(읽기 전용)
const immutableTransform = Transform.get(myEntity)

// 다음은 작동하지 않습니다:
// 	immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// 아래 코드는 엔티티의 위치를 변경합니다
mutableTransform.position.y = 2
```

참조 [변경 가능한 데이터](/docs/creator/content-creator-ko/sdk7/programming-patterns/mutable-data/) 자세한 내용은.

## 전체를 하나로 묶어 보기

해당 *엔진* 는 *엔티티*, 그리고 *구성 요소* 와 *시스템* 사이에 있는 것입니다.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

씬의 컴포넌트에 저장된 모든 값은 해당 시점의 씬 상태를 나타냅니다. 게임 루프의 매 틱마다 엔진은 각 시스템의 함수를 실행하여 컴포넌트에 저장된 값을 업데이트합니다.

모든 시스템이 실행된 후에는 각 엔티티의 컴포넌트가 새로운 값을 갖게 됩니다. 엔진이 씬을 렌더링할 때는 이 새로 업데이트된 값을 사용하며, 플레이어는 엔티티가 새 상태에 맞게 바뀌는 것을 보게 됩니다.

```ts
export function main() {
	// 엔티티 생성
	const cube = engine.addEntity()

	// 트랜스폼 컴포넌트를 통해 엔티티에 위치 부여
	Transform.create(cube, {
		position: Vector3.create(5, 1, 5),
	})

	// MeshRenderer 컴포넌트를 통해 엔티티에 보이는 형태를 부여
	MeshRenderer.setBox(cube)
}

// 시스템 정의
function rotationSystem(dt: number) {
	// MeshRenderer와 Transform 컴포넌트가 모두 포함된 엔티티를 쿼리
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.getMutable(entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(dt * 10, Vector3.Up())
		)
	}
}

// 엔진에 시스템 추가
engine.addSystem(rotationSystem)
```

위 예시에서는 `큐브` 엔티티와 `rotationSystem` 시스템이 엔진에 추가됩니다.  `큐브` 엔티티에는 `Transform`, 그리고  `MeshRenderer` 컴포넌트가 있습니다. 게임 루프의 매 틱마다 `rotationSystem` 시스템이 호출되며, `Transform` 컴포넌트의 회전 값을 변경합니다. `큐브` 엔티티를 사용하여 플레이어의 카메라 위치에서 앞으로 레이를 추적합니다.

위의 대부분 코드는 씬을 불러올 때 한 번만 실행된다는 점에 유의하세요. 예외는 `rotationSystem` 시스템으로, 이는 게임 루프의 매 틱마다 호출됩니다.

## 씬 분리

씬은 엔진(즉, 메인 스레드)과 같은 맥락에서 실행되지 않습니다. 우리는 SDK를 렌더링 엔진과 완전히 분리된 방식으로 만들었습니다. 안전성과 성능상의 이유로 이렇게 설계했습니다.

이러한 분리 때문에 씬 코드에서는 DOM이나 `window` 객체에 접근할 수 없으므로, 플레이어의 브라우저나 지리적 위치 같은 데이터에 접근할 수 없습니다.

이 분리는 RPC 프로토콜을 사용해 이루어집니다. 이 프로토콜은 클라이언트의 일부를 씬을 렌더링하고 이벤트를 제어하는 역할만 하도록 할당합니다.

또한 통신 프로토콜도 추상화했습니다. 덕분에 씬을 WebWorker에서 로컬로 실행할 수 있습니다.

개발자가 엔진의 내부 구조에 개입하거나, 심지어 엔진 내부가 무엇인지 알 필요가 없도록 하고 싶습니다. Decentraland 맵 전체에서 플레이어에게 일관된 경험을 보장해야 하며, 그런 '낮은' 수준에서 실수가 더 쉽게 발생할 수 있기 때문입니다.

이러한 분리는 플레이어가 다른 사람의 씬에 있는 동안 이웃한 씬이 플레이어의 경험에 간섭하는 것을 막는 데도 중요합니다. 플레이어는 동시에 여러 개의 인근 씬을 로드한 상태일 수 있으며, 각각은 자체 코드를 실행합니다. 일부 동작(예: 외부 링크 열기 또는 플레이어 이동)은 플레이어가 해당 씬 위에 서 있을 때만 허용되며, 씬이 로드되어 있더라도 플레이어가 바깥에 있으면 허용되지 않습니다.

## 트리 쉐이킹

TypeScript의 소스 코드를 압축된 JavaScript의 컴파일된 코드로 변환할 때, 이 과정은 [트리 쉐이킹](https://en.wikipedia.org/wiki/Tree_shaking) 을 수행하여 실제로 사용 중인 코드 부분만 변환되도록 합니다. 이는 씬의 최종 코드를 최대한 가볍게 유지하는 데 도움이 됩니다. 외부 라이브러리를 사용할 때 특히 유용한데, 이런 라이브러리에는 종종 사용하지 않는 기능이 많이 포함되어 있어 그렇지 않으면 씬이 불필요하게 커지기 때문입니다.

트리 쉐이킹의 결과로, 씬에서 실행되길 원하는 코드가 있다면 코드의 엔트리 포인트인 `main()` 의 `index.ts` 함수에 의해 어떤 방식으로든 참조되어야 합니다. 시스템은 또한 `index.ts` 파일에서 `main()`를 참조하지 않고도 엔진에 추가할 수 있습니다. 이러한 파일들에서 명시적 또는 간접적으로 참조되지 않는 코드는 씬에 포함되지 않습니다.

예를 들어, `extraContent.ts` 라는 파일이 있고 다음과 같은 내용이 있다면, 엔티티는 렌더링되지 않고 시스템도 실행되기 시작하지 않습니다:

```ts
// extraContent.ts

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: { x: 8, y: 0, z: 8 },
})
MeshRenderer.setBox(myEntity)

function mySystem(dt: number) {
	console.log('system running')
}

engine.addSystem(mySystem)
```

씬의 일부로 실행되게 하려면, 다음과 같이 `index.ts` 에서 참조할 수 있습니다:

```ts
// extraContent.ts에서

export function addEntities() {
	const myEntity = engine.addEntity()
	Transform.create(myEntity, {
		position: { x: 8, y: 0, z: 8 },
	})
	MeshRenderer.setBox(myEntity)
}

export function mySystem(dt: number) {
	console.log('system running')
}

/////////////////////////////

// index.ts에서

import { addEntities, mySystem } from './extraContent'

export function main() {
	addEntities()
}

engine.addSystem(mySystem)
```

이 규칙의 예외는 사용자 정의 컴포넌트의 정의입니다. 이들은 `main()` 함수 엔트리포인트를 통해 접근해서는 안 됩니다. 왜냐하면 다른 모든 것보다 먼저 해석되어야 하기 때문입니다.

## 가져오기

씬에서 사용되는 모든 함수, 객체, 컴포넌트 및 기타 요소는 사용하려는 각 파일로 가져와야 합니다. 이는 [트리 쉐이킹](#tree-shaking)의 결과로, 전체 SDK를 패키징하는 대신 씬이 사용하는 부분만 포함되기 때문입니다.

문서 전반의 예제 스니펫은 깔끔하게 보이도록 각 파일 맨 앞의 import 줄을 생략하지만, 작동하게 하려면 씬에 추가해야 합니다.

VS Studio Code로 씬을 작성할 때는 스마트 자동 완성 옵션이 입력 시 import 처리를 알아서 해 주므로, 이를 따로 의식할 필요가 없습니다.

하지만 스니펫을 씬에 붙여 넣으면, 해당 파일로 import되지 않은 일부 요소가 빨간색으로 표시되는 것을 볼 수 있습니다. 이를 수정하려면:

* 밑줄이 그어진 각 단어를 클릭하세요
* 줄 왼쪽의 전구 아이콘을 클릭하세요
* 선택 **Import From 추가**
* 파일 맨 앞에 import 줄이 나타납니다.

![](/docs/_remote/d183f4383d8bff7a66afa15b.webp)

가져와야 할 항목이 많다면 **누락된 모든 import 추가** 를 같은 드롭다운에서 선택할 수도 있습니다.

import는 요소가 사용되는 모든 파일에 추가해야 한다는 점에 유의하세요.

VS Studio Code는 import의 올바른 경로를 자체적으로 해결할 수 있어야 합니다. 어떤 이유로든 문제가 생기면, 파일 맨 앞에 다음의 빈 import 문을 붙여 넣어 보세요. 그러면 VS Studio가 나머지를 처리할 수 있을 것입니다.

```ts
import {} from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'
```

## SDK 버전

새 씬을 개발할 때는 기본적으로 `@latest` 안정 버전 SDK 릴리스를 사용합니다.

다음을 설치할 수 있습니다 `@next` 최신 안정 릴리스에는 아직 포함되지 않은 예정 기능을 활용하거나 미리 보려면 SDK 릴리스를 사용하세요.

그렇게 하려면 씬의 `package.json` 파일을 열고 다음 줄을 변경하세요:

```json
  "devDependencies": {
    "@dcl/js-runtime": "next",
    "@dcl/sdk": "next"
  },
```

그런 다음 씬 프로젝트 폴더에서 다음 명령을 실행하세요:

```
npm i
```

참조 [의존성 관리](/docs/creator/content-creator-ko/sdk7/libraries/manage-dependencies/) 자세한 내용은.


> [!WARNING]
> **📔 참고**: @next 버전은 때때로 문제가 발생할 수 있다는 점을 염두에 두세요. 새 기능의 문법과 이름은 안정 버전으로 릴리스되기 전에 바뀔 수 있습니다.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/coding-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
