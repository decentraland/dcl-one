> 프로젝트 전체 구조를 파고들 필요 없이 코드 기능을 부여하려면 Script 컴포넌트를 사용하세요.

# 스크립트 컴포넌트 사용

새로운 Script Component를 사용하면 엔티티 자체에서 사용자 지정 코드를 실행하는 엔티티를 만들 수 있습니다.

Script Component를 사용하면 직접 작업할 필요 없이 엔티티의 사용자 지정 동작을 실행할 수 있습니다. `index.ts` 그리고 잠재적으로 다른 파일들도.

## Script Component 설정하기

1. 에서 `+` 버튼을 클릭하고 선택하여 엔티티에 Script Component를 추가합니다. 새 Script를 만들려면 **+ 새 Script 모듈 추가** 를 클릭하고 이름을 선택하거나, 파일 경로를 사용합니다(기존 파일을 찾아보거나 드래그 앤 드롭).

![](/docs/_remote/2c5b0bcd10d2297a8c766d9b.webp)

2. 컴포넌트의 CODE 버튼을 클릭하여 기본 코드 편집기를 엽니다. 구조를 살펴보겠습니다. 기본 편집기를 선택하고 관리하는 방법에 대한 자세한 내용은 다음을 참조하세요. [코드와 결합](/docs/creator/content-creator-ko/scene-editor/extend-with-code/overview/).

## Script 구조 이해하기

Script를 처음 열면 다음 코드가 있습니다:

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class BuildingScript {
  /**
   * 속성
   * 메서드 전반에서 재사용하고 싶은 클래스 필드를 정의합니다.
   * 사용 예: this.myVariable
   */
   // private myVariable: boolean = true

  /**
   * 생성자 / 입력값
   * 여기서 선언한 매개변수는 Creator Hub의 Script Component UI에 표시됩니다.
   * 지원되는 타입: Entity, String, Number, Boolean, ActionCallback, Slider
   * 슬라이더로 편집하는 숫자의 경우, 예: public speed: Slider<0, 10, 0.5> = 1
   *
   * 참고: 이 파일을 편집한 후 Script Component UI에서 새로 고침 아이콘을 클릭하세요.
   * 업데이트된 입력값을 보려면.
   *
   * 생성자에서 `src` 및 `entity` 필드는 내부 참조에 의해 필요합니다.
   */
  constructor(
    public src: string,     // 삭제하지 마세요
    public entity: Entity,   // 삭제하지 마세요
    // 아래에 사용자 지정 입력값을 추가하세요
  ) {}

  /**
   * start()
   * 스크립트가 초기화될 때 한 번 호출됩니다.
   */
  start() {
    // Script 초기화
    console.log("엔티티에 대해 BuildingScript 초기화됨:", this.entity);
  }

  /**
   * update(dt)
   * 매 프레임마다 호출됩니다.
   * @param dt - (선택 사항) 마지막 프레임 이후의 델타 시간(초)
   */
  update(dt: number) {
    // 매 프레임마다 호출됨
  }
}
```

이 클래스는 세 가지 주요 부분으로 구성됩니다:

* 해당 **생성자**,
* the **start()** 메서드
* the **update()** 메서드입니다.

## 생성자

생성자에는 Creator Hub의 씬에서 동적으로 노출하고 수정할 매개변수가 포함됩니다.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public numericVariable: number, 
  ) {}
...
}
```

파일이 저장되면 **새로 고침** 버튼이 Script Component에서 모든 변경 사항을 업데이트합니다.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="새로 고침 버튼" width="360">

새로 고치면 Script Component에 이제 코드에 추가한 `numericVariable` 이 표시됩니다.

![](/docs/_remote/7bf7d0505e8aa1c4870ff35a.webp)

## 매개변수

다른 엔티티가 Script component에서 동일한 파일을 사용하더라도 각자 독립적인 매개변수를 가집니다. 씬에 두 개의 건물이 있고, `building1` 및 `building2`가 있으며 둘 다 `BuildingScript.ts` 파일을 가리키는 Script Component를 가지고 있더라도, 각 건물은 자체적으로 `numericVariable` 독립적으로 수정할 수 있는 매개변수를 가집니다.


> [!WARNING]
> **중요 참고 사항**: 수정/삭제하지 마세요 `public src: string` 및 `public entity: Entity`. 이 항목들 뒤에 새 입력값을 추가할 수 있습니다.


생성자 매개변수에 허용되는 타입은 다음과 같습니다:

* `Entity`
* `문자열`
* `number`
* `불리언`
* `ActionCallback`
* `Slider<Min, Max, Step>`

### 슬라이더가 있는 숫자 매개변수

숫자 매개변수를 다음과 같이 입력하면 `Slider<Min, Max, Step>` 일반 숫자 상자 대신 슬라이더로 편집할 수 있습니다. Creator Hub에는 슬라이더와 숫자 상자가 모두 표시됩니다.

```ts
constructor(
  public src: string,
  public entity: Entity,
  // 0에서 10까지, 0.5 단위로 움직이는 슬라이더
  public speed: Slider<0, 10, 0.5> = 1,
) {}
```

* `Step` 은 선택 사항이며 기본값은 `1`.
* 음수 경계도 허용됩니다. 예를 들어 `Slider<-90, 90>`.
* 실행 시 값은 일반적인 `number`이므로 `this.speed` 는 다른 숫자 매개변수와 동일하게 동작합니다.


> [!NOTE]
> **📔 참고**: 둘 다 `public` 및 `private` 생성자 매개변수는 Creator Hub에 노출됩니다. `private` 키워드는 `BuildingScript` 클래스 내부에서의 접근만 제한합니다. 자세한 내용은 TypeScript 공식 문서의\
> [매개변수 속성](https://www.typescriptlang.org/docs/handbook/2/classes.html#parameter-properties).


### Script 내부에서 매개변수에 접근하기

코드에서 매개변수 값을 접근하려면 다음 표기법을 사용하세요. `this.definedParameter`. 예를 들어, `this.numericVariable` 또는 `this.entity`.

기본 Script 템플릿에는 start() 메서드에 다음 줄이 포함됩니다:

`console.log("엔티티에 대해 BuildingScript 초기화됨:", this.entity);`.

생성자에서 정의한 값의 값을 로그로 출력하려면 다음과 같이 변경하세요:

`console.log("numericVariable과 함께 BuildingScript 초기화됨:`, `this.numericVariable);`

Creator Hub UI에서 매개변수 값을 변경하면 로그에 표시되는 값도 그에 맞게 반영되는 것을 볼 수 있습니다.

### 기본 매개변수

생성자에는 기본적으로 `src` 그리고 `엔티티` 매개변수가 포함되며, 이는 스크립트 코드에 매우 유용합니다:

* `this.entity` 항상 `Script` 컴포넌트를 포함하는 엔티티를 가리키며, 이를 사용해 엔티티에 대한 정보를 가져오거나 컴포넌트를 추가할 수 있습니다.
* `this.src` 는 스크립트가 저장된 경로입니다. 이는 다른 사람들이 사용하도록 만든 Smart Item을 생성할 때 특히 유용합니다. 스마트 아이템의 경로가 변경되거나 이름이 바뀌더라도, 이 필드를 사용해 스마트 아이템과 함께 패키징된 파일의 경로를 구성하세요.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
  ) {}

  start() {
    Material.setPbrMaterial(this.entity, {
      texture: Material.Texture.Common({
        src: this.src + '/images/myImage.png',
      })
    });
  }
}
```

위 스크립트는 스크립트를 소유한 엔티티를 가져와 텍스처를 적용합니다. 이 텍스처는 스마트 아이템 폴더 안의 `.png` 파일에서 가져오며, `/images`라는 하위 폴더에 있습니다.  `this.src`를 사용하면 스마트 아이템이 씬으로 `/assets/custom/itemName` 또는 `/assets/asset-packs/itemName`

### 매개변수에 대한 툴팁

입력 매개변수에 툴팁을 추가하여 사용자가 이 필드가 무엇에 사용되는지, 또는 어떤 값이 허용되는지 알 수 있게 하세요. 사용자는 Script component UI의 각 필드 옆에 있는 툴팁 아이콘을 보게 되며, 아이콘 위에 마우스를 올리면 사용자 지정 텍스트를 읽을 수 있습니다.

생성자에 툴팁을 추가하려면 생성자 바로 앞에 주석 처리된 블록을 추가하고, 각 툴팁마다 `@param` 과 필드 이름을 붙인 뒤 설명을 적으세요.

```ts
  /**
   * @param startDate - 이벤트 시작 날짜(YYYY-MM-DD 형식)
   * @param yOffset - 아이템을 지면 위 몇 미터에 표시할지
   */
  constructor(
    public src: string,
    public entity: Entity,
    public startDate?: string,
    public yOffset: number = 0.5,
  ) {
  }
```

툴팁의 변경 사항을 보려면 Script component UI에서 새로 고침 아이콘을 클릭해야 할 수도 있습니다.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="새로 고침 버튼" width="360">

## start() & update() 메서드

해당 **start()** 메서드에는 엔티티가 생성될 때(이 경우 씬이 처음 로드될 때) 한 번만 실행되는 코드가 들어 있습니다.

씬을 미리 보고 로그를 확인하세요(**팁**: `` ` `` 바로가기 키를 사용할 수 있습니다): `numericVariable` 매개변수.

![](/docs/_remote/3391665f06d28b07d1c783f1.webp)

해당 **update()** 메시지에는 `PlayerEntity` 가 포함되어 있으며, 반면 이 메서드는 게임의 매 프레임마다 코드가 실행됩니다(시스템처럼). 예를 들어,

다음 코드는 게임의 매 프레임마다 로그를 출력합니다. `PlayerEntity` 가 이전에 정의한 `numericVariable`보다 큰 경우, 스크립트에서 동작을 트리거하기 위해 Creator Hub UI에서 작성자가 동적으로 제공하는 것입니다.

```ts
update(dt: number) {
    if (Transform.get(engine.PlayerEntity).position.y > this.numericVariable ) {
      console.log("플레이어의 높이가 ", this.numericVariable, "보다 큽니다.");
    }}
```

<img src="/docs/_remote/cff6c35c55f5fdd39baaefae.webp" alt="Update Method" data-size="line">

첫 번째 로그는 start() 메서드에 속하며, numericVariable을 설정했다는 것을 나타냅니다. 두 번째 로그는 update() 메서드에 속하며, 플레이어가 그 값보다 높을 때 표시됩니다.

## Creator Hub에 작업을 노출하기

Script Component 스크립트 안에 `액션` 를 정의하고 Creator Hub UI에서 접근 가능하게 만들 수 있습니다. 이렇게 하면 이 `액션` 를 다른 엔티티와 함께 트리거할 수 있습니다.

```ts
  /**
   * 트리거되도록 이 작업을 노출
   * @action
   */
  exposedAction(creatorHubParameter: number) {
    console.log("다른 엔티티에서 매개변수로 트리거됨: ", this.creatorHubParameter);
  }
```

`creatorHubParameter` 는 `액션` 매개변수로 노출되어 사용자 지정 값을 줄 수 있습니다. Script Component를 새로 고친 후에는 새 작업이 Actions 드롭다운에서 옵션으로 제공됩니다.

![](/docs/_remote/eb92dee98d90a44d44de70b3.webp)

작업을 추가한 후에는 Creator Hub의 어떤 엔티티든 `트리거`

![](/docs/_remote/e8bcd93f2829f1275473cb37.webp)


> [!NOTE]
> **📔 참고**: 필요한 만큼 Script 안에 여러 작업을 추가할 수 있습니다. 이들은 모두 `액션` 드롭다운과는 독립적으로 접근할 수 있습니다.


## 외부에서 Script 메서드 호출하기

다른 Script 또는 `src/index.ts`에서 Script 메서드를 호출하려면 다음 단계를 따라야 합니다:

1. 만드세요 `public` 메서드를 Script 클래스 안에 둡니다.
2. 실행하세요 `npm run build` 씬의 루트 디렉터리에서.
3. 공개 메서드를 사용하려는 파일에서 다음을 추가하세요. `import { callScriptMethod } from '~sdk/script-utils'`.
4. 호출하세요 `callScriptMethod` 필요한 매개변수와 함께(이 경우, `someParamter`).

다음은 `public` 노출된 메서드의 예입니다

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    ...,
  ) {}

  public publicMethod(boolParameter: boolean, someNumberParameter: number) {
    if (boolParameter) {
      console.log("true 매개변수로 public 메서드가 호출됨!: ", someNumberParameter);
    } else {
      console.log("false 매개변수로 public 메서드가 호출됨!", someNumberParameter);
    }
  }
...
}
```

이를 호출하려면 `src/index.ts`다음을 사용하세요:

```ts
import { callScriptMethod } from '~sdk/script-utils'


export function main() {
    const buildingEntity = engine.getEntityOrNullByName("building")
    if (buildingEntity) {
        const scriptMethod = callScriptMethod(
            buildingEntity,
            "assets/scene/Scripts/BuildingScript.tsx",
            "publicMethod",
            false,
            3,
        )

        scriptMethod
    }
}
```

먼저 `main` 함수는 `Entity` 를 찾습니다. Script 컴포넌트를 가진 `Entity` 가 존재하면, `callScriptMethod` 는 다음 매개변수와 함께 호출됩니다:

1. `엔티티`: `Entity` 안으로 들어가고 `public` 메서드입니다.
2. `scriptPath`: `경로` 해당 `Script` 클래스가 있는 곳.
3. `methodName`: 호출할 `public` 메서드의 이름.
4. `...args`: 메서드의 인자들. 이 경우 두 개가 있으며, 순서대로 하나씩 추가해야 합니다.

셋째, 정의된 `callScriptMethod`을 호출합니다. 이 경우 `scriptMethod`.

제공된 매개변수 값으로 출력은 다음과 같습니다:

![](/docs/_remote/732f383d31c15cab43ea285e.webp)


> [!NOTE]
> **📔 참고**: 동일한 논리로 다른 Script나 파일에서 `public` Script 메서드를 호출할 수 있습니다. 이를 사용해 `public` Script 클래스의 변수에서 값을 가져오거나 변경할 수 있습니다.


## Script에서 다른 엔티티의 액션 트리거하기

타입의 매개변수를 사용하는 것이 가능합니다. `ActionCallback` Script 클래스 생성자에서. 이를 통해 다른 `Entity`의 `액션` Creator Hub UI를 통해 정의된 것을 Script의 메서드에서 트리거할 수 있습니다.

이 예에서는 `anotherEntityAction` 이 `public` 매개변수.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public anotherEntityAction: ActionCallback,
    ...,
  ) {}
  ...
}
```

선택 가능한 `Entity` 및 `액션` 는 이제 Creator Hub UI에서 Script Component를 새로 고치면 사용할 수 있습니다. `스피어` 는 씬에 이미 존재하는 엔티티이며, `크기 조정`.

![](/docs/_remote/b12b866e818c2c2562ab037c.webp)

라는 작업을 가지고 있습니다. 다른 엔티티의 작업은 이제 Script 클래스에서 접근할 수 있습니다. 이는 여러 가지 다른 방식으로 사용할 수 있습니다. 다음 예에서는 E를 누르면 `this.anotherEntityAction` 를 `pointerEventsSystem` 을 `시작` 메서드입니다.

```ts
  start() {
    pointerEventsSystem.onPointerDown(
      {
        entity: this.entity,
        opts: {
          button: InputAction.IA_PRIMARY,
          hoverText: "다른 엔티티의 작업을 트리거하려면 E를 누르세요.",
        },
      },
      () => {
        this.anotherEntityAction();
      }
    );
  }
```

### 선택적 액션 콜백

하나의 `ActionCallback` 작성자가 연결하지 않은 매개변수는 `undefined`이며, 해당 타입은 `ActionCallback | undefined`입니다. 호출하기 전에 확인하여 액션이 연결되어 있든 없든 스크립트가 동작하게 하세요:

```ts
if (this.anotherEntityAction) {
  this.anotherEntityAction();
}
```


> [!NOTE]
> **📔 참고**: 노출과 트리거 결합하기 `동작` 는 매우 강력한 도구입니다. 한 엔티티에 Script Component를 정의하고 `public` 메서드를 사용해 작업을 노출한 다음, 다른 엔티티의 Script Component에서 `ActionCallback` 매개변수.


## 또한 참조

* [스마트 항목 - 기본](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items/)
* [스마트 아이템 - 고급](/docs/creator/content-creator-ko/scene-editor/interactivity/smart-items-advanced/)
* [상태와 조건](/docs/creator/content-creator-ko/scene-editor/interactivity/states-and-conditions/)
* [어떤 항목이든 스마트 항목으로 만들기](/docs/creator/content-creator-ko/scene-editor/interactivity/make-any-item-smart/)
* [SDK 빠른 시작](/docs/creator/content-creator-ko/sdk7/getting-started/sdk-101/): 빠른 입문 과정을 위해 이 미니 튜토리얼을 따라하세요.
* [개발 워크플로](/docs/creator/content-creator-ko/sdk7/getting-started/dev-workflow/): 씬 생성 과정을 처음부터 끝까지 이해하려면 이것을 읽어보세요.
* [예제](/docs/creator/content-creator-ko/tutorials-and-examples/examples/): 작업 예제 씬으로 바로 들어가 보세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/scene-editor/extend-with-code/script-component.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
