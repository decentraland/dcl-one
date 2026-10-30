# Writing Tests

Tests can be written in various ways. This section tries to give a standard and a rationale behind the way we writes tests.

Tests MUST be written using the **describe** and the **it** methods provided by jest

## Describing and building context

The **describe** method SHOULD be used to describe, whenever it's possible, the context on which the piece of code to be tested will be executed and SHOULD build that context inside of its scope.

```tsx
describe('when the flag is red', () => {
	...
})

describe('when the flag is blue', () => {
	...
})
```

To describe the context, we SHOULD use one of the following words: **when** for top level describes and **having** or **and** to indicate that there's a context that depends on other context that was defined before. The rationale behind this is to have a self described contexts that can be easily understood and checked by combining the describe sentences. As the idea is to concatenate the descriptions of the contexts, **describes** MUST be written in lowercase.

```tsx
describe('when the flag is red', () => {
	...
	describe('and the wind is strong', () => {
     // The context here is described by the conjuncton of the describes:
     // "when the red flag is red and the wind is strong"
     ...
  })
})
```

Multiple describes can be nested at the same and at a lower level. Describes at the same level denote different contexts and describes in lower levels (nested describes) denote a more specific context.

```tsx
describe('when the flag is red', () => {
	...
	describe('and the wind is strong', () => {
     ...
  })

	describe('and the wind is weak', () => {
     ...
  })
})
```

As mentioned before, each describe SHOULD be accompanied by a method that builds the context or a method that destroys or changes the context. The methods that MUST be used are the **beforeEach** and the **afterEach**.


> [!WARNING]
> The Jest framework has two methods, **beforeEach** and **beforeAll**, but combining both of them generates confusion and execution problems when nested contexts are present, as [the order on which they are executed](https://jestjs.io/docs/setup-teardown#scoping) is not what we would expect.


Contexts created with the **beforeEach** method will be more and more defined as we go deeper into the describe tree. Each **beforeEach** will specify the context more according to what their text says.

### Variable Scoping

The **variables** set in the contexts SHOULD be defined according to the scope that they're going to be used in. That is, if a variable is used only for a specific context in a nested level of describes, the variable SHOULD be defined only on that specific context. The rationale behind this is to keep the variables close to the code that they're being used in, making the code easier to understand.

```tsx
let flag = 'blue'
describe('when the flag is red', () => {
	let wind = 'mild'
	beforeEach(() => {
    flag = 'red'
  })
  ...

	describe('and the wind is strong', () => {
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
```

Variables of **primitive types**, that **won't be changed** in the context that they're defined or in subsequent contexts SHOULD be defined as **const** once, over the **beforeEach** definition in the scope.

Variables of **primitives types** that will be changed in the context that they're defined or in subsequent contexts MUST be defined as **let**, as the nature of Typescript obliges it.

Variables of **non primitive types** (objects, arrays, etc) MUST be defined as **let** and their value MUST be set in a **beforeEach.** The rationale behind this is that objects in JS are mutable, that means that any execution of code that uses that object is subject to change and affect other tests in an involuntarily manner.

```tsx
let flag
describe('when the flag is red', () => {
  // A primitive value that is going to be modified in multiple scopes.
	let wind
  // An object value that is going to be used in the test's code.
  let someObject
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// A constant primitive value that is going to be used only in this scope.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

Variables SHOULD be correctly typed whenever possible. Repeated types SHOULD be abstracted into a type.

```tsx
let flag: string
describe('when the flag is red', () => {
  // A primitive value that is going to be modified in multiple scopes.
	let wind: string
  // An object value that is going to be used in the test's code.
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1 }
  })
  ...

	describe('and the wind is strong', () => {
		// A constant primitive value that is going to be used only in this scope.
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    ...
  })

	describe('and the wind is weak', () => {
		beforeEach(() => {
	    wind = 'weak'
	  })
    ...
  })
})
...
```

## Describing expectations and executing code

The **it** method MUST be always placed inside the scope of a **describe** and MUST be used to describe what is that we expect from the code that we're going to test and SHOULD contain, if possible, only **one** assertion. Multiple assertions per **it** can exist if there's a performance problem, as the tests will be ran once per **it** (because of the **beforeEach**), or if what is expected can be described with clarity in the expectation description.

```tsx
let flag: string
describe('when the flag is red', () => {
	let wind: string
  let someObject: { id: string }
	beforeEach(() => {
    flag = 'red'
		someObject = { id: 1, swim: jest.fn() }
  })
  ...

  // Single expectation of the execution of a test in a context (one it)
	describe('and the wind is strong', () => {
		const aConstantPrimitiveValue = 'something'
		beforeEach(() => {
	    wind = 'strong'
	  })
    
		it('should not go swimming', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(false)
    })
  })

  // Multiple expectations of the execution of a test in a context (two its)
	describe('and the wind is weak', () => {
    let result: boolean
		beforeEach(() => {
	    wind = 'weak'
      result = goSwimming(flag, wind, someObject)
	  })

		it('should go swimming', () => {
      expect(goSwimming(flag, wind, someObject)).toBe(true)
    })

    it('should have called the swim method', () => {
      expect(someObject.swim).toHaveBeenCalled()
    })
  })
})
...
```

The rationale behind this structure is to provide clarity to the reviewer of the tests and to the developers that will maintain and make changes to the code as each context and expectation is clearly enumerated, making it easier to understand what is being tested, how it is tested, and what's left to test.

Following the **describe** descriptions down to the **it**s the developers can easily understand what is being tested by concatenating the sentences.

```tsx
// This is read as: "when the flag is red and the wind is strong, 
// it should go swimming"
describe('when the flag is red', () => {
  ...
  describe('and the wind is strong', () => {
		...
		it('should go swimming', () => {
			...
		})
  })
})
```

### Writing Clear Expectations

The description of the expectations written in the **it**s MUST be as descriptive as possible about what it is expected from the execution of the tests. The develops MUST NOT use abstract or general phrasing to define expectations as it removes clarity on what is being expected from the code.


> [!CAUTION]
> **The developer MUST NOT use phrases like:**
> 
> * "should work as expected" ⇒ What is to work as expected?
> * "should return the correct value" ⇒ What is the correct value?
> * "should resolve/return/work correctly" ⇒ How does something work correctly?
> * "should fail" ⇒ How should it fail? What's the message that it should provide?


It must be taken into consideration that when specifying what is expected in the **it**s or what the context will be in the **describe**s the developers MAY use function names or exact error messages if it is required to better understand the intention, but they SHOULD use a textual representation when possible to make tests easier to maintain.

```tsx
// math.ts
export function div(a: number, b: number): number {
	if(b === 0)	{
		throw new Error('The divisor b equals 0, the division can\'t be performed')
	}
}

// test.spec.ts
import { div } from './math.ts'

describe('when dividing by zero', () => {
  // The description of the it describes the the intention of the exception's message
	it('should throw an exception signaling that a division by 0 is not possible', () => {
		expect(() => div(12, 0)).toThrowError('The divisor b equals 0, the division can\'t be performed')
	})
})
```

## What to test

Choosing what to test can vary depending on the code that is being executed. Different factors, from performance or a big domain of inputs can definitely change what should or should not be tested from the tests. Here we'll expose a single set of cases that the developer SHOULD test if possible.

```tsx
function run(kilometers: number): number {
	if(kilometers > 1000) {
    throw new Error('The runner can\'t run more than 1000 kilometers')
  }

  if(kilometers <= 10) {
		return kilometers
  } else if(kilometers > 10) {
		doSomething(kilometers)
    return beLazy(kilometers)
  }
}
```

The run function here has a couple of different execution flows. Functions, or sections of code, SHOULD NOT be tested based only on the different possible execution flows, but in what is expected from the function, as the function might not be doing what it is was written for and the tests could be tightly coupled with the code and not catch different issues with it. This implies that the developer SHOULD always test all the possible execution paths and SHOULD test other possible paths too in accordance with the semantics of the function.

For this specific case, the developer SHOULD test **at least** the following cases:

1. Running the run function with an amount of kilometers greater than 1000
2. Running the run function with an amount of kilometers equal to 10
3. Running the run function with an amount of kilometers greater than 10 but lower than 1000

For the first case, the developer needs to test that the run function throws an exception. **Exceptions MUST be checked for its error message**, as any other exception could be also thrown, invalidating the purpose of the test. If the exception is a custom exception, the developer MAY check for the instance of the exception.

For the second case, the developer needs to test that the function returned the same number of kilometers that is was given.

For the third case, the developer needs to test that the function returned what an external function, **beLazy** returned and, as **doSomething** (also executed in the method) can't be checked through the return value of the function, the developer SHOULD **test that it was called with the correct arguments**.

```tsx
import { doSomething, beLazy } from '../runningUtils'
jest.mock('../runningUtils')

// Note that there's no main describe
// The only global context are functions that we want to mock with jest

const mockDoSomething = doSomething as jest.MockedFunction<typeof doSomething>
const mockBeLazy = beLazy as jest.MockedFunction<typeof beLazy>

describe('when running more than 1000 kilometers', () => {
	it('should throw an error signaling that the runner can\'t run more than 1000 kilometers', () => {
		expect(() => run(1001)).toThrowError("The runner can't run more than 1000 kilometers")
	})
})

describe('when running less or equal than 10', () => {
	it('should return the same amount of kilometers as the ones given', () => {
		expect(run(2)).toEqual(2)
	})
})

describe('when running more than 10 kilometers but less than 1000', () => {
	const kilometers = 50
	let result: number
	beforeEach(() => {
		mockDoSomething.mockReturnValueOnce(undefined)
		mockBeLazy.mockImplementationOnce(value => value)
		result = run(kilometers)
	})
	
	it('should return the kilometers after being lazy', () => {
		expect(result).toEqual(kilometers)
	})

	it('should call the doSomething function with the given kilometers', () => {
		expect(mockDoSomething).toHaveBeenCalledWith(kilometers)
	})
})
```

## When to mock

Mocking will depend mostly on the type of tests that the developer is writing.

* Unit tests SHOULD have every external function mocked, with the exception of functions that do simple stuff, like formatting strings, etc.
* API Tests SHOULD only have mocks for communicating with external services, that is, DBs or different APIs. If one of the operations that is done in the test affects the performance of the test suite, a mock could be implemented to mitigate this issue.

All mocks MUST be done using the tools provided by Jest, with the exception of the `redux-saga-test-plan` mocks.

### Mocks and utility functions

* Any Jest mocks SHOULD be mocked using their **once** method when possible, that is, `mockReturnValueOnce` or `mockResolvedValueOnce` is recommended against `mockReturnValue` or `mockResolvedValue`. The rationale behind this is to prevent unwanted mocks from executing, changing the execution of our tests.
* Jest offers a bunch of Types that you can use for different kind of mocks. For example, when mocking an object you SHOULD use `jest.Mocked<typeof someObject>`, for classes you SHOULD use `jest.MockedClass<typeof SomeClass>`, and for functions, `jest.MockedFunction<typeof someFunction>`.
* An `afterEach` SHOULD be written to run after each tests with a `jest.resetAllMocks` to clear any mocked implementation of globally mocked modules that might leak into other tests. You can ignore this reset if you are using `mockReturnValueOnce` or `mockResolvedValueOnce` because it is done automatically for you.
* A directory named `mocks` MAY be created to store mocks, which should be placed in the `test` or `spec` directory. Big mocks MUST be stored in different files from the tests to avoid having extended test files. Small mocks, if there aren't many, SHOULD be kept in the tests.
  * Mock files SHOULD be named as the entity they're mocking, so for example, if we're mocking a profile, the mock should be placed under the `/test/mocks/profile.ts` file.
  * In case several big mocks are required, they SHOULD be placed in different files inside a directory called as the entity to be mocked, so for example, if we have two profile mocks, we'll store them as: `/test/mocks/profile/profile-with-wearables.ts` and `/test/mocks/profile/profile-without-wearables.ts`. To easy access them, you SHOULD create an `index.ts` file inside the `/test/mocks/profile/` directory and export them.
  * In order to avoid mutation of mocked objects, mocks **MUST be exported as functions** that return them.
