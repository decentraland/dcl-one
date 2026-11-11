# Testar UIs

Esta secção descreve como os developers irão testar a base de código relacionada com a UI.

## Stack de testes

Todos os nossos testes de UI DEVEM ser feitos usando [Jest](https://jestjs.io/) como o principal framework de testes com [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan) como ferramenta de teste de Sagas e [React Testing Library](https://testing-library.com/docs/react-testing-library/intro/) como ferramenta de teste de componentes UI. O código do teste DEVE ser escrito usando [TypeScript](https://www.typescriptlang.org/) e executado usando [ts-jest](https://github.com/kulshekhar/ts-jest) para ter suporte de verificação de tipos.

## O que testar

* componentes UI
* Sagas
* Reducers
* Selectors
* Criadores de ações
* Funções utilitárias

## Testando Sagas

As Sagas permitem-nos lidar com side-effects ao permitir criar generators que tratam ações Redux. Uma das principais vantagens das Sagas é a sua testabilidade, uma vez que os effects amplamente usados em Sagas são altamente testáveis.

Estes handlers DEVEM ser testados usando [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan), e o teste DEVE exercitar toda a execução, desde o handler do módulo inteiro (o seu handler principal ou Sagas) até toda a execução dos handlers que irão reagir à ação que vamos despachar. Esta abordagem torna os testes de Sagas fáceis de construir e manter, e acrescenta valor aos testes, uma vez que percorrem o máximo possível de todo o fluxo.

Os testes dos handlers DEVEM ser testados através da sua interação com os effects, principalmente com o `put` um, pois o seu propósito é tratar ações e produzir novas. A maioria dos testes certamente conterá um `provide` call, usado para simular effects, um ou vários `put` calls, para verificar que os handlers despacham as ações como esperado e um único `dispatch` seguido por um `run` call para executar todas as Sagas até à conclusão.

A simulação de recursos externos ao módulo que está a ser testado DEVE ser feita usando os providers do redux-saga-test-plan, tirando partido dos diferentes effects fornecidos pelo redux-sagas. Effects como call, apply ou outros DEVEM ser usados sempre que possível quando uma função é chamada num teste, para permitir simulá-las facilmente. O Jest NÃO DEVE ser usado para simular módulos ou funções.

O texto no describe e os seus conteúdos DEVEM seguir estas diretrizes:

* O describe principal de uma Saga ou de um handler principal DEVE começar com *ao tratar*, indicando que o que vai ser testado é um handler. Após a *ao tratar* frase, deve ser escrita uma descrição da ação (ou da intenção da ação) a ser tratada. Ex.: *ao tratar a ação que sinaliza um fetch bem-sucedido.*
* Os describe internos irão diferenciar contextos de execução; seguem as mesmas regras descritas na [Descrição e construção do contexto](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) secção.
* O **it**Os s DEVEM descrever o que é esperado do teste, começando com um *should put* e a descrição das ações que devem ser despachadas ao executar o handler.

### Exemplo de Sagas

```tsx
...
export function* tiersSaga(builder: BuilderAPI) {
	function* handleBuyThirdPartyItemTierRequest(action: BuyThirdPartyItemTiersRequestAction) {
    const { thirdParty, tier } = action.payload
    try {
			// Obter o chain id pode falhar; deve ser criado um teste para garantir que este cenário é exercitado
      const maticChainId: ChainId = yield call(getChainIdByNetwork, Network.MATIC)
      const thirdPartyContract = yield call(getContract, ContractName.ThirdPartyRegistry, maticChainId)
			// O envio da transação pode ter sucesso ou falhar; ambos os casos devem ser exercitados num teste
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

### Exemplo de teste de Sagas

```tsx
// O describe principal começa com 'ao tratar...'
describe('ao tratar o pedido para comprar um nível de item de third party', () => {
	// Todos os casos, negativos e positivos, são exercitados
  describe('e o chain id não pôde ser obtido', () => {
    it('deve despachar a ação que sinaliza a falha da compra de um nível de item com o nível e o id do third party', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
				// As chamadas de provide simulam os diferentes effects, construindo diferentes cenários
        .provide([[call(getChainIdByNetwork, Network.MATIC), Promise.reject(new Error(defaultError))]])
				// As chamadas de put verificam que o resultado final esperado aconteceu
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
				// É feito um único dispatch por teste para exercitar o tratamento da ação que estamos a testar
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('e o envio da transação falhar', () => {
    it('deve despachar a ação que sinaliza a falha da compra de um nível de item com o nível e o id do third party', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
					// O call corresponde ao effect exato de call, tendo em conta os parâmetros
					// Isto faz com que o nosso teste garanta que o código executado antes de um put fez o que esperávamos
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
					// Os matchers são usados para corresponder apenas à função e não aos parâmetros
          [matchers.call.fn(sendTransaction), Promise.reject(new Error(defaultError))]
        ])
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('e o envio da transação for bem-sucedido', () => {
    let contract: any
    beforeEach(() => {
      contract = { buyItemSlots: jest.fn() }
    })

    it('deve despachar a ação que sinaliza a compra bem-sucedida de um nível de item com o nível, o third party e os detalhes da transação', () => {
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

## Testando Reducers

Os Reducers são funções que recebem um estado e uma ação e devolvem um novo estado. Este novo estado PODE conter alterações programadas para ocorrer para a ação dada.

Os testes DEVEM variar os estados iniciais e os diferentes parâmetros da ação a ser testada, se necessário. As asserções DEVEM ser feitas sobre todo o estado devolvido, uma vez que uma ação pode modificar qualquer parte do estado inicial dado.

Quando reduzimos uma ação, DEVEMOS usar o action creator, tornando os testes mais fáceis de manter.

Para padronizar a forma como escrevemos **describe**s e **it**s:

* Os describes principais DEVEM começar por descrever quais ações vão ser reduzidas usando a frase *ao reduzir a ação* seguida por uma descrição da ação. NÃO DEVEMOS usar o tipo da ação para descrever a ação a ser testada, uma vez que o tipo pode mudar.
* O **it**s DEVEM descrever de forma clara a maneira como o estado devolvido mudou. Ex.: *deve devolver um estado com o erro nulo e os fruits definidos*.

### Exemplo de Reducer

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

### Exemplo de teste de Reducer

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

describe('ao reduzir a ação que sinaliza a obtenção de fruits', () => {
	beforeEach(() => {
		// É definido um erro para garantir que ele seja limpo ao reduzir a ação.
		state.error = 'anError'
	})

	it('deve devolver um estado onde o erro é limpo e o estado de loading é definido como loading', () => {
		// Os action creators são usados para testar o reducer, para que ele possa ser mantido com facilidade.
		const action = fetchFruitsRequest()

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error: null,
			/* Como o reducer de loading faz parte de outro reducer (e não precisa de ser testado),
				 podemos optar por usá-lo tal como está ou podemos usar um helper que o emule.
			*/
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('ao reduzir a ação que sinaliza a falha da obtenção de fruits', () => {
	const error = 'anError'

	it('deve devolver um estado onde o erro é definido e o estado de loading é limpo da ação de fetch de fruits', () => {
		const action = fetchFruitsFailure(error)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error,
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('ao reduzir a ação que sinaliza o sucesso da obtenção de fruits', () => {
	let fruits: string[]
	beforeEach(() => {
		fruits = ['maçãs', 'peras']
	})

	it('deve devolver um estado onde os fruits são definidos e o reducer de loading é limpo da ação de fetch de fruits', () => {
		const action = fetchFruitsSuccess(fruits)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			fruits,
			loading: loadingReducer(state.loading, action),
		})
	})
})
```

## Testando Selectors

Os Selectors são métodos que recebem um estado e devolvem uma secção desse estado ou uma secção transformada do estado.

Existem dois tipos de selectors, selectors memorizados e selectors normais ou comuns. Os selectors normais ou comuns DEVEM ser testados como qualquer outra função testada unitariamente e os selectors memorizados DEVEM ser testados usando `resultFunc` ou a função que recebe as partes memorizadas do estado e devolve o valor pretendido.

**Describe**s e **it**s DEVEM ser escritos como descrito na [Descrição e construção do contexto](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) secção.

### Exemplo de Selectors

```tsx
// Selector comum
export const collectionExists = (state: RootState, id: string): boolean => {
	return state.collections.data.some((collection) => collection.id === id)
}

// Selector memorizado usando reselect que devolve uma lista de collections melhoradas
export const getEnhancedCollections = createSelector(
	getCollections,
	getExtraData,
	(collections, extraData) => {
		return collections.map((collection) =>
			({ ...collection, ...extraData[collection[id]] }))
	}
)
```

### Exemplo de teste de Selectors

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

// Os selectors comuns são testados unitariamente como funções normais
describe('ao verificar se uma collection existe', () => {
	describe('e não há nenhuma collection com o id fornecido', () => {
		it('deve devolver false', () => {
			expect(collectionExists(state, "anotherId")).toBe(false)
		})
	})

	describe('e existe uma collection com o id fornecido', () => {
		it('deve devolver true', () => {
			expect(collectionExists(state, collection.id)).toBe(true)
		})
	})
})

describe('ao obter as collections melhoradas', () => {
	let enhacements: Record<string, Enhacement>

	describe('e não há dados para as melhorar', () => {
		beforeEach(() => {
			enhacements = {}
		})

		// Os selectors memorizados devem ser testados unitariamente usando resultFunc
		it('deve obter todas as collections sem as melhorar', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				collection
			])
		})
	})

	describe('e há dados para as melhorar', () => {
		beforeEach(() => {
			enhacements = { [collection.id]: { aNewProperty: true } }
		})

		it('deve obter todas as collections melhoradas', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				{ ...collection, ...enhacements[collection.id] }
			])
		})
	})
})
```

## Testando criadores de ações

Os criadores de ações são responsáveis por criar as ações que mais tarde serão processadas nos reducers. Embora os criadores de ações sejam normalmente simples, DEVEM ser testados para garantir que fazem o que esperamos.

**Describe**s e **it**s DEVEM ser escritos como descrito na [Descrição e construção do contexto](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) secção.

### Exemplo de criadores de ações

```tsx
export const BUY_THIRD_PARTY_ITEM_TIERS_REQUEST = '[Pedido] Comprar um nível de item de third party'
export const BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS = '[Sucesso] Comprar um nível de item de third party'
export const BUY_THIRD_PARTY_ITEM_TIERS_FAILURE = '[Falha] Comprar um nível de item de third party'

export const buyThirdPartyItemTiersRequest = (thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_REQUEST, { thirdParty, tier })
export const buyThirdPartyItemTiersSuccess = (txHash: string, chainId: ChainId, thirdParty: ThirdParty, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_SUCCESS, { thirdParty, tier, ...buildTransactionPayload(chainId, txHash, { tier, thirdParty }) })
export const buyThirdPartyItemTiersFailure = (error: string, thirdPartyId: string, tier: ThirdPartyItemTier) =>
  action(BUY_THIRD_PARTY_ITEM_TIERS_FAILURE, { error, thirdPartyId, tier })
```

### Exemplo de teste de criador de ações

```tsx
describe('ao criar a ação que sinaliza o início de um pedido de fetch de tiers', () => {
  it('deve devolver uma ação a sinalizar o início do fetch de tiers', () => {
    expect(fetchThirdPartyItemTiersRequest()).toEqual({ type: FETCH_THIRD_PARTY_ITEM_TIERS_REQUEST })
  })
})

describe('ao criar a ação que sinaliza o fetch bem-sucedido de tiers', () => {
  it('deve devolver uma ação a sinalizar o sucesso do fetch de tiers', () => {
    expect(fetchThirdPartyItemTiersSuccess([thirdPartyItemTier])).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_SUCCESS,
      payload: { tiers: [thirdPartyItemTier], error: undefined, meta: undefined }
    })
  })
})

describe('ao criar a ação que sinaliza o fetch sem sucesso de tiers', () => {
  it('deve devolver uma ação a sinalizar a falha do fetch de tiers', () => {
    expect(fetchThirdPartyItemTiersFailure(defaultError)).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_FAILURE,
      payload: { error: defaultError }
    })
  })
})
```

## Testando componentes UI

Para testar componentes UI estamos a usar [React Testing Library](https://testing-library.com/docs). Estamos a usar esta biblioteca em vez de outras opções como enzyme, pois permite-nos testar componentes tendo em conta a forma como o utilizador irá interagir com eles e não a implementação em si. Não iremos interagir com instâncias de componentes React; em vez disso, interagimos com elementos DOM da forma como são renderizados.

### O que testar

O objetivo deste teste de componentes UI é testar o comportamento do componente e a forma como o utilizador interage com ele. Em muitos casos pode haver componentes ligados ao redux store para obter algumas propriedades e chamar algumas ações. Normalmente tratamos isto criando um `container` ficheiro que irá interagir com o store. Estes ficheiros estão fora do âmbito deste tipo de teste. Estamos a testar como o componente se comporta com diferentes valores de props; não nos deve importar a origem desses valores (redux store, context, componente pai), mas sim a forma como influenciam a renderização final.

### Renderização

Cada teste deve começar por chamar a função render. Isto irá renderizar não só o componente que enviamos como parâmetro, mas também todos os filhos. Devemos evitar fazer isto na configuração do teste (`beforeEach`) porque isso pode causar problemas e fazer com que a árvore DOM tenha mais componentes do que queremos. Devemos guardar o resultado numa propriedade chamada **screen**. Ao fazer isto, conseguimos obter todas as propriedades de que precisamos no teste sem ter de alterar constantemente a desestruturação.

```jsx
it('deve renderizar o componente hello world', () => {
	const screen = render(<HelloWorld />)
})
```

Se o documento tiver vários testes e o componente tiver várias props, podemos criar uma função no topo do ficheiro que trate de definir props predefinidas e renderizar o componente.

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

### Consultas

As consultas são os métodos que a Testing Library vos dá para encontrar elementos na página. Existem diferentes [tipos de queries](https://testing-library.com/docs/queries/about/#priority) que podemos usar para aceder aos elementos. Devemos tentar usar sempre as consultas que são `Acessíveis para toda a gente` porque são as que refletem a experiência de todos os utilizadores (visuais/com rato e com tecnologia assistiva).

Essas consultas são `getByRole`, `getByLabelText`, `getByPlaceholderText`, `getByText`, `getByDisplayValue`. Se conseguirmos aceder ao elemento usando estas consultas, isso provavelmente significa que o componente não é acessível.

### Testando eventos do utilizador

Não vamos conseguir aceder a funções dentro do componente, por isso devemos imitar o comportamento do utilizador que leva a que essas funções sejam chamadas. Para isso estamos a usar `userEvent` biblioteca.

```jsx
const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
const commonPill = screen.getByTestId('pill-common')
await userEvent.click(within(commonPill).getByRole('button'))
```

### Depuração

Para depurar o teste e ver qual é o estado atual do DOM num determinado momento, podemos usar `debug` a função que é uma das propriedades devolvidas quando chamamos render

```jsx
const { debug } = render(<HelloWorld />)

debug()
```

### Exemplo completo de teste de componente UI

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

describe('filtro de raridades', () => {
  it('deve renderizar raridades', () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions
    })
    expect(screen.getByText(Rarity.COMMON)).toBeInTheDocument()
    expect(screen.getByText(Rarity.EPIC)).toBeInTheDocument()
  })

  it('deve chamar onBrowse sem a raridade removida', async () => {
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

## Estrutura de diretórios

Os testes DEVEM ser colocados ao lado dos ficheiros ou módulos que vão exercitar, com o mesmo nome, mas com a extensão `spec.ts` em vez de `ts`.

A estrutura de ficheiros DEVE ser assim:

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
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-uis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
