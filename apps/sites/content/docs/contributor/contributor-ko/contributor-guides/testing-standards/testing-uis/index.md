# UI 테스트

이 섹션에서는 개발자들이 UI와 관련된 코드베이스를 어떻게 테스트할지 설명합니다.

## 테스트 스택

모든 UI 테스트는 다음을 사용하여 반드시 수행해야 합니다 [Jest](https://jestjs.io/) 을 기본 테스트 프레임워크로 사용하고 [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan) 을 사가 테스트 도구로, 그리고 [react testing library](https://testing-library.com/docs/react-testing-library/intro/) 를 UI 컴포넌트 테스트 도구로 사용해야 합니다. 테스트 코드는 반드시 다음을 사용하여 작성되어야 합니다 [TypeScript](https://www.typescriptlang.org/) 그리고 다음을 사용하여 실행되어야 합니다 [ts-jest](https://github.com/kulshekhar/ts-jest) 하여 타입 검사 지원을 갖춰야 합니다.

## 무엇을 테스트할 것인가

* UI 컴포넌트
* 사가
* 리듀서
* 셀렉터
* 액션 생성자
* 유틸리티 함수

## 사가 테스트하기

사는 redux 액션을 처리하는 제너레이터를 만들어 부수 효과를 다룰 수 있게 해줍니다. 사가의 주요 장점 중 하나는 테스트 용이성이며, 사가에서 널리 사용되는 이펙트들은 매우 테스트하기 쉽습니다.

이 핸들러는 반드시 다음을 사용하여 테스트해야 합니다 [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan), 그리고 테스트는 전체 실행 흐름을 검증해야 합니다. 즉, 모듈의 전체 핸들러(주 핸들러 또는 사가)부터 우리가 디스패치하려는 액션에 반응할 모든 핸들러의 실행까지 모두 포함해야 합니다. 이 접근 방식은 사가 테스트를 만들고 유지하기 쉽게 해주며, 가능한 한 전체 흐름을 최대한 많이 거치므로 테스트에 가치를 더합니다.

핸들러 테스트는 이펙트와의 상호작용을 통해, 주로 다음을 통해 테스트해야 합니다 `put` 이들의 목적은 액션을 처리하고 새 액션을 만들어내는 것이기 때문입니다. 대부분의 테스트에는 분명 다음이 포함됩니다 `provide` 호출을 사용해 이펙트를 모킹하고, 하나 이상의 `put` 호출을 사용해 핸들러가 예상대로 액션을 디스패치하는지 검증하며, 그리고 단일 `dispatch` 다음에 이어지는 `run` 호출로 전체 사가를 완료될 때까지 실행합니다.

테스트 중인 모듈 외부의 리소스 모킹은 redux-saga-test-plan providers를 사용하여 해야 하며, redux-saga가 제공하는 다양한 이펙트를 활용해야 합니다. call, apply 같은 이펙트나 기타 이펙트는 테스트에서 함수가 호출될 때마다 쉽게 모킹할 수 있도록 가능한 한 사용해야 합니다. Jest는 모듈이나 함수를 모킹하는 데 사용해서는 안 됩니다.

describe 안의 텍스트와 그 내용은 다음 지침을 따라야 합니다:

* 사가 또는 주 핸들러의 주요 describe는 다음으로 시작해야 합니다 *when handling*, 이는 테스트 대상이 핸들러임을 나타냅니다. 다음 *when handling* 문구 뒤에는 처리될 액션에 대한 설명(또는 액션의 의도)을 작성해야 합니다. 예: *성공적인 가져오기를 알리는 액션을 처리할 때.*
* 내부 describe는 실행 컨텍스트를 구분하며, 다음에 설명된 것과 같은 규칙을 따릅니다 [컨텍스트 설명 및 구성](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) 섹션.
* 이 **it**s는 테스트에서 기대하는 바를 설명해야 하며, 다음으로 시작해야 합니다 *should put* 그리고 핸들러 실행 시 put되어야 하는 액션에 대한 설명이어야 합니다.

### 사가 예시

```tsx
...
export function* tiersSaga(builder: BuilderAPI) {
	function* handleBuyThirdPartyItemTierRequest(action: BuyThirdPartyItemTiersRequestAction) {
    const { thirdParty, tier } = action.payload
    try {
			// 체인 ID를 가져오는 데 실패할 수 있으므로, 이 시나리오가 실행되는지 보장하는 테스트를 만들어야 합니다
      const maticChainId: ChainId = yield call(getChainIdByNetwork, Network.MATIC)
      const thirdPartyContract = yield call(getContract, ContractName.ThirdPartyRegistry, maticChainId)
			// 트랜잭션 전송은 성공하거나 실패할 수 있으므로, 두 경우 모두 테스트에서 실행되어야 합니다
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

### 사가 테스트 예시

```tsx
// 주요 describe는 'when handling...'으로 시작합니다
describe('서드파티 아이템 티어 구매 요청을 처리할 때', () => {
	// 모든 경우, 실패와 성공 모두가 실행됩니다
  describe('체인 ID를 가져올 수 없을 때', () => {
    it('아이템 슬롯 티어 구매 실패를 알리는 액션을 tier와 서드파티 id와 함께 put해야 한다', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
				// provide 호출은 다양한 이펙트를 모킹해 서로 다른 시나리오를 구성합니다
        .provide([[call(getChainIdByNetwork, Network.MATIC), Promise.reject(new Error(defaultError))]])
				// put 호출은 기대한 최종 결과가 발생했는지 검증합니다
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
				// 테스트 대상 액션의 처리를 검증하기 위해 테스트당 단일 dispatch만 수행합니다
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('트랜잭션 전송이 실패할 때', () => {
    it('아이템 슬롯 티어 구매 실패를 알리는 액션을 tier와 서드파티 id와 함께 put해야 한다', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
					// call은 매개변수를 고려하여 정확한 call 이펙트와 일치해야 합니다
					// 이를 통해 put 이전에 실행된 코드가 우리가 기대한 대로 동작했는지 테스트로 보장할 수 있습니다
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
					// 매처는 함수만 일치시키고 매개변수는 일치시키지 않는 데 사용됩니다
          [matchers.call.fn(sendTransaction), Promise.reject(new Error(defaultError))]
        ])
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('트랜잭션 전송이 성공할 때', () => {
    let contract: any
    beforeEach(() => {
      contract = { buyItemSlots: jest.fn() }
    })

    it('티어, 서드파티 및 트랜잭션 세부 정보와 함께 아이템 슬롯 티어 구매 성공을 알리는 액션을 put해야 한다', () => {
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

## 리듀서 테스트하기

리듀서는 state와 action을 받아 새 state를 반환하는 함수입니다. 이 새 state에는 주어진 action에 대해 수행되도록 프로그래밍된 변경 사항이 포함될 수 있습니다.

테스트는 필요한 경우 초기 state와 테스트할 action의 다양한 매개변수를 달리해야 합니다. 액션이 주어진 초기 state의 어느 부분이든 수정할 수 있으므로, 검증은 반환된 전체 state에 대해 수행해야 합니다.

액션을 리듀싱할 때는 테스트를 더 쉽게 유지보수할 수 있도록 action creator를 사용하는 것이 좋습니다.

우리가 작성하는 **describe**와 **it**를 표준화하기 위해:

* 주요 describe는 다음 문구를 사용해 어떤 액션들이 리듀스될지 설명하는 것으로 시작해야 합니다 *when reducing the action* 그 뒤에 액션 설명이 이어집니다. 테스트할 액션을 설명할 때 action type을 사용해서는 안 되는데, type은 변경될 수 있기 때문입니다.
* 이 **it**반환된 state가 어떻게 변경되었는지를 명확하게 설명해야 합니다. 예: *error가 null로 초기화되고 fruits가 설정된 state를 반환해야 합니다*.

### 리듀서 예시

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

### 리듀서 테스트 예시

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

describe('과일 가져오기를 알리는 액션을 리듀싱할 때', () => {
	beforeEach(() => {
		// 액션을 리듀싱할 때 오류가 지워지도록 하기 위해 error를 설정합니다.
		state.error = 'anError'
	})

	it('error가 설정되고 fruits 가져오기 액션에서 loading state가 지워진 state를 반환해야 합니다', () => {
		// 리듀서를 테스트할 때 action creator를 사용하면 쉽게 유지보수할 수 있습니다.
		const action = fetchFruitsRequest()

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error: null,
			/* loading reducer는 다른 reducer의 일부이므로(그리고 테스트할 필요가 없으므로),
				 그대로 사용하거나 이를 에뮬레이트하는 헬퍼를 사용할 수 있습니다.
			*/
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('과일 가져오기 실패를 알리는 액션을 리듀싱할 때', () => {
	const error = 'anError'

	it('error가 설정되고 fruits 가져오기 액션에서 loading state가 지워진 state를 반환해야 합니다', () => {
		const action = fetchFruitsFailure(error)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error,
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('과일 가져오기 성공을 알리는 액션을 리듀싱할 때', () => {
	let fruits: string[]
	beforeEach(() => {
		fruits = ['apples', 'pears']
	})

	it('fruits가 설정되고 fruits 가져오기 액션에서 loading reducer가 지워진 state를 반환해야 합니다', () => {
		const action = fetchFruitsSuccess(fruits)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			fruits,
			loading: loadingReducer(state.loading, action),
		})
	})
})
```

## 셀렉터 테스트하기

셀렉터는 state를 받아 해당 state의 한 부분이나 변환된 state의 한 부분을 반환하는 메서드입니다.

셀렉터에는 메모이즈된 셀렉터와 일반 셀렉터 두 종류가 있습니다. 일반 셀렉터는 다른 단위 테스트 함수와 마찬가지로 테스트해야 하며, 메모이즈된 셀렉터는 다음을 사용하여 테스트해야 합니다 `resultFunc` 또는 메모이즈된 state 부분을 받아 원하는 값을 반환하는 함수.

**describe**와 **it**는 다음에 설명된 대로 작성해야 합니다 [컨텍스트 설명 및 구성](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) 섹션.

### 셀렉터 예시

```tsx
// 일반 셀렉터
export const collectionExists = (state: RootState, id: string): boolean => {
	return state.collections.data.some((collection) => collection.id === id)
}

// 향상된 컬렉션 목록을 반환하는 reselect를 사용하는 메모이즈된 셀렉터
export const getEnhancedCollections = createSelector(
	getCollections,
	getExtraData,
	(collections, extraData) => {
		return collections.map((collection) =>
			({ ...collection, ...extraData[collection[id]] }))
	}
)
```

### 셀렉터 테스트 예시

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

// 일반 셀렉터는 일반 함수처럼 단위 테스트합니다
describe('컬렉션이 있는지 확인할 때', () => {
	describe('주어진 id를 가진 컬렉션이 없을 때', () => {
		it('false를 반환해야 한다', () => {
			expect(collectionExists(state, "anotherId")).toBe(false)
		})
	})

	describe('주어진 id를 가진 컬렉션이 있을 때', () => {
		it('true를 반환해야 한다', () => {
			expect(collectionExists(state, collection.id)).toBe(true)
		})
	})
})

describe('향상된 컬렉션을 가져올 때', () => {
	let enhacements: Record<string, Enhacement>

	describe('컬렉션을 향상시킬 데이터가 없을 때', () => {
		beforeEach(() => {
			enhacements = {}
		})

		// 메모이즈된 셀렉터는 resultFunc를 사용해 단위 테스트해야 합니다
		it('향상시키지 않은 채 모든 컬렉션을 가져와야 한다', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				collection
			])
		})
	})

	describe('컬렉션을 향상시킬 데이터가 있을 때', () => {
		beforeEach(() => {
			enhacements = { [collection.id]: { aNewProperty: true } }
		})

		it('모든 향상된 컬렉션을 가져와야 한다', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				{ ...collection, ...enhacements[collection.id] }
			])
		})
	})
})
```

## 액션 생성자 테스트하기

액션 생성자는 나중에 리듀서에서 처리될 액션을 만드는 일을 담당합니다. 액션 생성자는 보통 단순하지만, 우리가 기대하는 대로 동작하는지 보장하기 위해 반드시 테스트해야 합니다.

**describe**와 **it**는 다음에 설명된 대로 작성해야 합니다 [컨텍스트 설명 및 구성](/docs/contributor/contributor-ko/contributor-guides/testing-standards/writing-tests/#describing-and-building-context) 섹션.

### 액션 생성자 예시

```tsx
export const BUY_THIRD_PARTY_ITEM_TIERS_REQUEST = '[요청] 서드파티 아이템 티어 구매'
export const BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS = '[성공] 서드파티 아이템 티어 구매'
export const BUY_THIRD_PARTY_ITEM_TIERS_FAILURE = '[실패] 서드파티 아이템 티어 구매'

export const buyThirdPartyItemTiersRequest = (thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_REQUEST, { thirdParty, tier })
export const buyThirdPartyItemTiersSuccess = (txHash: string, chainId: ChainId, thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS, { thirdParty, tier, ...buildTransactionPayload(chainId, txHash, { tier, thirdParty }) })
export const buyThirdPartyItemTiersFailure = (error: string, thirdPartyId: string, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_FAILURE, { error, thirdPartyId, tier })
```

### 액션 생성자 테스트 예시

```tsx
describe('티어 가져오기 요청의 시작을 알리는 액션을 만들 때', () => {
  it('티어 가져오기의 시작을 알리는 액션을 반환해야 한다', () => {
    expect(fetchThirdPartyItemTiersRequest()).toEqual({ type: FETCH_THIRD_PARTY_ITEM_TIERS_REQUEST })
  })
})

describe('티어 가져오기가 성공했음을 알리는 액션을 만들 때', () => {
  it('티어 가져오기의 성공을 알리는 액션을 반환해야 한다', () => {
    expect(fetchThirdPartyItemTiersSuccess([thirdPartyItemTier])).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_SUCCESS,
      payload: { tiers: [thirdPartyItemTier], error: undefined, meta: undefined }
    })
  })
})

describe('티어 가져오기가 실패했음을 알리는 액션을 만들 때', () => {
  it('티어 가져오기의 실패를 알리는 액션을 반환해야 한다', () => {
    expect(fetchThirdPartyItemTiersFailure(defaultError)).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_FAILURE,
      payload: { error: defaultError }
    })
  })
})
```

## UI 컴포넌트 테스트하기

UI 컴포넌트 테스트에는 다음을 사용합니다 [React Testing Library](https://testing-library.com/docs). 우리는 enzyme 같은 다른 옵션 대신 이 라이브러리를 사용하는데, 구현 자체가 아니라 사용자가 어떻게 상호작용하는지를 고려하여 컴포넌트를 테스트할 수 있게 해주기 때문입니다. 우리는 React 컴포넌트 인스턴스와 상호작용하는 것이 아니라, 렌더링된 방식 그대로 DOM 요소와 상호작용합니다.

### 무엇을 테스트할 것인가

이 UI 컴포넌트 테스트의 목적은 컴포넌트의 동작과 사용자가 그것과 어떻게 상호작용하는지를 테스트하는 것입니다. 많은 경우 어떤 속성을 얻고 일부 액션을 호출하기 위해 redux store에 연결된 컴포넌트가 있을 수 있습니다. 우리는 보통 이를 별도의 `container` 파일을 만들어 store와 상호작용하게 함으로써 처리합니다. 이러한 파일은 이런 종류의 테스트 범위 밖입니다. 우리는 컴포넌트가 서로 다른 props 값을 가지고 어떻게 동작하는지 테스트하며, 그 값의 출처(redux store, context, 부모 컴포넌트)는 중요하지 않고 최종 렌더링에 어떤 영향을 주는지만 중요합니다.

### 렌더링

모든 테스트는 render 함수를 호출하는 것으로 시작해야 합니다. 이렇게 하면 인자로 전달한 컴포넌트뿐 아니라 모든 자식도 렌더링됩니다. 이는 테스트 설정에서(`beforeEach`) 수행하는 것을 피해야 하는데, DOM 트리에 우리가 원하지 않는 더 많은 컴포넌트가 생길 수 있기 때문입니다. 결과는 다음 이름의 속성에 저장해야 합니다 **screen**. 이렇게 하면 구조 분해를 계속 바꿀 필요 없이 테스트에 필요한 모든 속성을 가져올 수 있습니다.

```jsx
it('hello world 컴포넌트를 렌더링해야 한다', () => {
	const screen = render(<HelloWorld />)
})
```

문서에 여러 테스트가 있고 컴포넌트에 여러 props가 있다면, 파일 상단에 기본 props를 설정하고 컴포넌트를 렌더링하는 함수를 만들 수 있습니다.

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

### 쿼리

쿼리는 Testing Library가 페이지의 요소를 찾기 위해 제공하는 메서드입니다. 다양한 [종류의 쿼리](https://testing-library.com/docs/queries/about/#priority) 가 있으며 이를 사용해 요소에 접근할 수 있습니다. 우리는 항상 `모든 사람이 접근할 수 있는` 쿼리를 사용하려고 해야 합니다. 왜냐하면 그것들이 시각/마우스 사용자와 보조 기술 사용자 모두의 경험을 반영하기 때문입니다.

이러한 쿼리는 다음과 같습니다 `getByRole`, `getByLabelText`, `getByPlaceholderText`, `getByText`, `getByDisplayValue`. 이 쿼리들을 사용해 요소에 접근할 수 있다면, 아마도 컴포넌트가 접근 가능하지 않다는 뜻일 것입니다.

### 사용자 이벤트 테스트하기

컴포넌트 내부의 함수에 접근할 수 없으므로, 그 함수가 호출되도록 이끄는 사용자의 행동을 모방해야 합니다. 이를 위해 다음을 사용합니다 `userEvent` 라이브러리.

```jsx
const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
const commonPill = screen.getByTestId('pill-common')
await userEvent.click(within(commonPill).getByRole('button'))
```

### 디버깅

특정 시점의 테스트와 현재 DOM 상태를 디버그하려면 다음을 사용할 수 있습니다 `debug` render를 호출할 때 반환되는 속성 중 하나인 함수

```jsx
const { debug } = render(<HelloWorld />)

debug()
```

### UI 컴포넌트 테스트 전체 예시

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

describe('희귀도 필터', () => {
  it('희귀도를 렌더링해야 한다', () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions
    })
    expect(screen.getByText(Rarity.COMMON)).toBeInTheDocument()
    expect(screen.getByText(Rarity.EPIC)).toBeInTheDocument()
  })

  it('삭제된 희귀도를 제외하고 onBrowse를 호출해야 한다', async () => {
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

## 디렉터리 구조

테스트는 파일이나 모듈과 동일한 이름을 사용하되 확장자는 다음과 같이 하여, 테스트할 파일이나 모듈과 나란히 배치해야 합니다 `spec.ts` 대신 `ts`.

파일 구조는 다음과 같아야 합니다:

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
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards/testing-uis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
