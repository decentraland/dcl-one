# 测试 UI

本节描述开发者将如何测试与 UI 相关的代码库。

## 测试栈

我们所有的 UI 测试都必须使用 [Jest](https://jestjs.io/) 作为主要测试框架，并配合 [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan) 作为 sagas 测试工具，以及 [react testing library](https://testing-library.com/docs/react-testing-library/intro/) 作为 UI 组件测试工具。测试代码必须使用 [TypeScript](https://www.typescriptlang.org/) 并使用 [ts-jest](https://github.com/kulshekhar/ts-jest) 以获得类型检查支持。

## 测试内容

* UI 组件
* Sagas
* Reducers
* Selectors
* Action creators
* 工具函数

## 测试 sagas

Sagas 让我们能够通过创建处理 redux actions 的生成器来处理副作用。sagas 的主要优势之一是其可测试性，因为 sagas 中广泛使用的 effects 都非常容易测试。

这些处理器必须使用 [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan)，并且测试必须覆盖整个执行过程，从整个模块的处理器（其主处理器或 sagas）开始，一直到所有会对我们将要 dispatch 的 action 作出响应的处理器的执行。这个方法使 sagas 测试更容易构建和维护，并且由于测试尽可能覆盖整个流程，因此也提升了测试价值。

处理器的测试必须通过它们与 effects 的交互来进行，主要是通过 `put` 这个，因为它们的目的是处理 actions 并生成新的 actions。大多数测试肯定会包含一个 `provide` call，用于模拟 effects，一个或多个 `put` call，用于验证处理器是否按预期 dispatch 了 actions，以及一个单独的 `dispatch` 之后跟一个 `run` 调用，以执行整个 sagas 直到完成。

对被测试模块外部资源的模拟 SHOULD 使用 redux-saga-test-plan 的 providers 来完成，并利用 redux-sagas 提供的不同 effects。像 call、apply 或其他 effects，只要在测试中调用某个函数时有可能，就 SHOULD 使用，这样便于轻松地进行模拟。Jest 不应被用来模拟模块或函数。

describe 中的文本及其内容必须遵循以下准则：

* 一个 saga 或主处理器的主 describe 必须以 *当处理*开头，表示接下来要测试的是一个处理器。紧接着 *当处理* 这个短语，必须写出要处理的 action（或该 action 的意图）的描述。例如： *当处理表示成功获取的 action 时。*
* 内部 describe 将用于区分执行上下文，它们遵循与 [描述和构建上下文](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#describing-and-building-context) 章节中相同的规则。
* 该 **其**s 应描述测试的预期结果，并以一个 *应 put* 以及在执行处理器时应该被 put 的 action 描述。

### Sagas 示例

```tsx
...
export function* tiersSaga(builder: BuilderAPI) {
	function* handleBuyThirdPartyItemTierRequest(action: BuyThirdPartyItemTiersRequestAction) {
    const { thirdParty, tier } = action.payload
    try {
			// 获取 chain id 可能会失败，必须编写测试来确保这种场景被覆盖
      const maticChainId: ChainId = yield call(getChainIdByNetwork, Network.MATIC)
      const thirdPartyContract = yield call(getContract, ContractName.ThirdPartyRegistry, maticChainId)
			// 发送交易可能成功也可能失败，这两种情况都必须在测试中被覆盖
      const txHash: string = yield call(sendTransaction, thirdPartyContract, instantiatedThirdPartyContractContract =>
        instantiatedThirdPartyContractContract.buyItemSlots(thirdParty.id, tier.id, tier.price)
      )
      yield put(buyThirdPartyItemTiersSuccess(txHash, maticChainId, thirdParty, tier))
    } catch (error) {
      yield put(buyThirdPartyItemTiersFailure(error.message, thirdParty.id, tier))
    }
  }
}
```

### Sagas 测试示例

```tsx
// 主 describe 以 'when handling...' 开头
describe('当处理购买第三方物品等级的请求时', () => {
	// 所有情况，无论正面还是负面，都被覆盖
  describe('并且无法获取 chain id 时', () => {
    it('应放置一个表示购买物品槽位等级失败的 action，其中包含该等级和第三方 id', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
				// 通过 provide 调用模拟不同的 effects，构建不同的场景
        .provide([[call(getChainIdByNetwork, Network.MATIC), Promise.reject(new Error(defaultError))]])
				// 通过 put 调用验证预期的最终结果是否发生
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
				// 每个测试只进行一次 dispatch，以覆盖我们正在测试的 action 处理
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('并且发送交易失败时', () => {
    it('应放置一个表示购买物品槽位等级失败的 action，其中包含该等级和第三方 id', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
					// call 要精确匹配调用 effect，并考虑参数
					// 这能确保我们的测试验证在 put 之前执行的代码是否符合预期
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
					// matchers 只用于匹配函数，而不匹配参数
          [matchers.call.fn(sendTransaction), Promise.reject(new Error(defaultError))]
        ])
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('并且发送交易成功时', () => {
    let contract: any
    beforeEach(() => {
      contract = { buyItemSlots: jest.fn() }
    })

    it('应放置一个表示成功购买物品槽位等级的 action，其中包含该等级、第三方和交易详情', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
          [matchers.call.fn(sendTransaction), Promise.resolve(txHash)]
        ])
        .put(buyThirdPartyItemTiersSuccess(txHash, ChainId.MATIC_MUMBAI, thirdParty, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })
})
```

## 测试 reducers

Reducers 是接收 state 和 action 并返回新 state 的函数。这个新 state 可能包含针对给定 action 预期发生的更改。

如有需要，测试应该变化初始 state 和待测 action 的不同参数。断言必须针对整个返回的 state，因为一个 action 可能会修改给定 initial state 的任何部分。

在归约一个 action 时，我们应该使用 action creator，以便让测试更易维护。

为了统一我们的编写方式 **describe**s 和 **其**s：

* 主 describe 应以说明将要被归约的 actions 开头，使用短语 *当归约该 action 时* ，后面跟着对该 action 的描述。我们不应使用 action type 来描述待测 action，因为 type 可能会变化。
* 该 **其**s 应以清晰的方式描述返回的 state 是如何变化的。例如： *它应返回一个 error 被清空且 fruits 已设置的 state*.

### Reducer 示例

```tsx
const INITIAL_STATE: FruitsState = {
  data: {
    fruits: []
  },
  loading: []
}

export function fruitsReducer(state = INITIAL_STATE, action: any): FruitsState {
  switch (action.type) {
    case FETCH_FRUIT_REQUEST: {
      return {
        ...state,
        loading: loadingReducer(state.loading, action),
				error: null
      }
    }
		case FETCH_FUIT_ERROR: {
			const { error } = action
      return {
        ...state,
				error,
        loading: loadingReducer(state.loading, action)
      }
		}
		case FETCH_FRUIT_SUCCESS: {
			const { fruits } = action
      return {
        ...state,
				fruits,
        loading: loadingReducer(state.loading, action)
      }
		}
}
```

### Reducers 测试示例

```tsx
let state: FruitsState

beforeEach(() => {
	state = {
	  data: {
	    fruits: []
	  },
	  loading: [],
		error: null
	}
})

describe('当归约表示获取 fruits 的 action 时', () => {
	beforeEach(() => {
		// 设置一个 error，以确保在归约该 action 时它会被清除。
		state.error = 'anError'
	})

	it('应返回一个 error 已清除且 loading 状态已设置为 loading 的 state', () => {
		// 使用 action creators 来测试 reducer，以便更容易维护。
		const action = fetchFruitsRequest()

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error: null,
			/* 由于 loading reducer 是另一个 reducer 的一部分（并且不需要被测试），
				 我们可以选择直接使用它，或者使用一个模拟它的辅助函数。
			*/
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('当归约表示获取 fruits 失败的 action 时', () => {
	const error = 'anError'

	it('应返回一个 error 已设置且 loading 状态已从 fruits 获取 action 中清除的 state', () => {
		const action = fetchFruitsFailure(error)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error,
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('当归约表示获取 fruits 成功的 action 时', () => {
	let fruits: string[]
	beforeEach(() => {
		fruits = ['apples', 'pears']
	})

	it('应返回一个 fruits 已设置且 loading reducer 已从 fruits 获取 action 中清除的 state', () => {
		const action = fetchFruitsSuccess(fruits)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			fruits,
			loading: loadingReducer(state.loading, action),
		})
	})
})
```

## 测试 selectors

Selectors 是接收 state 并返回该 state 的某一部分或其转换后部分的方法。

有两种 selectors：带缓存的 selectors 和普通 selectors。普通 selectors 必须像其他单元测试函数一样进行测试，而带缓存的 selectors 应使用以下方式进行测试： `resultFunc` 或接收 state 中已缓存部分并返回所需值的函数。

**描述**s 和 **其**s 必须按照以下内容所述进行编写： [描述和构建上下文](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#describing-and-building-context) 章节中相同的规则。

### Selectors 示例

```tsx
// 普通 selector
export const collectionExists = (state: RootState, id: string): boolean => {
	return state.collections.data.some((collection) => collection.id === id)
}

// 使用 reselect 的带缓存 selector，返回增强后的 collection 列表
export const getEnhancedCollections = createSelector(
	getCollections,
	getExtraData,
	(collections, extraData) => {
		return collections.map((collection) =>
			({ ...collection, ...extraData[collection[id]] }))
	}
)
```

### Selectors 测试示例

```tsx
let state: RootState
let collection: { id: string }
beforeEach(() => {
	collection = {
		id: 'anId'
	}
	state = {
		collections: {
			data: []
		}
	}
})

beforeEach(() => {
	state.collections.data = [collection]
})

// 普通 selectors 按照普通函数进行单元测试
describe('当检查某个 collection 是否存在时', () => {
	describe('并且没有具有给定 id 的 collection 时', () => {
		it('应返回 false', () => {
			expect(collectionExists(state, "anotherId")).toBe(false)
		})
	})

	describe('并且存在具有给定 id 的 collection 时', () => {
		it('应返回 true', () => {
			expect(collectionExists(state, collection.id)).toBe(true)
		})
	})
})

describe('当获取增强后的 collections 时', () => {
	let enhacements: Record<string, Enhacement>

	describe('并且没有可用于增强它们的数据时', () => {
		beforeEach(() => {
			enhacements = {}
		})

		// 带缓存的 selectors 应使用 resultFunc 进行单元测试
		it('应获取所有 collection，但不对它们进行增强', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				collection
			])
		})
	})

	describe('并且有可用于增强它们的数据时', () => {
		beforeEach(() => {
			enhacements = { [collection.id]: { aNewProperty: true } }
		})

		it('应获取所有已增强的 collection', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				{ ...collection, ...enhacements[collection.id] }
			])
		})
	})
})
```

## 测试 action creators

Action creators 负责创建之后会在 reducers 中处理的 action。尽管 action creators 通常很简单，但它们仍然必须经过测试，以确保它们按预期工作。

**描述**s 和 **其**s 必须按照以下内容所述进行编写： [描述和构建上下文](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/writing-tests/#describing-and-building-context) 章节中相同的规则。

### Action creators 示例

```tsx
export const BUY_THIRD_PARTY_ITEM_TIERS_REQUEST = '[请求] 购买第三方物品等级'
export const BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS = '[成功] 购买第三方物品等级'
export const BUY_THIRD_PARTY_ITEM_TIERS_FAILURE = '[失败] 购买第三方物品等级'

export const buyThirdPartyItemTiersRequest = (thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_REQUEST, { thirdParty, tier })
export const buyThirdPartyItemTiersSuccess = (txHash: string, chainId: ChainId, thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS, { thirdParty, tier, ...buildTransactionPayload(chainId, txHash, { tier, thirdParty }) })
export const buyThirdPartyItemTiersFailure = (error: string, thirdPartyId: string, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_FAILURE, { error, thirdPartyId, tier })
```

### Action creator 测试示例

```tsx
describe('当创建表示 tiers 获取请求开始的 action 时', () => {
  it('应返回一个表示 tiers 获取开始的 action', () => {
    expect(fetchThirdPartyItemTiersRequest()).toEqual({ type: FETCH_THIRD_PARTY_ITEM_TIERS_REQUEST })
  })
})

describe('当创建表示 tiers 获取成功的 action 时', () => {
  it('应返回一个表示 tiers 获取成功的 action', () => {
    expect(fetchThirdPartyItemTiersSuccess([thirdPartyItemTier])).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_SUCCESS,
      payload: { tiers: [thirdPartyItemTier], error: undefined, meta: undefined }
    })
  })
})

describe('当创建表示 tiers 获取失败的 action 时', () => {
  it('应返回一个表示 tiers 获取失败的 action', () => {
    expect(fetchThirdPartyItemTiersFailure(defaultError)).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_FAILURE,
      payload: { error: defaultError }
    })
  })
})
```

## 测试 UI 组件

为了测试 UI 组件，我们使用 [React Testing Library](https://testing-library.com/docs)。我们使用这个库而不是 enzyme 等其他选项，因为它让我们能够在考虑用户将如何与组件交互的基础上测试组件，而不是测试实现本身。我们不会与 React 组件实例交互，而是以它们渲染出来的方式与 DOM 元素交互。

### 测试内容

这个 UI 组件测试的目的是测试组件的行为以及用户如何与其交互。在许多情况下，可能会有一些组件连接到 Redux store，以获取某些属性并调用某些 action。我们通常通过单独创建一个 `container` 文件来与 store 交互。这些文件不在这种测试的范围内。我们测试的是组件在不同 props 值下的行为，我们不应关心这些值的来源（redux store、context、父组件），而应关注它们如何影响最终渲染。

### 渲染

每个测试都应该先调用 render 函数。这不仅会渲染我们作为参数传入的组件，还会渲染所有子组件。我们应避免在测试 setup 中这样做（`beforeEach`），因为这可能会导致问题，使 DOM 树中包含比我们需要更多的组件。我们应将结果保存到一个名为 **screen**的属性中。这样做可以让我们在测试中获取所需的所有属性，而无需不断更改解构。

```jsx
it('应渲染 hello world 组件', () => {
	const screen = render(<HelloWorld />)
})
```

如果该文档将有多个测试，且组件有多个 props，我们可以在文件顶部创建一个函数，用于设置默认 props 并渲染组件。

```jsx
function renderSelectedFilters(props: Partial<Props> = {}) {
  return render(
    <SelectedFilters
      isLandSection={false}
      category={undefined}
      browseOptions={{}}
      onBrowse={jest.fn()}
      {...props}
    />
  )
}
```

### 查询

Queries 是 Testing Library 提供给你用于在页面上查找元素的方法。它们有不同的 [查询类型](https://testing-library.com/docs/queries/about/#priority) 可用于访问元素。我们应尽量始终使用那些 `对所有人都可访问的` 因为它们反映了所有用户的体验（视觉/鼠标用户以及使用辅助技术的用户）。

这些查询包括 `getByRole`, `getByLabelText`, `getByPlaceholderText`, `getByText`, `getByDisplayValue`。如果我们能够使用这些查询访问元素，那可能意味着该组件不可访问。

### 测试用户事件

我们无法直接访问组件内部的函数，因此应模拟会导致这些函数被调用的用户行为。为此我们使用 `userEvent` 库。

```jsx
const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
const commonPill = screen.getByTestId('pill-common')
await userEvent.click(within(commonPill).getByRole('button'))
```

### 调试

要调试测试以及在特定时间点查看 DOM 的当前状态，我们可以使用 `debug` 函数，它是调用 render 时返回的属性之一

```jsx
const { debug } = render(<HelloWorld />)

debug()
```

### UI 组件测试完整示例

```jsx
function renderSelectedFilters(props: Partial<Props> = {}) {
  return render(
    <SelectedFilters
      isLandSection={false}
      category={undefined}
      browseOptions={{}}
      onBrowse={jest.fn()}
      {...props}
    />
  )
}

describe('稀有度筛选', () => {
  it('应渲染稀有度', () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions
    })
    expect(screen.getByText(Rarity.COMMON)).toBeInTheDocument()
    expect(screen.getByText(Rarity.EPIC)).toBeInTheDocument()
  })

  it('应在未删除稀有度的情况下调用 onBrowse', async () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const onBrowseMock = jest.fn()

    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
    const commonPill = screen.getByTestId('pill-common')
    await userEvent.click(within(commonPill).getByRole('button'))
    expect(onBrowseMock).toHaveBeenCalledWith({ rarities: [Rarity.EPIC] })
  })
})
```

## 目录结构

测试必须与它们所对应的文件或模块放在一起，名称相同，但扩展名为 `spec.ts` 而不是 `ts`.

文件结构必须如下所示：

```
actions.ts
action.spec.ts
reducer.ts
reducer.spec.ts
sagas.ts
sagas.spec.ts
selectors.ts
selectors.spec.ts
utils.ts
utils.spec.ts
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-uis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
