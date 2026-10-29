# Testing UIs

This section describes how developers will test codebase related to the UI.

## Testing stack

All of our UI tests MUST be done using [Jest](https://jestjs.io/) as the main testing framework with [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan) as the sagas testing tool and [react testing library](https://testing-library.com/docs/react-testing-library/intro/) as ui components testing tool. The test's code MUST be written using [Typescript](https://www.typescriptlang.org/) and ran using [ts-jest](https://github.com/kulshekhar/ts-jest) to have type checking support.

## What to test

* UI components
* Sagas
* Reducers
* Selectors
* Action creators
* Utility functions

## Testing sagas

Sagas lets us handle side-effects by letting us create generators that handle redux actions. One of the main advantage of sagas is its testability, as the effects are widely used in sagas are highly testable.

These handler MUST be tested using [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan), and the test MUST exercise the whole execution, from whole module's handler (its main handler or sagas) through all the execution of the handlers that will react to the action we're going to dispatch. This approach makes sagas tests easy to build and maintain, and brings value to the tests as they're going through as much as the whole flow as possible.

Handlers' tests MUST be tested through their interaction with the effects, mainly with the `put` one, as their purpose is to handle actions and produce new ones. Most of the tests will surely contain a `provide` call, used to mock effects, one or multiple `put` calls, to verify that the handlers dispatches the actions as expected and a single `dispatch` followed by a `run` call to execute the whole sagas until completion.

The mocking of resources external to the module that is being tested SHOULD be done using the redux-saga-test-plan providers by taking advantage of the different effects provided by redux-sagas. Effects like call, apply or others SHOULD be used whenever it's possible when a function is called in a test to allow mocking them easily. Jest SHOULD NOT be used to mock modules or functions.

The text in the describe and its MUST follow these guidelines:

* The main describe of a saga or main handler MUST start with *when handling*, indicating that what is going to be tested is a handler. Following the *when handling* phrase, a description of the action (or the intention that the action) to be handled MUST be written. E.g. *when handling the action that signals a successful fetch.*
* Internal describes will differentiate execution contexts, they follow the same rules as described in the [Describing and building context](/docs/contributor/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) section.
* The **it**s SHOULD describe what is expected from the test, starting with a *should put* and the description of the actions that should be put when executing the handler.

### Sagas example

```tsx
...
export function* tiersSaga(builder: BuilderAPI) {
	function* handleBuyThirdPartyItemTierRequest(action: BuyThirdPartyItemTiersRequestAction) {
    const { thirdParty, tier } = action.payload
    try {
			// Getting the chain id can fail, a test must be made to ensure that this scenarios is excercised
      const maticChainId: ChainId = yield call(getChainIdByNetwork, Network.MATIC)
      const thirdPartyContract = yield call(getContract, ContractName.ThirdPartyRegistry, maticChainId)
			// Sending the transasction can either success or fail, both cases must be exercised in a test
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

### Sagas test example

```tsx
// The main describe starts with 'when handling...'
describe('when handling the request to buy a third party item tier', () => {
	// All cases, negative and positive are exercised
  describe('and the chain id couldn\'t be retrieved', () => {
    it('should put the action signaling the failure of the purchase of an item slots tier with the tier and the third party id', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
				// Provide calls mock the different effects, building different scenarios
        .provide([[call(getChainIdByNetwork, Network.MATIC), Promise.reject(new Error(defaultError))]])
				// Put calls verify that what was expected as the end result happened
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
				// A single dispatch per test is done to exercise the handling of the action that we're testing
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('and sending the transaction fails', () => {
    it('should put the action signaling the failure of the purchase of an item slots tier with the tier and the third party id', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
					// Call match the exact call effect, taking into consideration the parameters
					// This makes our test ensure that the code executed before a put did what we expected
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
					// Matchers are used to only match the function and not the parameters
          [matchers.call.fn(sendTransaction), Promise.reject(new Error(defaultError))]
        ])
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('and sending the transaction succeeds', () => {
    let contract: any
    beforeEach(() => {
      contract = { buyItemSlots: jest.fn() }
    })

    it('should put the action signaling the successful purchase of an item slots tier with the tier, the third party and the transaction details', () => {
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

## Testing reducers

Reducers are functions that take a state and an action and return a new state. This new state MAY contain changes programmed to occur for the given action.

Tests SHOULD vary the initial states and the different parameters of the action to be tested if required. The assertions MUST be done to whole returned state as an action could modify any part of the given initial state.

When reducing an action, we SHOULD use the action creator, making the tests easier to maintain.

To standardize the way we write **describe**s and **it**s:

* Main describes SHOULD start with describing which actions are going to be reduced by using the phrase *when reducing the action* followed by a description of the action. We SHOULD NOT use the action type to describe the action to be tested, as the type might change.
* The **it**s SHOULD describe the way the returned state changed in a clear way. E.g: *it should return a state with the error nulled and the fruits set*.

### Reducer example

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

### Reducers test example

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

describe('when reducing the action that signals the fetching of fruits', () => {
	beforeEach(() => {
		// An error is set to ensure that it gets cleared when reducing the action.
		state.error = 'anError'
	})

	it('should return a state where the error is cleared and the loading state is set to loading', () => {
		// Action creators are used to test the reducer so it can be maintained with ease.
		const action = fetchFruitsRequest()

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error: null,
			/* As the loading reducer is part of another reducer (and doesn't need to be tested),
				 we MAY choose to use it as is or we MAY use a helper that emulates it.
			*/
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('when reducing the action that signals the failure of fetching fruits', () => {
	const error = 'anError'

	it('should return a state where the error is set and the loading state is cleared from the fruits fetch action', () => {
		const action = fetchFruitsFailure(error)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error,
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('when reducing the action that signals the success of fetching fruits', () => {
	let fruits: string[]
	beforeEach(() => {
		fruits = ['apples', 'pears']
	})

	it('should return a state where the fruits are set and the loading reducer is cleared from the fruits fetch action', () => {
		const action = fetchFruitsSuccess(fruits)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			fruits,
			loading: loadingReducer(state.loading, action),
		})
	})
})
```

## Testing selectors

Selectors are methods that takes a state and returns a section of that state or a transformed section of the state.

There are two types of selectors, memoized selectors and normal or common selectors. Normal or common selectors MUST be tested like any other unit tested function and memoized selectors SHOULD be tested using the `resultFunc` or the function that receives the memoized parts of the state and returns the wanted value.

**Describe**s and **it**s MUST be written as described in the [Describing and building context](/docs/contributor/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) section.

### Selectors example

```tsx
// Common selector
export const collectionExists = (state: RootState, id: string): boolean => {
	return state.collections.data.some((collection) => collection.id === id)
}

// Memoized selector using reselect that returns a list of enhanced collections
export const getEnhancedCollections = createSelector(
	getCollections,
	getExtraData,
	(collections, extraData) => {
		return collections.map((collection) =>
			({ ...collection, ...extraData[collection[id]] }))
	}
)
```

### Selectors test example

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

// Common selectors are unit tested as normal functions
describe('when checking if a collection exists', () => {
	describe('and there\'s no collection with the given id', () => {
		it('should return false', () => {
			expect(collectionExists(state, "anotherId")).toBe(false)
		})
	})

	describe('and there\'s a collection with the given id', () => {
		it('should return true', () => {
			expect(collectionExists(state, collection.id)).toBe(true)
		})
	})
})

describe('when getting the enhanced collections', () => {
	let enhacements: Record<string, Enhacement>

	describe('and there\'s no data to enhance them', () => {
		beforeEach(() => {
			enhacements = {}
		})

		// Memoized selectors should be unit tested using resultFunc
		it('should retrieve all the collections without enhancing them', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				collection
			])
		})
	})

	describe('and there\'s data to enhance them', () => {
		beforeEach(() => {
			enhacements = { [collection.id]: { aNewProperty: true } }
		})

		it('should retrieve all the enhanced collections', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				{ ...collection, ...enhacements[collection.id] }
			])
		})
	})
})
```

## Testing action creators

Action creators are in charge of creating the actions that will later be processed in the reducers. Although action creators are usually simple, they MUST be tested to ensure that they do what we expect.

**Describe**s and **it**s MUST be written as described in the [Describing and building context](/docs/contributor/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) section.

### Action creators example

```tsx
export const BUY_THIRD_PARTY_ITEM_TIERS_REQUEST = '[Request] Buy a third party item tier'
export const BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS = '[Success] Buy a third party item tier'
export const BUY_THIRD_PARTY_ITEM_TIERS_FAILURE = '[Failure] Buy a third party item tier'

export const buyThirdPartyItemTiersRequest = (thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_REQUEST, { thirdParty, tier })
export const buyThirdPartyItemTiersSuccess = (txHash: string, chainId: ChainId, thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS, { thirdParty, tier, ...buildTransactionPayload(chainId, txHash, { tier, thirdParty }) })
export const buyThirdPartyItemTiersFailure = (error: string, thirdPartyId: string, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_FAILURE, { error, thirdPartyId, tier })
```

### Action creator test example

```tsx
describe('when creating the action that signals the start of a tiers fetch request', () => {
  it('should return an action signaling the start of the tiers fetch', () => {
    expect(fetchThirdPartyItemTiersRequest()).toEqual({ type: FETCH_THIRD_PARTY_ITEM_TIERS_REQUEST })
  })
})

describe('when creating the action that signals the successful fetch of tiers', () => {
  it('should return an action signaling the success of the tiers fetch', () => {
    expect(fetchThirdPartyItemTiersSuccess([thirdPartyItemTier])).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_SUCCESS,
      payload: { tiers: [thirdPartyItemTier], error: undefined, meta: undefined }
    })
  })
})

describe('when creating the action that signals the unsuccessful fetch of tiers', () => {
  it('should return an action signaling the failure of the tiers fetch', () => {
    expect(fetchThirdPartyItemTiersFailure(defaultError)).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_FAILURE,
      payload: { error: defaultError }
    })
  })
})
```

## Testing ui components

For testing UI components we are using [React Testing Library](https://testing-library.com/docs). We are using this library instead of other options like enzyme as it lets us test components taking into consideration how the user will interact with it instead of the implementation itself. We won't be interacting with react component instances, instead we are interacting with dom elements in the way they are rendered.

### What to test

The purpose of this ui components test is to test the behavior of the component and how the user interacts with it. In many cases there might be components that are connected to the redux store to get some properties and call some actions. We usually handle this by making a separate `container` file that will interact with the store. These files are out of the scope for this kind of testing. We are testing how the component behaves with different props values, we shouldn't care the source of those values (redux store, context, parent component), but how they impact the final render.

### Rendering

Every test should start calling the render function. This will render not only the component we are sending as the parameter, but also all the children. We should avoid doing this in test setup (`beforeEach`) as it can cause issues making the dom tree have more components that we want. We should save the result in a property name **screen**. Doing this will let us get all the properties we need in the test without the need to change constantly the destructuring.

```jsx
it('should render hello world component', () => {
	const screen = render(<HelloWorld />)
})
```

If the document will have multiple tests and the component has multiple props we can create a function at the top of the file that will take care of setting default props and render the componet.

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

### Queries

Queries are the methods that Testing Library gives you to find elements on the page. The are different [kinds of queries](https://testing-library.com/docs/queries/about/#priority) that we can use to access the elements. We should try to always use the queries that are `Accessible to everyone` as they are the ones that reflect the experience for all users (visual/mouse and using assistive technology).

Those queries are `getByRole`, `getByLabelText`, `getByPlaceholderText`, `getByText`, `getByDisplayValue`. If we can access the element using this queries it probably means that the component is not accessible.

### Testing user events

We won't be able to access functions inside the component so we should imitate the user behavior that leads to that functions being called. To do this we are using `userEvent` library.

```jsx
const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
const commonPill = screen.getByTestId('pill-common')
await userEvent.click(within(commonPill).getByRole('button'))
```

### Debugging

To debug the test and what is the current state of the dom in an specific time we can use `debug` function which is one of the properties returned when calling render

```jsx
const { debug } = render(<HelloWorld />)

debug()
```

### Full example of ui component test

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

describe('rarities filter', () => {
  it('should render rarities', () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions
    })
    expect(screen.getByText(Rarity.COMMON)).toBeInTheDocument()
    expect(screen.getByText(Rarity.EPIC)).toBeInTheDocument()
  })

  it('should call onBrowse without deleted rarity', async () => {
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

## Directory structure

Tests MUST be placed alongside the files or modules that they will exercise, with the same name, but with the extension `spec.ts` instead of `ts`.

A file structure MUST look like this:

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
