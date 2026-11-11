# 테스트 작성

테스트는 다양한 방식으로 작성할 수 있습니다. 이 섹션에서는 우리가 테스트를 작성하는 방식의 표준과 그 배경이 되는 이유를 제시하려고 합니다.

테스트는 다음을 사용하여 작성해야 합니다. **describe** 그리고 **it** jest가 제공하는 메서드

## 컨텍스트 설명 및 구성

이 **describe** 메서드는 가능한 경우 테스트할 코드 조각이 실행될 컨텍스트를 설명하는 데 사용해야 하며, 그 컨텍스트를 자신의 범위 안에 만들어야 합니다.

```tsx
describe('flag가 빨간색일 때', () => {
	...
})

describe('flag가 파란색일 때', () => {
	...
})
```

컨텍스트를 설명하기 위해 다음 단어 중 하나를 사용해야 합니다: **when** 최상위 describe에서는 그리고 **having** 또는 **및** 이전에 정의된 다른 컨텍스트에 의존하는 컨텍스트가 있음을 나타내기 위해 사용합니다. 이렇게 하는 이유는 describe 문장을 조합하여 쉽게 이해하고 확인할 수 있는 자기 설명적 컨텍스트를 갖기 위함입니다. 컨텍스트 설명을 연결하는 것이 목적이므로, **describes** 소문자로 작성해야 합니다.

```tsx
describe('flag가 빨간색일 때', () => {
	...
	describe('그리고 바람이 강할 때', () => {
     // 여기서 컨텍스트는 describe들의 결합으로 설명됩니다:
     // "flag가 빨간색이고 바람이 강할 때"
     ...
  })
})
```

여러 describe는 같은 수준과 더 낮은 수준에서 중첩될 수 있습니다. 같은 수준의 describe는 서로 다른 컨텍스트를 나타내고, 더 낮은 수준의 describe(중첩된 describe)는 더 구체적인 컨텍스트를 나타냅니다.

```tsx
describe('flag가 빨간색일 때', () => {
	...
	describe('그리고 바람이 강할 때', () => {
     ...
  })

	describe('그리고 바람이 약할 때', () => {
     ...
  })
})
```

앞서 언급했듯이, 각 describe에는 컨텍스트를 만드는 메서드 또는 컨텍스트를 파괴하거나 변경하는 메서드가 함께 있어야 합니다. 반드시 사용해야 하는 메서드는 다음과 같습니다. **beforeEach** 그리고 **afterEach**.


> [!WARNING]
> Jest 프레임워크에는 두 가지 메서드가 있습니다, **beforeEach** 및 **beforeAll**하지만 둘을 함께 사용하면 중첩된 컨텍스트가 있을 때 혼란과 실행 문제가 발생합니다. [실행되는 순서](https://jestjs.io/docs/setup-teardown#scoping) 는 우리가 예상하는 것과 다릅니다.


다음으로 생성된 컨텍스트는 **beforeEach** 메서드는 describe 트리로 더 깊이 들어갈수록 점점 더 구체적으로 정의됩니다. 각 **beforeEach** 는 해당 텍스트가 말하는 바에 따라 컨텍스트를 더 구체적으로 지정합니다.

### 변수 범위 지정

이 **변수** 컨텍스트에서 설정된 값은 사용될 범위에 따라 정의해야 합니다. 즉, 중첩된 describe 수준에서 특정 컨텍스트에만 변수가 사용된다면, 그 변수는 해당 특정 컨텍스트에만 정의해야 합니다. 이렇게 하는 이유는 변수를 사용되는 코드 가까이에 두어 코드를 더 쉽게 이해할 수 있게 하기 위함입니다.

```tsx
let flag = 'blue'
describe('flag가 빨간색일 때', () => {
	let wind = 'mild'
	beforeEach(() => {
    flag = 'red'
  })
  ...

	describe('그리고 바람이 강할 때', () => {
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('그리고 바람이 약할 때', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
```

의 변수는 **원시 타입**, **변경되지 않을** 컨텍스트에서 정의되거나 이후 컨텍스트에서 정의되지 않을 경우 다음과 같이 정의해야 합니다. **const** 한 번, **beforeEach** 범위 내의 정의보다 우선합니다.

의 변수는 **원시 타입** 컨텍스트에서 변경되거나 이후 컨텍스트에서 변경될 변수는 반드시 다음과 같이 정의해야 합니다. **let**, 이는 TypeScript의 특성상 그렇게 해야 하기 때문입니다.

의 변수는 **비원시 타입** (객체, 배열 등)는 반드시 다음과 같이 정의해야 합니다. **let** 그리고 그 값은 다음에서 설정해야 합니다. **beforeEach** 그 이유는 JS의 객체가 변경 가능하기 때문이며, 즉 해당 객체를 사용하는 코드의 모든 실행은 변경될 수 있고 의도치 않게 다른 테스트에 영향을 줄 수 있습니다.

```tsx
let flag
describe('flag가 빨간색일 때', () => {
  // 여러 범위에서 수정될 원시 값입니다.
	let wind
  // 테스트 코드에서 사용될 객체 값입니다.
  let someObject
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('그리고 바람이 강할 때', () => {
		// 이 범위에서만 사용될 상수 원시 값입니다.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('그리고 바람이 약할 때', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

변수는 가능한 경우 올바르게 타입이 지정되어야 합니다. 반복되는 타입은 타입으로 추상화해야 합니다.

```tsx
let flag: string
describe('flag가 빨간색일 때', () => {
  // 여러 범위에서 수정될 원시 값입니다.
	let wind: string
  // 테스트 코드에서 사용될 객체 값입니다.
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('그리고 바람이 강할 때', () => {
		// 이 범위에서만 사용될 상수 원시 값입니다.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('그리고 바람이 약할 때', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

## 기대 사항 설명 및 코드 실행

이 **it** 메서드는 항상 다음의 범위 안에 배치되어야 합니다. **describe** 그리고 우리가 테스트하려는 코드에서 무엇을 기대하는지를 설명하는 데 사용해야 하며, 가능한 경우 오직 **하나의** 어설션만 포함해야 합니다. 여러 어설션이 각 **it** 에 대해 존재할 수 있습니다. 이는 성능 문제가 있을 경우인데, 테스트가 각각의 **it** 테스트가 **beforeEach**에 대해 한 번씩 실행되기 때문입니다. 또는 기대되는 바를 기대 설명에서 명확하게 설명할 수 있는 경우입니다.

```tsx
let flag: string
describe('flag가 빨간색일 때', () => {
	let wind: string
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1, swim: jest.fn() }
  })
  ...

  // 컨텍스트에서 테스트 실행에 대한 단일 기대(it 하나)
	describe('그리고 바람이 강할 때', () => {
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    
		it('수영하러 가지 않아야 한다', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(false)
    })
  })

  // 컨텍스트에서 테스트 실행에 대한 여러 기대(it 두 개)
	describe('그리고 바람이 약할 때', () => {
    let result: boolean
		beforeEach(() => {
	    wind = 'weak'
      result = goSwimming(flag, wind, someObject)
	  })

		it('수영하러 가야 한다', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(true)
    })

    it('swim 메서드가 호출되었어야 한다', () => {
      expect(someObject.swim).toHaveBeenCalled()
    })
  })
})
...
```

이 구조의 배경은 각 컨텍스트와 기대 사항이 명확하게 번호로 나뉘어 있어 무엇을 테스트하는지, 어떻게 테스트하는지, 그리고 무엇이 아직 테스트되지 않았는지를 더 쉽게 이해할 수 있게 하여 테스트를 검토하는 사람과 코드를 유지보수하고 변경하는 개발자에게 명확성을 제공하기 위함입니다.

다음에 따라 **describe** 설명을 **it**내려가면 개발자들은 문장을 연결하여 무엇이 테스트되고 있는지 쉽게 이해할 수 있습니다.

```tsx
// 이것은 다음과 같이 읽습니다: "flag가 빨간색이고 바람이 강할 때, 
// 수영하러 가야 한다"
describe('flag가 빨간색일 때', () => {
  ...
  describe('그리고 바람이 강할 때', () => {
		...
		it('수영하러 가야 한다', () => {
			...
		})
  })
})
```

### 명확한 기대 사항 작성

다음에 작성된 기대 사항의 설명은 **it**가능한 한 테스트 실행에서 무엇이 기대되는지에 대해 구체적이어야 합니다. 개발자는 기대 사항을 정의할 때 추상적이거나 일반적인 표현을 사용해서는 안 되는데, 이는 코드에서 무엇이 기대되는지의 명확성을 떨어뜨리기 때문입니다.


> [!CAUTION]
> **개발자는 다음과 같은 표현을 사용해서는 안 됩니다:**
> 
> * "기대한 대로 동작해야 한다" ⇒ 무엇이 기대한 대로 동작해야 합니까?
> * "올바른 값을 반환해야 한다" ⇒ 올바른 값은 무엇입니까?
> * "정상적으로 resolve/return/work해야 한다" ⇒ 무엇이 어떻게 정상적으로 동작합니까?
> * "실패해야 한다" ⇒ 어떻게 실패해야 합니까? 어떤 메시지를 제공해야 합니까?


다음의 내용을 지정할 때는 반드시 고려해야 합니다. **it**s 또는 컨텍스트가 다음에 어떤 모습일지 **describe**s에서 개발자는 의도를 더 잘 이해하는 데 필요하다면 함수 이름이나 정확한 오류 메시지를 사용할 수 있지만, 테스트를 더 쉽게 유지보수할 수 있도록 가능한 경우 텍스트 형태의 표현을 사용해야 합니다.

```tsx
// math.ts
export function div(a: number, b: number): number {
	if(b === 0)	{
		throw new Error('분할수 b가 0입니다. 나눗셈을 수행할 수 없습니다')
	}
}

// test.spec.ts
import { div } from './math.ts'

describe('0으로 나눌 때', () => {
  // it의 설명은 예외 메시지의 의도를 설명합니다
	it('0으로 나눌 수 없다는 예외를 던져야 한다', () => {
		expect(() => div(12, 0)).toThrowError('분할수 b가 0입니다. 나눗셈을 수행할 수 없습니다')
	})
})
```

## 무엇을 테스트할 것인가

무엇을 테스트할지 선택하는 것은 실행되는 코드에 따라 달라질 수 있습니다. 성능이나 큰 입력 도메인과 같은 다양한 요인은 테스트에서 무엇을 테스트해야 하거나 하지 말아야 하는지를 확실히 바꿀 수 있습니다. 여기서는 개발자가 가능한 경우 테스트해야 하는 단일 사례 집합을 제시합니다.

```tsx
function run(kilometers: number): number {
	if(kilometers > 1000) {
    throw new Error('러너는 1000킬로미터를 초과해 달릴 수 없습니다')
  }

  if(kilometers <= 10) {
		return kilometers
  } else if(kilometers > 10) {
		doSomething(kilometers)
    return beLazy(kilometers)
  }
}
```

여기서 run 함수는 몇 가지 서로 다른 실행 흐름을 가지고 있습니다. 함수나 코드의 섹션은 가능한 실행 흐름의 차이만을 기준으로 테스트해서는 안 되며, 함수에서 기대되는 바를 기준으로 테스트해야 합니다. 함수가 원래 의도한 대로 동작하지 않을 수도 있고, 테스트가 코드에 너무 강하게 결합되어 다른 문제를 잡아내지 못할 수 있기 때문입니다. 이는 개발자가 항상 가능한 모든 실행 경로를 테스트해야 하며, 함수의 의미에 따라 다른 가능한 경로도 테스트해야 함을 의미합니다.

이 특정 경우에는 개발자가 **적어도** 다음 경우를 테스트해야 합니다:

1. 1000킬로미터를 초과한 거리로 run 함수를 실행
2. 10킬로미터와 같은 거리로 run 함수를 실행
3. 10킬로미터보다 크고 1000킬로미터보다 작은 거리로 run 함수를 실행

첫 번째 경우에는 run 함수가 예외를 던지는지 테스트해야 합니다. **예외는 오류 메시지를 확인해야 합니다**, 다른 예외가 던져질 수도 있어 테스트의 목적을 무효화할 수 있기 때문입니다. 예외가 사용자 정의 예외라면, 개발자는 예외 인스턴스를 확인할 수도 있습니다.

두 번째 경우에는 함수가 전달받은 것과 같은 거리 수를 반환하는지 테스트해야 합니다.

세 번째 경우에는 함수가 외부 함수인 **beLazy** 가 반환한 값을 반환하는지, 그리고 **doSomething** (메서드에서도 실행됨)는 함수의 반환값으로는 확인할 수 없으므로, 개발자는 **올바른 인수로 호출되었는지 테스트해야 합니다**.

```tsx
import { doSomething, beLazy } from '../runningUtils'
jest.mock('../runningUtils')

// main describe가 없다는 점에 유의하세요
// 전역 컨텍스트는 jest로 목킹하고자 하는 함수들뿐입니다

const mockDoSomething = doSomething as jest.MockedFunction<typeof doSomething>
const mockBeLazy = beLazy as jest.MockedFunction<typeof beLazy>

describe('1000킬로미터보다 더 많이 달릴 때', () => {
	it('러너가 1000킬로미터보다 더 많이 달릴 수 없다는 오류를 던져야 한다', () => {
		expect(() => run(1001)).toThrowError("러너는 1000킬로미터를 초과해 달릴 수 없습니다")
	})
})

describe('10 이하로 달릴 때', () => {
	it('주어진 것과 같은 거리 수를 반환해야 한다', () => {
		expect(run(2)).toEqual(2)
	})
})

describe('10킬로미터보다 많이, 하지만 1000킬로미터보다 적게 달릴 때', () => {
	const kilometers = 50
	let result: number
	beforeEach(() => {
		mockDoSomething.mockReturnValueOnce(undefined)
		mockBeLazy.mockImplementationOnce(value => value)
		result = run(kilometers)
	})
	
	it('게으르게 된 후 킬로미터를 반환해야 한다', () => {
		expect(result).toEqual(kilometers)
	})

	it('주어진 킬로미터와 함께 doSomething 함수를 호출해야 한다', () => {
		expect(mockDoSomething).toHaveBeenCalledWith(kilometers)
	})
})
```

## 목킹을 언제 할 것인가

목킹은 주로 개발자가 작성하는 테스트의 유형에 따라 달라집니다.

* 단위 테스트는 문자열 포맷팅처럼 간단한 일을 하는 함수들을 제외하고 모든 외부 함수를 목킹해야 합니다.
* API 테스트는 외부 서비스와 통신하는 목킹만 가져야 합니다. 즉, DB나 다른 API입니다. 테스트에서 수행되는 작업 중 하나가 테스트 스위트의 성능에 영향을 준다면, 이 문제를 완화하기 위해 목을 구현할 수 있습니다.

모든 목은 반드시 Jest가 제공하는 도구를 사용해 만들어야 합니다. 단, 다음은 예외입니다. `redux-saga-test-plan` 목

### 목과 유틸리티 함수

* 모든 Jest 목은 가능한 경우 그들의 **once** 메서드를 사용해 목킹해야 합니다. 즉, `mockReturnValueOnce` 또는 `mockResolvedValueOnce` 는 권장되지 않습니다. `mockReturnValue` 또는 `mockResolvedValue`이렇게 하는 이유는 원치 않는 목이 실행되어 테스트의 실행을 바꾸는 것을 방지하기 위함입니다.
* Jest는 다양한 종류의 목에 사용할 수 있는 여러 타입을 제공합니다. 예를 들어 객체를 목킹할 때는 `jest.Mocked<typeof someObject>`를 사용해야 하고, 클래스의 경우에는 `jest.MockedClass<typeof SomeClass>`를 사용해야 하며, 함수의 경우에는 `jest.MockedFunction<typeof someFunction>`.
* 하나의 `afterEach` 는 다음과 함께 각 테스트 후에 실행되도록 작성해야 합니다. `jest.resetAllMocks` 전역적으로 목킹된 모듈의 목 구현이 다른 테스트로 새어 나가는 것을 지우기 위해서입니다. 만약 `mockReturnValueOnce` 또는 `mockResolvedValueOnce` 를 사용한다면 이 리셋은 무시해도 됩니다.
* 라는 이름의 디렉터리를 `mocks` 목을 저장하기 위해 만들 수 있으며, 이는 `test` 또는 `spec` 디렉터리에 두어야 합니다. 큰 목은 테스트 파일이 지나치게 길어지는 것을 피하기 위해 테스트와 다른 파일에 저장해야 합니다. 작은 목은 많지 않다면 테스트 안에 두는 것이 좋습니다.
  * 목 파일은 목킹하는 엔터티의 이름으로 지정해야 합니다. 예를 들어 프로필을 목킹한다면, 목은 `/test/mocks/profile.ts` 파일에 두어야 합니다.
  * 여러 개의 큰 목이 필요한 경우에는, 목킹할 엔터티 이름의 디렉터리 안에 서로 다른 파일로 두어야 합니다. 예를 들어 두 개의 프로필 목이 있다면 다음과 같이 저장합니다: `/test/mocks/profile/profile-with-wearables.ts` 및 `/test/mocks/profile/profile-without-wearables.ts`. 쉽게 접근할 수 있도록 다음을 만들어야 합니다. `index.ts` 파일을 `/test/mocks/profile/` 디렉터리 안에 만들고 export해야 합니다.
  * 목킹된 객체의 변이를 방지하기 위해, 목은 **함수로 export해야 하며** 이를 반환해야 합니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
