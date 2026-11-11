# 编写测试

测试可以用多种方式编写。本节试图给出一种标准，以及我们编写测试的方式背后的理由。

测试必须使用 **describe** 以及 **其** jest 提供的方法

## 描述和构建上下文

该 **describe** 应当使用该方法来描述，在可能的情况下，待测试代码片段将要执行的上下文，并且应在其作用域内构建该上下文。

```tsx
describe('当旗帜是红色时', () => {
	...
})

describe('当旗帜是蓝色时', () => {
	...
})
```

为了描述上下文，我们应使用以下词语之一： **当……时** 用于顶层 describe，并且 **在……具有** 或 **和** 用来表示存在一个依赖于先前已定义上下文的上下文。这样做的理由是，让上下文自描述，并且可以通过组合 describe 语句轻松理解和检查。由于这个思路是把上下文描述串联起来， **describe** 必须使用小写。

```tsx
describe('当旗帜是红色时', () => {
	...
	describe('并且风很大', () => {
     // 这里的上下文由这些 describe 的合取来描述：
     // “当红旗是红色且风很大时”
     ...
  })
})
```

多个 describe 可以在同一级别以及更低级别进行嵌套。同一级别的 describe 表示不同的上下文，而更低级别的 describe（嵌套的 describe）表示更具体的上下文。

```tsx
describe('当旗帜是红色时', () => {
	...
	describe('并且风很大', () => {
     ...
  })

	describe('并且风很弱', () => {
     ...
  })
})
```

如前所述，每个 describe 都应配有一个构建上下文的方法，或者一个销毁或更改上下文的方法。必须使用的方法是 **beforeEach** 以及 **afterEach**.


> [!WARNING]
> Jest 框架有两个方法， **beforeEach** 和 **beforeAll**，但当存在嵌套上下文时，同时使用二者会产生混淆和执行问题，因为 [它们执行的顺序](https://jestjs.io/docs/setup-teardown#scoping) 并不是我们所期望的。


使用以下方法创建的上下文 **beforeEach** 方法创建的上下文会随着我们深入 describe 树而变得越来越具体。每个 **beforeEach** 都会根据其文本所述进一步指定上下文。

### 变量作用域

该 **变量** 在上下文中设置的变量应根据它们将被使用的作用域来定义。也就是说，如果某个变量只用于嵌套 describe 层级中的特定上下文，那么该变量应只在该特定上下文中定义。这样做的理由是让变量尽量靠近使用它们的代码，使代码更容易理解。

```tsx
let flag = '蓝色'
describe('当旗帜是红色时', () => {
	let wind = '微风'
	beforeEach(() => {
    flag = '红色'
  })
  ...

	describe('并且风很大', () => {
		beforeEach(() => {
	    wind = '强风'
	  })
    ...
  })

	describe('并且风很弱', () => {
		beforeEach(() => {
	    wind = '弱风'
	  })
    ...
  })
})
```

原始类型的变量 **原始类型**，且 **不会被更改的** 在其定义的上下文或后续上下文中应定义为 **const** 一次，优先于 **beforeEach** 在作用域中的定义。

原始类型的变量 **原始类型** 在其定义的上下文或后续上下文中会被更改的，必须定义为 **let**，因为 TypeScript 的特性要求如此。

原始类型的变量 **非原始类型** （对象、数组等）必须定义为 **let** 并且它们的值必须在一个 **beforeEach 中设置。** 这样做的理由是，JS 中的对象是可变的，这意味着任何使用该对象的代码执行都可能发生变化，并且会在非自愿的情况下影响其他测试。

```tsx
let flag
describe('当旗帜是红色时', () => {
  // 一个将在多个作用域中被修改的原始值。
	let wind
  // 一个将在测试代码中使用的对象值。
  let someObject
	beforeEach(() => {
    flag = '红色'
		someObject = { id: 1 }
  })
  ...

	describe('并且风很大', () => {
		// 一个只会在此作用域中使用的常量原始值。
		const aConstantPrimitiveValue = '某个值'
		beforeEach(() => {
	    wind = '强风'
	  })
    ...
  })

	describe('并且风很弱', () => {
		beforeEach(() => {
	    wind = '弱风'
	  })
    ...
  })
})
...
```

变量应尽可能正确地标注类型。重复出现的类型应抽象成一个类型。

```tsx
let flag: string
describe('当旗帜是红色时', () => {
  // 一个将在多个作用域中被修改的原始值。
	let wind: string
  // 一个将在测试代码中使用的对象值。
  let someObject: { id: string }
	beforeEach(() => {
    flag = '红色'
		someObject = { id: 1 }
  })
  ...

	describe('并且风很大', () => {
		// 一个只会在此作用域中使用的常量原始值。
		const aConstantPrimitiveValue = '某个值'
		beforeEach(() => {
	    wind = '强风'
	  })
    ...
  })

	describe('并且风很弱', () => {
		beforeEach(() => {
	    wind = '弱风'
	  })
    ...
  })
})
...
```

## 描述期望并执行代码

该 **其** 方法必须始终放在一个 **describe** 并且必须用于描述我们对将要测试的代码的期望，并且如果可能，应只包含 **一条** 断言。每个断言的多个 **其** 如果存在性能问题，则可以存在多个，因为测试将按每个 **其** （因为 **beforeEach**），或者如果期望内容可以在期望描述中清晰地表达出来。

```tsx
let flag: string
describe('当旗帜是红色时', () => {
	let wind: string
  let someObject: { id: string }
	beforeEach(() => {
    flag = '红色'
		someObject = { id: 1, swim: jest.fn() }
  })
  ...

  // 对一个上下文中的测试执行的单个期望（一个 it）
	describe('并且风很大', () => {
		const aConstantPrimitiveValue = '某个值'
		beforeEach(() => {
	    wind = '强风'
	  })
    
		it('不应该去游泳', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(false)
    })
  })

  // 对一个上下文中的测试执行的多个期望（两个 it）
	describe('并且风很弱', () => {
    let result: boolean
		beforeEach(() => {
	    wind = '弱风'
      result = goSwimming(flag, wind, someObject)
	  })

		it('应该去游泳', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(true)
    })

    it('应该已经调用了 swim 方法', () => {
      expect(someObject.swim).toHaveBeenCalled()
    })
  })
})
...
```

这样做的理由是为了让测试的审阅者以及后续维护和修改代码的开发者更清楚地理解，因为每个上下文和期望都被清晰地枚举出来，从而更容易理解正在测试什么、如何测试，以及还剩下什么需要测试。

按照 **describe** 描述一直往下到 **其**，开发者就能通过拼接这些句子轻松理解正在测试什么。

```tsx
// 这可以读作：“当旗帜是红色且风很大时， 
// 它应该去游泳”
describe('当旗帜是红色时', () => {
  ...
  describe('并且风很大', () => {
		...
		it('应该去游泳', () => {
			...
		})
  })
})
```

### 编写清晰的期望

写在 **其**it 中的期望描述必须尽可能详细地说明测试执行所期望的结果。开发者绝不能使用抽象或笼统的措辞来定义期望，因为这会削弱对代码期望内容的清晰度。


> [!CAUTION]
> **开发者绝不能使用如下措辞：**
> 
> * “应按预期工作” ⇒ 什么叫按预期工作？
> * “应返回正确值” ⇒ 什么是正确值？
> * “应正确地 resolve/返回/工作” ⇒ 某事如何才算正确地工作？
> * “应失败” ⇒ 它应如何失败？它应提供什么消息？


必须考虑到，在指定 **其**it 中的期望或在以下内容中的上下文时 **describe**，开发者可以在有助于更好理解意图时使用函数名或精确的错误消息，但如果可能，应使用文本表述，以便让测试更易于维护。

```tsx
// math.ts
export function div(a: number, b: number): number {
	if(b === 0)	{
		throw new Error('除数 b 等于 0，无法执行除法')
	}
}

// test.spec.ts
import { div } from './math.ts'

describe('当除以 0 时', () => {
  // it 的描述说明了异常消息的意图
	it('应该抛出一个异常，表明无法进行除以 0 的运算', () => {
		expect(() => div(12, 0)).toThrowError('除数 b 等于 0，无法执行除法')
	})
})
```

## 测试内容

选择要测试的内容会因正在执行的代码而异。不同因素，从性能到大量输入域，都会显著改变测试中应该测试或不应该测试的内容。这里我们将列出一组如果可能开发者应当测试的用例。

```tsx
function run(kilometers: number): number {
	if(kilometers > 1000) {
    throw new Error('跑者不能跑超过 1000 公里')
  }

  if(kilometers <= 10) {
		return kilometers
  } else if(kilometers > 10) {
		doSomething(kilometers)
    return beLazy(kilometers)
  }
}
```

这里的 run 函数有几条不同的执行路径。函数，或代码片段，不应仅仅根据不同可能的执行路径来测试，而应根据对函数的期望来测试，因为函数可能并没有按照其编写目的运行，而测试又可能与代码紧耦合，无法捕捉到其中的不同问题。这意味着开发者应始终测试所有可能的执行路径，并且也应根据函数的语义测试其他可能的路径。

对于这个具体情况，开发者应测试 **至少** 以下情况：

1. 以大于 1000 公里的距离运行 run 函数
2. 以等于 10 公里的距离运行 run 函数
3. 以大于 10 公里但小于 1000 公里的距离运行 run 函数

对于第一种情况，开发者需要测试 run 函数会抛出异常。 **异常必须检查其错误消息**，因为也可能抛出其他异常，从而使测试目的失效。如果该异常是自定义异常，开发者可以检查该异常的实例。

对于第二种情况，开发者需要测试函数返回的公里数与输入相同。

对于第三种情况，开发者需要测试函数返回了外部函数 **beLazy** 返回的结果，并且由于 **doSomething** （同样在该方法中执行）无法通过函数的返回值来检查，开发者应 **测试它是否以正确的参数被调用**.

```tsx
import { doSomething, beLazy } from '../runningUtils'
jest.mock('../runningUtils')

// 注意，这里没有主 describe
// 唯一的全局上下文是我们想要用 jest mock 的函数

const mockDoSomething = doSomething as jest.MockedFunction<typeof doSomething>
const mockBeLazy = beLazy as jest.MockedFunction<typeof beLazy>

describe('当跑步距离超过 1000 公里时', () => {
	it('应该抛出一个错误，表明跑者不能跑超过 1000 公里', () => {
		expect(() => run(1001)).toThrowError('跑者不能跑超过 1000 公里')
	})
})

describe('当跑步距离小于或等于 10 公里时', () => {
	it('应该返回与输入相同的公里数', () => {
		expect(run(2)).toEqual(2)
	})
})

describe('当跑步距离大于 10 公里但小于 1000 公里时', () => {
	const kilometers = 50
	let result: number
	beforeEach(() => {
		mockDoSomething.mockReturnValueOnce(undefined)
		mockBeLazy.mockImplementationOnce(value => value)
		result = run(kilometers)
	})
	
	it('应该在惰性处理后返回公里数', () => {
		expect(result).toEqual(kilometers)
	})

	it('应该使用给定的公里数调用 doSomething 函数', () => {
		expect(mockDoSomething).toHaveBeenCalledWith(kilometers)
	})
})
```

## 何时进行 mock

是否进行 mock 主要取决于开发者正在编写的测试类型。

* 单元测试应将所有外部函数都进行 mock，除了那些只做简单事情的函数，例如格式化字符串等。
* API 测试应只对与外部服务通信的部分进行 mock，也就是数据库或其他 API。如果测试中执行的某个操作影响了测试套件的性能，可以实现一个 mock 来缓解这个问题。

所有 mock 都必须使用 Jest 提供的工具完成，以下情况除外： `redux-saga-test-plan` mock。

### Mock 和工具函数

* 任何 Jest mock 都应尽可能使用其 **一次性** 方法来实现，也就是 `mockReturnValueOnce` 或 `mockResolvedValueOnce` 不建议使用 `mockReturnValue` 或 `mockResolvedValue`。这样做的理由是防止不必要的 mock 被执行，从而改变测试的执行。
* Jest 提供了许多类型，可用于不同种类的 mock。例如，在 mock 一个对象时，你应使用 `jest.Mocked<typeof someObject>`，对于类你应使用 `jest.MockedClass<typeof SomeClass>`，而对于函数， `jest.MockedFunction<typeof someFunction>`.
* 一个 `afterEach` 应编写为在每个测试之后运行，并使用一个 `jest.resetAllMocks` 来清除全局 mock 模块的任何 mock 实现，因为它们可能泄漏到其他测试中。如果你使用 `mockReturnValueOnce` 或 `mockResolvedValueOnce` ，则可以忽略这个重置，因为它会自动为你完成。
* 一个名为 `mocks` 可以创建用来存放 mock 的目录，这些 mock 应放在 `test` 或 `spec` 目录中。较大的 mock 必须与测试放在不同的文件中，以避免测试文件过长。较小的 mock，如果数量不多，应保留在测试中。
  * Mock 文件应以它们所 mock 的实体命名。例如，如果我们在 mock 一个 profile，那么该 mock 应放在 `/test/mocks/profile.ts` 文件中。
  * 如果需要多个较大的 mock，它们应放在一个以被 mock 实体命名的目录中的不同文件里。例如，如果我们有两个 profile mock，我们会将它们存为： `/test/mocks/profile/profile-with-wearables.ts` 和 `/test/mocks/profile/profile-without-wearables.ts`。为了便于访问，你应创建一个 `index.ts` 文件，放在 `/test/mocks/profile/` 目录中并导出它们。
  * 为了避免被 mock 对象发生变更，mock **必须以函数形式导出** ，由这些函数返回它们。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
