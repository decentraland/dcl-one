> 공식 SDK Skills를 사용한 AI 지원 바이브 코딩과 Creator Hub, SDK로 첫 Decentraland 씬을 만드세요.

# SDK 빠른 시작

이 튜토리얼에서는 Creator Hub의 시각적 Scene Editor, Decentraland SDK를 사용한 직접 작성 코드, 그리고 AI 지원 **바이브 코딩**입니다. 세 가지를 모두 함께 사용하며, 각각이 언제 가장 빛나는지도 배우게 됩니다.

## Decentraland 씬은 어떻게 만들나요?

1. **먼저** [**크리에이터 허브**](https://decentraland.org/download/creator-hub), 씬을 만들고, 미리 보고, 게시하기 위한 공식 데스크톱 앱입니다. 모든 숙련도의 크리에이터에게 권장되는 도구입니다.
2. **AI 코딩 어시스턴트를 사용하는 경우** (Claude Code, Cursor, Copilot 같은) 공식 [Decentraland SDK 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 를 설치하여 검증된 SDK7 패턴을 알 수 있도록 하세요: `npx skills add decentraland/sdk-skills`
3. **해당** [**CLI**](/docs/creator/content-creator-ko/sdk7/getting-started/using-the-cli/) 는 고급 사용자와 자동화 워크플로를 위한 대안입니다.

이 튜토리얼의 나머지 부분에서는 그 권장 경로를 단계별로 따라갑니다.

## Creator Hub 설치

Creator Hub를 사용하면 Decentraland 씬을 만들고, 미리 보고, 배포할 수 있습니다. Creator Hub를 다운로드하세요 [여기](https://decentraland.org/download/creator-hub).

씬 코드를 편집하려면 코드 편집기도 필요합니다. [Visual Studio Code](https://code.visualstudio.com/) 및 [Cursor](https://www.cursor.com/) 둘 다 훌륭한 선택이지만, 어떤 코드 편집기든 괜찮습니다.

다음을 읽어보세요 [설치 가이드](/docs/creator/content-creator-ko/scene-editor/get-started/editor-installation/) 자세한 내용은.

## 첫 번째 씬 만들기

1. Creator Hub를 여세요.
2. 다음을 선택하세요 **씬** 탭을 선택하고, 클릭하세요 **새 씬**.

   ![](/docs/_remote/efc0e8d68c33532f7a840f5e.webp)
3. 시작 템플릿을 선택하세요. 이 연습에서는 다음을 선택하세요 **빈 씬**.

이 단계에는 몇 분 정도 걸릴 수 있습니다. 기본 씬에 필요한 기본 파일 세트를 폴더에 채워 넣습니다. 완료되면 씬의 빈 그리드가 보입니다.

## 에셋 팩에서 항목 추가하기

다음을 살펴보세요 **Asset packs** 를 Scene Editor의 하단 섹션에서 살펴보고, 몇 개의 항목을 씬에 끌어다 놓으세요. 지금은 어떤 항목이든 괜찮습니다.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

이미 배치된 항목은 클릭한 뒤 드래그하여 위치를 바꿀 수 있습니다. 다음을 참고하세요 [씬 편집기 필수 요소](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#position-items) 자세한 내용은.


> [!NOTE]
> **💡 팁**: 씬 전체를 바닥 항목으로 덮으세요. 유형이 **지면** 에는 페인트 통 아이콘이 있습니다. 이 항목 중 하나를 장면으로 끌어오면 장면의 모든 지면이 이 항목의 복사본으로 덮입니다.
> 
> <img src="/docs/_remote/e120bb99677fe9cb301458fb.webp" alt="Ground" data-size="original">


## 미리보기 실행

다음을 클릭하세요: **재생** 버튼을 눌러 Decentraland 안에서 씬을 불러오세요. 이제 Decentraland 아바타로 씬을 둘러볼 수 있습니다.

![](/docs/_remote/77e30200d3905dd6bed10959.webp)

작업하면서 미리보기 창을 열어 둬도 됩니다. 변경할 때마다 업데이트됩니다. 자세한 내용은 [씬 미리보기](/docs/creator/content-creator-ko/sdk7/getting-started/preview-scene/).

## 사용자 지정 3D 에셋

다음에서 이 아보카도 3D 모델을 다운로드하세요 *glb* 형식으로 다음 [링크](https://github.com/decentraland-scenes/avocado/raw/main/avocado-glb.zip) 에서 압축을 풀어주세요.

![](/docs/_remote/54ed4744f3c2c873fc597b60.webp)

다음 파일을 끌어다 놓으세요 **avocado.glb** 파일을 파일 탐색기에서 Scene Editor 하단 패널로 옮기세요(같은 패널에는 **Asset Packs** 및 **로컬 에셋** 탭도 들어 있습니다) 그리고 **가져오기**.

![](/docs/_remote/63d04fd8ef301b90c747fa37.webp)

이제 다음을 찾을 수 있습니다 **avocado.glb** 모델을 **로컬 에셋** 탭 안의 **씬** 폴더입니다. 파일을 씬으로 끌어다 놓아 Asset Packs의 다른 항목처럼 사용하세요.

![](/docs/_remote/23bd93a0c282b7207519dc58.webp)

## 씬 코드 편집

다음을 클릭하세요: **<> 코드** 버튼을 눌러 씬 프로젝트를 코드 편집기에서 여세요.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)


> [!WARNING]
> **📔 참고**: 아무것도 열리지 않으면, 다음과 같은 코드 편집기가 설치되어 있는지 확인하세요. [Visual Studio Code](https://code.visualstudio.com/) 또는 [Cursor](https://www.cursor.com/) 설치되어 있어야 합니다.


코드 편집기의 왼쪽 여백에서 프로젝트의 파일 및 폴더 구조를 탐색할 수 있습니다. 다음을 여세요 `index.ts` 파일을 다음 안의 `src` 폴더입니다. 내용은 다음과 같아야 합니다:

```ts
import {} from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'

export function main() {}
```

Decentraland 씬은 다음 언어로 작성됩니다 [TypeScript](https://www.typescriptlang.org/), Decentraland SDK를 사용합니다. 이 라이브러리에는 3D 콘텐츠를 배치하고, 상호작용을 추가하고, 씬에서 일어나는 일을 제어하는 데 필요한 모든 것이 들어 있습니다.

이 파일은 다음이라는 함수를 정의합니다 `main()`, 이것이 씬의 진입점입니다: 여기에 넣은 코드는 씬이 처음 로드될 때 실행됩니다.

이미 한 개의 아보카도를 시각적으로 씬에 끌어다 놓았습니다. 이제 두 번째 아보카도를 코드로 추가해 봅시다. 다음으로, 여러분의 `index.ts` 파일을 다음 내용으로 바꾸세요:

```ts
import { Vector3 } from '@dcl/sdk/math'
import { engine, Transform, GltfContainer } from '@dcl/sdk/ecs'

export function main() {
	// 새 Entity를 만듭니다
	let avocado2 = engine.addEntity()

	// Transform을 부여합니다
	Transform.create(avocado2, {
		position: Vector3.create(8, 0, 8),
	})

	// GLTF를 부여합니다
	GltfContainer.create(avocado2, {
		src: 'assets/scene/avocado.glb',
	})
}
```

이 줄들은 새 [엔티티](/docs/creator/content-creator-ko/sdk7/architecture/entities-components/) 그것에 [형태](/docs/creator/content-creator-ko/sdk7/3d/shape-components/) 다운로드한 3D 모델을 기반으로 한 [위치를 설정하고](/docs/creator/content-creator-ko/sdk7/3d/entity-positioning/) 다음을 통해 **Transform** 컴포넌트도 부여해야 합니다.

일반적으로, `index.ts` 여러분이 작성하는 코드는 모두 `main()` 안에 있어야 합니다(또는 main에서 간접적으로 참조되는 다른 함수 안에 있어야 합니다). 다음을 참고하세요 [씬 생명주기](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/#scene-lifecycle) 자세한 내용은.

씬 미리보기를 실행하세요: 이제 Scene Editor에서 추가한 아보카도와 코드로 추가한 아보카도, 두 개가 보여야 합니다.

![](/docs/_remote/9e7996b651530bdb72380bb3.webp)


> [!WARNING]
> **📔 참고**: 두 번째 아보카도는 **오직 코드에만 존재합니다**. 씬을 실행하면 나타나지만, Scene Editor 캔버스와 엔티티 트리는 다음에서 생성된 엔티티를 표시할 수 없습니다 `index.ts`. 편집기에서 보이지 않더라도 놀라지 마세요. 시각적 편집과 코드를 함께 사용할 때 기억해야 할 핵심 사항입니다.


두 아보카도는 모두 같은 구성 요소로 만들어졌습니다. 다음을 선택하세요 **첫 번째** 아보카도를 Scene Editor에서 선택하면: 속성 패널에 **Transform** 그리고 **컴포넌트가 있거나, 아예 보이는 형태가 없는 엔티티에도** 컴포넌트가 코드에서 방금 작성한 것과 같은 종류의 필드를 보여 줍니다. 둘은 같은 것의 두 가지 보기입니다.

![](/docs/_remote/90ea2c66a117f593fd4e77cc.webp)

## Script로 상호작용 추가하기

아보카도가 플레이어에게 반응하게 만들어 봅시다. 이미 편집 중인 `index.ts` 에 코드를 추가하는 방법도 있지만, 대신 **Script 컴포넌트**: Scene Editor의 항목에 코드를 직접 연결하는 방법입니다. 각 항목의 동작을 독립적으로 유지해 주며, 같은 스크립트를 여러 항목에 재사용할 수도 있습니다.

1. Scene Editor에서 첫 번째 아보카도(끌어다 놓은 것)를 선택하세요. 코드로만 만든 아보카도는 여기에는 보이지 않는다는 점을 기억하세요.
2. 다음을 클릭하세요: **+** 버튼을 속성 패널 상단에서 눌러 **Script** 컴포넌트를 추가하세요.
3. 클릭 **+ 새 스크립트 만들기** 라고 이름을 지정하고 `AvocadoScript`.

![](/docs/_remote/73d08b2cfd32963d68a7790e.webp)

4. 다음을 클릭하세요: **<> 코드** Script 컴포넌트의 버튼을 눌러 새 파일을 코드 편집기에서 여세요.

스크립트는 세 개의 주요 부분으로 이루어진 클래스입니다(간결하게 보기 위해 주석은 줄였습니다):

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string, // 삭제하지 마세요
		public entity: Entity, // 삭제하지 마세요
		// 아래에 사용자 지정 입력을 추가하세요
	) {}

	start() {
		// 씬이 로드될 때 한 번 호출됩니다
	}

	update(dt: number) {
		// 매 프레임마다 호출됩니다
	}
}
```

* 해당 **constructor** constructor는 Creator Hub의 Script 컴포넌트에 편집 가능한 필드로 표시되는 매개변수를 정의합니다. 삭제하지 마세요 `src` 또는 `엔티티`.
* **`start()`** 실행됩니다 **한 번**, 씬이 로드될 때. 설정 작업(컴포넌트 생성, 클릭 핸들러 등록 등)에 사용하세요.
* **`update(dt)`** 다음에서 실행됩니다 **매 프레임마다** 게임의, 대략 초당 30회. 이동 같은 지속적인 동작에 사용하세요. `dt` 마지막 프레임 이후 몇 초가 지났는지 알려 줍니다.

클래스 내부에서, `this.entity` 는 항상 Script 컴포넌트를 담고 있는 엔티티를 가리킵니다: 이 경우에는 아보카도입니다. 이것이 스크립트를 재사용 가능하게 만드는 요소입니다. 같은 스크립트를 열 개의 항목에 붙이면, 각 항목이 자기 자신에 대해 동작합니다.

### 첫 상호작용을 바이브 코딩해 보세요

다음 코드는 직접 작성할 수도 있지만, 지금이 바로 다음을 시도해 볼 좋은 순간입니다 **바이브 코딩**: AI 비서에게 원하는 내용을 설명하고 코드를 작성하게 하는 것입니다. 대부분의 코드 편집기에는 내장 기능이 하나씩 있습니다. 예를 들어 Cursor의 채팅, VS Code의 GitHub Copilot, 또는 [Claude Code](https://claude.com/product/claude-code). 또한 Claude Desktop이나 명령줄의 Claude Code처럼 코드 편집기 밖에서도 프롬프트를 작성할 수 있습니다.

첫 프롬프트를 보내기 전에, AI가 SDK의 패턴을 알고 훨씬 적은 실수를 하도록 Decentraland SDK 스킬을 설치하세요:

```bash
npx skills add decentraland/sdk-skills
```

이 스킬은 AI가 SDK로 무언가를 하는 방법을 확신하지 못할 때마다 참고하는 문서이므로, 추측 대신 올바른 코드를 작성합니다. 다음을 참고하세요 [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 설정 옵션과 프롬프트 작성 팁은

이제 다음과 같은 프롬프트를 시도해 보세요:

> AvocadoScript.ts에서 플레이어가 아보카도를 클릭하면 콘솔에 메시지를 기록하도록 하세요.

다음과 비슷한 결과가 나와야 합니다(또는 직접 붙여 넣으세요):

```ts
import { engine, Entity, pointerEventsSystem, InputAction } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER },
			},
			() => {
				console.log('아보카도를 클릭했습니다')
			}
		)
	}

	update(dt: number) {}
}
```

그것은 `start()` 함수 안에 클릭 동작을 추가했습니다. 클릭 동작은 한 번만 정의하면 되고, 플레이어가 항목을 클릭할 때마다 반응합니다. 다음 `pointerEventsSystem.onPointerDown()` 문은 세 가지를 정의합니다:

* 무엇에 `엔티티` 클릭 이벤트를 적용할지: 여기서는 `this.entity`, 스크립트를 담고 있는 아보카도입니다.
* 하나의 `opts` 객체: 사용할 버튼과, 지금은 사용하지 않는 다른 선택적 인수들입니다.
* 엔티티가 클릭될 때마다 실행되는 함수.

기록된 메시지를 보려면 미리보기를 실행하고, 오른쪽 상단 모서리의 ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) 아이콘을 클릭하여 콘솔을 여세요. 또한 **\`** 키를 눌러 토글할 수도 있습니다. 아보카도를 클릭할 때마다 새 줄이 나타납니다:

![](/docs/_remote/609636cba47e8f304bc52855.webp)


> [!WARNING]
> **📔 참고**: 엔티티가 클릭 가능하려면 콜라이더 지오메트리가 있어야 합니다. 여기 사용된 모델에는 이미 포함되어 있습니다. 다음을 참고하세요 [콜라이더](/docs/creator/content-creator-ko/3d/colliders/) 해당하지 않는 모델에 대한 우회 방법은


### 아보카도를 사라지게 만들기

이제 클릭으로 더 흥미로운 일을 해봅시다! AI 비서에게 다음을 요청하세요:

> 아보카도를 클릭하면 사라지게 해주세요. 줄어들면서 탄력 있게 사라졌으면 합니다. 그리고 아보카도를 가리킬 때 보이는 힌트가 "수집"이라고 표시되었으면 좋겠습니다.

다음과 비슷한 결과가 나와야 합니다:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER, hoverText: '수집' },
			},
			() => {
				this.collect()
			}
		)
	}

	collect() {
		Tween.setScale(
			this.entity,
			Vector3.One(),
			Vector3.Zero(),
			500,
			EasingFunction.EF_EASEINBOUNCE
		)
	}

	update(dt: number) {}
}
```

AI가 무엇을 했는지 살펴봅시다.

* 탄력 있는 애니메이션을 만들기 위해, **트윈**를 사용했습니다. 트윈은 엔티티의 위치, 회전 또는 크기가 시간에 따라 점진적으로 변하는 것을 설명합니다. 여기서는 `Tween.setScale()` 가 아보카도를 전체 크기(`Vector3.One()`)에서 아무것도 없는 크기(`Vector3.Zero()`)까지 500밀리초에 걸쳐, 탄력 있는 이징 곡선을 사용해 줄입니다. 트윈에 대해 자세히 알아보려면 [엔티티 이동](/docs/creator/content-creator-ko/sdk7/3d/move-entities/).
* 클릭 함수 안에 트윈을 직접 쓰는 대신, 별도의 `collect()` 메서드를 만들었습니다. 이것이 필수는 아니지만(AI가 트윈을 클릭 함수 안에 넣었다면 그것도 잘 작동합니다), 좋은 관행입니다. 메서드는 코드를 읽기 쉽게 해 주고, 여러 곳에서 같은 로직을 재사용할 수 있게 해 줍니다.
* 아보카도에 "수집" 힌트를 표시하려고, 그것을 `호버 텍스트` 값.

미리보기를 실행하고 아보카도를 클릭하세요: 멋지게 사라질 것입니다.


> [!WARNING]
> **📔 참고**: 아보카도는 크기 0으로 줄어들지만, 엔티티는 여전히 존재합니다. 이상적으로는 트윈이 끝난 뒤 엔티티를 삭제하여 씬을 더 가볍게 유지해야 합니다. 다음을 참고하세요 [트윈 완료 시](/docs/creator/content-creator-ko/sdk7/3d/move-entities/#on-tween-finished).


### 매 프레임마다 코드 실행

지금까지 모든 코드는 `start()`에서 실행되었습니다. 이제 `update()` 를 사용하여 아보카도가 계속 회전하도록 만들어 봅시다. 생성자와 `update()` 을 다음으로 바꾸세요:

```ts
	constructor(
		public src: string,
		public entity: Entity,
		public speed: number = 45,
	) {}

	update(dt: number) {
		const transform = Transform.getMutable(this.entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(this.speed * dt, Vector3.Up())
		)
	}
```

또한 파일 상단의 import에 `Transform` 및 `Quaternion` 를 추가해야 하므로, 다음과 같아야 합니다:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction, Transform } from '@dcl/sdk/ecs'
import { Vector3, Quaternion } from '@dcl/sdk/math'
```


> [!NOTE]
> **💡 팁**: import를 직접 편집하는 대신, 코드 편집기에서 표시한 오류를 클릭하면 자동으로 추가되게 할 수 있습니다.


매 프레임마다 이 아보카도는 조금씩 더 회전합니다. `dt` 로 곱하면 동작이 부드럽고 프레임 속도에 독립적으로 됩니다. 즉, 플레이어의 기계가 얼마나 빠르게 실행되든 아보카도는 초당 45도씩 회전합니다.

왜냐하면 `speed` 는 생성자 매개변수이므로, Creator Hub의 Script 컴포넌트에도 필드로 표시됩니다. Script 컴포넌트 오른쪽 상단의 새로고침 아이콘을 클릭해 확인한 뒤, 코드를 건드리지 않고 값을 조정해 보세요.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="새로고침 버튼" width="360">

각 엔티티는 **Speed** 필드에 대한 고유한 값을 저장하므로, 여러 항목이 같은 스크립트를 공유하면서도 다르게 동작할 수 있습니다. 직접 해 보세요: Ctrl + C와 Ctrl + V로 아보카도를 복사하고, 두 개가 겹치지 않도록 복사본을 옮긴 뒤, 각각 다른 속도를 설정하세요. 이제 각 아보카도는 자기만의 속도로 회전합니다.

스크립트는 다른 스마트 항목이 트리거할 수 있는 동작을 노출하는 등 훨씬 더 많은 일을 할 수 있습니다. 전체 내용을 보려면 [Script 컴포넌트](/docs/creator/content-creator-ko/scene-editor/extend-with-code/script-component/) 를 참고하세요.

## Scene Editor의 항목 참조하기

Script 컴포넌트는 단일 항목에 동작을 부여하는 가장 쉬운 방법이지만, 다른 방법도 있습니다. 다음에 있는 코드는 `index.ts` 시각적으로 추가한 어떤 항목이든 이름으로 가져올 수 있습니다 `engine.getEntityOrNullByName()`. 이는 하나의 로직이 여러 항목을 포함하거나, 모든 것을 한곳에 두고 싶을 때 유용합니다. 다음에 보이는 이름을 사용하세요 [엔티티 트리](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/#the-entity-tree).

이 예에서는 **Yellow Crate**라는 이름의 엔티티를 사용합니다. 어떤 항목이든 사용할 수 있으며, 엔티티 트리에 표시된 이름을 정확히 그대로 적기만 하면 됩니다.


> [!NOTE]
> **💡 팁**: 엔티티 트리에서 마우스 오른쪽 버튼을 클릭하고 **이름 바꾸기** 를 선택하면 엔티티 이름을 바꿀 수 있습니다.


```ts
export function main() {
	// 이전 avocado2 코드
	// (...)

	const crate = engine.getEntityOrNullByName('Yellow Crate')

	if (crate) {
		pointerEventsSystem.onPointerDown(
			{
				entity: crate,
				opts: { button: InputAction.IA_POINTER, hoverText: '열기' },
			},
			function () {
				console.log('상자를 클릭했습니다')
			}
		)
	}
}
```

여기서 `engine.getEntityOrNullByName()` 라는 이름의 엔티티에 대한 참조를 가져옵니다 *Yellow Crate*. `if (crate)` 검사는 해당 이름의 엔티티가 실제로 씬에 존재하는지 확인합니다. 그 이름의 엔티티가 없다면, `crate` 가 `null`로 설정하는 것입니다. 참조: [참조 항목](/docs/creator/content-creator-ko/scene-editor/extend-with-code/reference-items/) 를 참조하세요.


> [!NOTE]
> **💡 팁**: Scene Editor를 통해 추가된 모든 엔티티는 `main()` 가 실행될 때 이미 로드되어 있으므로, 그 안이나 `main()`.


## 더 많은 튜토리얼

읽어보세요 [씬 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/coding-scenes/) Decentraland 씬이 어떻게 작동하는지에 대한 상위 수준의 이해를 위해.

SDK7로 만든 예제를 보려면 [예제 페이지](/docs/creator/content-creator-ko/tutorials-and-examples/examples/)를 확인하세요. 재사용 가능한 씬과 저장소가 들어 있습니다.

다음을 참고하세요 **개발 가이드** 섹션에서 씬에 콘텐츠를 추가하는 방법에 대한 자세한 지침을 확인하세요.

## 다른 개발자들과 소통하기

다음을 방문하세요 [Decentraland Discord](https://dcl.gg/discord) 과 [Decentraland DAO Discord](https://discord.gg/bxHtcMxUs4) 에서 Decentraland Discord의 **크리에이터** 섹션.

문제를 디버그하려면 다음 [문제 해결](/docs/creator/content-creator-ko/sdk7/debugging/troubleshooting/) 및 [디버그](/docs/creator/content-creator-ko/sdk7/debugging/debug-in-preview/) 섹션을 확인하세요. 다음이 설치된 AI 비서는 [SDK 스킬](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/) 훌륭한 디버깅 도우미이기도 합니다. 콘솔의 오류 메시지를 붙여 넣거나, 예상대로 동작하지 않는 내용을 설명하면, 보통 코드의 문제를 찾아낼 수 있습니다. 해결책을 찾지 못했다면 다음에 게시할 수 있습니다 [SDK 지원 카테고리](https://forum.decentraland.org/c/support-sdk/11) Decentraland Forum에서.

## 3D 아트 에셋

좋은 경험에는 그에 어울리는 훌륭한 3D 아트가 필요합니다. 직접 3D 모델을 만들고 싶다면 [3D 모델링 섹션](/docs/creator/content-creator-ko/3d/3d-models/). 하지만 코딩이나 게임 디자인에 더 집중하고 싶다면, 직접 에셋을 만들 필요는 없습니다! 다음을 참고하세요 [유용한 자료](/docs/creator/content-creator-ko/sdk7/getting-started/useful-resources/) 에셋 라이브러리와 AI 도구를 사용해 씬의 3D 모델을 찾을 수 있습니다.

## 씬 게시

Decentraland NAME, ETH ENS 이름, 또는 LAND를 보유하고 있거나, 해당 권한을 가진 사람이 권한을 부여했다면 씬을 Decentraland에 업로드할 수 있습니다. 다음을 참고하세요 [게시](/docs/creator/content-creator-ko/sdk7/publishing/publishing/).

## 기타 유용한 정보

* [AI와 함께하는 바이브 코딩](/docs/creator/content-creator-ko/sdk7/getting-started/vibe-coding/)
* [개발 워크플로](/docs/creator/content-creator-ko/sdk7/getting-started/dev-workflow/)
* [씬 편집](/docs/creator/content-creator-ko/scene-editor/get-started/about-editor/)
* [씬 편집기 필수 요소](/docs/creator/content-creator-ko/scene-editor/get-started/scene-editor-essentials/)
* [게임 설계 제약 사항](/docs/creator/content-creator-ko/sdk7/designing-the-experience/design-games/)
* [3D 모델링](/docs/creator/content-creator-ko/3d/3d-models/)
* [장면 제한](/docs/creator/content-creator-ko/sdk7/optimizing/scene-limitations/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/getting-started/sdk-101.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
