# Pruebas de UI

Esta sección describe cómo los desarrolladores probarán la base de código relacionada con la UI.

## Pila de pruebas

Todas nuestras pruebas de UI DEBEN realizarse usando [Jest](https://jestjs.io/) como el framework principal de pruebas con [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan) como herramienta de pruebas de sagas y [React Testing Library](https://testing-library.com/docs/react-testing-library/intro/) como herramienta de pruebas de componentes de UI. El código de la prueba DEBE escribirse usando [TypeScript](https://www.typescriptlang.org/) y ejecutarse usando [ts-jest](https://github.com/kulshekhar/ts-jest) para tener soporte de comprobación de tipos.

## Qué probar

* Componentes de UI
* Sagas
* Reducers
* Selectores
* Creadores de acciones
* Funciones de utilidad

## Pruebas de sagas

Sagas nos permite manejar efectos secundarios al permitirnos crear generadores que manejan acciones de redux. Una de las principales ventajas de las sagas es su capacidad de prueba, ya que los effects que se usan ampliamente en las sagas son altamente testeables.

Estos handlers DEBEN probarse usando [redux-saga-test-plan](https://github.com/jfairbank/redux-saga-test-plan)y la prueba DEBE ejercitar toda la ejecución, desde el handler completo del módulo (su handler principal o sagas) hasta toda la ejecución de los handlers que reaccionarán a la acción que vamos a despachar. Este enfoque hace que las pruebas de sagas sean fáciles de construir y mantener, y aporta valor a las pruebas, ya que recorren tanto como sea posible todo el flujo.

Las pruebas de los handlers DEBEN verificarse a través de su interacción con los effects, principalmente con el `put` uno, ya que su propósito es manejar acciones y producir nuevas. La mayoría de las pruebas seguramente contendrá un `provide` call, usado para simular effects, uno o varios `put` calls, para verificar que los handlers despachan las acciones como se espera y un único `dispatch` seguido de un `run` call para ejecutar todas las sagas hasta su finalización.

La simulación de recursos externos al módulo que se está probando DEBERÍA hacerse usando los providers de redux-saga-test-plan, aprovechando los distintos effects proporcionados por redux-sagas. Effects como call, apply u otros DEBERÍAN usarse siempre que sea posible cuando se llama a una función en una prueba para permitir simularlos fácilmente. Jest NO DEBERÍA usarse para simular módulos o funciones.

El texto en el describe y sus pruebas DEBE seguir estas pautas:

* El describe principal de una saga o handler principal DEBE comenzar con *al manejar*, indicando que lo que se va a probar es un handler. Siguiendo la *al manejar* frase, DEBE escribirse una descripción de la acción (o de la intención de la acción) a manejar. P. ej. *al manejar la acción que señala una obtención exitosa.*
* Los describes internos diferenciarán los contextos de ejecución; siguen las mismas reglas que se describen en la [Describir y construir contexto](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) sección.
* El **it**s DEBERÍAN describir lo que se espera de la prueba, comenzando con un *debería despachar* y la descripción de las acciones que deberían despacharse al ejecutar el handler.

### Ejemplo de sagas

```tsx
...
export function* tiersSaga(builder: BuilderAPI) {
	function* handleBuyThirdPartyItemTierRequest(action: BuyThirdPartyItemTiersRequestAction) {
    const { thirdParty, tier } = action.payload
    try {
			// Obtener el chain id puede fallar; debe hacerse una prueba para asegurar que se ejerciten estos escenarios
      const maticChainId: ChainId = yield call(getChainIdByNetwork, Network.MATIC)
      const thirdPartyContract = yield call(getContract, ContractName.ThirdPartyRegistry, maticChainId)
			// Enviar la transacción puede tener éxito o fallar; ambos casos deben ejercerse en una prueba
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

### Ejemplo de prueba de sagas

```tsx
// El describe principal comienza con 'al manejar...'
describe('al manejar la solicitud para comprar un nivel de ítem de un tercero', () => {
	// Se ejercitan todos los casos, negativos y positivos
  describe('y no se pudo obtener el chain id', () => {
    it('debería despachar la acción que señala el fallo de la compra de un nivel de slots de ítem con el nivel y el id del tercero', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
				// provide simula los distintos effects, construyendo diferentes escenarios
        .provide([[call(getChainIdByNetwork, Network.MATIC), Promise.reject(new Error(defaultError))]])
				// Las llamadas a put verifican que ocurrió el resultado final esperado
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
				// Se realiza un único dispatch por prueba para ejercitar el manejo de la acción que estamos probando
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('y al enviar la transacción falla', () => {
    it('debería despachar la acción que señala el fallo de la compra de un nivel de slots de ítem con el nivel y el id del tercero', () => {
      return expectSaga(tiersSaga, mockedBuilderApi)
        .provide([
					// Call coincide con el effect call exacto, teniendo en cuenta los parámetros
					// Esto hace que nuestra prueba garantice que el código ejecutado antes de un put hizo lo que esperábamos
          [call(getChainIdByNetwork, Network.MATIC), ChainId.MATIC_MUMBAI],
          [call(getContract, ContractName.ThirdPartyRegistry, ChainId.MATIC_MUMBAI), contract],
					// Los matchers se usan para coincidir solo con la función y no con los parámetros
          [matchers.call.fn(sendTransaction), Promise.reject(new Error(defaultError))]
        ])
        .put(buyThirdPartyItemTiersFailure(defaultError, thirdParty.id, thirdPartyItemTier))
        .dispatch(buyThirdPartyItemTiersRequest(thirdParty, thirdPartyItemTier))
        .run({ silenceTimeout: true })
    })
  })

  describe('y al enviar la transacción tiene éxito', () => {
    let contract: any
    beforeEach(() => {
      contract = { buyItemSlots: jest.fn() }
    })

    it('debería despachar la acción que señala la compra exitosa de un nivel de slots de ítem con el nivel, el tercero y los detalles de la transacción', () => {
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

## Pruebas de reducers

Los reducers son funciones que toman un state y una action y devuelven un nuevo state. Este nuevo state PUEDE contener cambios programados para ocurrir para la action dada.

Las pruebas DEBERÍAN variar los estados iniciales y los diferentes parámetros de la action a probar, si es necesario. Las aserciones DEBEN hacerse sobre todo el state devuelto, ya que una action podría modificar cualquier parte del state inicial dado.

Al reducir una action, DEBERÍAMOS usar el action creator, lo que hace que las pruebas sean más fáciles de mantener.

Para estandarizar la forma en que escribimos **describe**s y **it**s:

* Los describes principales DEBERÍAN comenzar describiendo qué actions van a reducirse usando la frase *al reducir la action* seguida de una descripción de la action. NO DEBERÍAMOS usar el tipo de action para describir la action a probar, ya que el type podría cambiar.
* El **it**s DEBERÍAN describir de forma clara cómo cambió el state devuelto. P. ej.: *debería devolver un state con el error en null y las fruits establecidas*.

### Ejemplo de reducer

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

### Ejemplo de prueba de reducers

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

describe('al reducir la action que señala la obtención de frutas', () => {
	beforeEach(() => {
		// Se establece un error para asegurar que se limpie al reducir la action.
		state.error = 'anError'
	})

	it('debería devolver un state donde el error se limpia y el loading state se establece como loading', () => {
		// Se usan action creators para probar el reducer, de modo que pueda mantenerse con facilidad.
		const action = fetchFruitsRequest()

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error: null,
			/* Como el loading reducer forma parte de otro reducer (y no necesita probarse),
				 podemos elegir usarlo tal cual o podemos usar un helper que lo emule.
			*/
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('al reducir la action que señala el fallo al obtener frutas', () => {
	const error = 'anError'

	it('debería devolver un state donde el error se establece y el loading state se limpia a partir de la action de obtención de frutas', () => {
		const action = fetchFruitsFailure(error)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			error,
			loading: loadingReducer(state.loading, action),
		})
	})
})

describe('al reducir la action que señala el éxito al obtener frutas', () => {
	let fruits: string[]
	beforeEach(() => {
		fruits = ['apples', 'pears']
	})

	it('debería devolver un state donde las fruits se establecen y el loading reducer se limpia a partir de la action de obtención de frutas', () => {
		const action = fetchFruitsSuccess(fruits)

		expect(fruitsReducer(state, action)).toEqual({
			...state,
			fruits,
			loading: loadingReducer(state.loading, action),
		})
	})
})
```

## Pruebas de selectors

Los selectors son métodos que toman un state y devuelven una sección de ese state o una sección transformada del state.

Hay dos tipos de selectors, selectors memoizados y selectors normales o comunes. Los selectors normales o comunes DEBEN probarse como cualquier otra función probada de forma unitaria y los selectors memoizados DEBERÍAN probarse usando el `resultFunc` o la función que recibe las partes memoizadas del state y devuelve el valor deseado.

**Describe**s y **it**s DEBEN escribirse como se describe en la [Describir y construir contexto](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) sección.

### Ejemplo de selectors

```tsx
// Selector común
export const collectionExists = (state: RootState, id: string): boolean => {
	return state.collections.data.some((collection) => collection.id === id)
}

// Selector memoizado que usa reselect y devuelve una lista de collections mejoradas
export const getEnhancedCollections = createSelector(
	getCollections,
	getExtraData,
	(collections, extraData) => {
		return collections.map((collection) =>
			({ ...collection, ...extraData[collection[id]] }))
	}
)
```

### Ejemplo de prueba de selectors

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

// Los selectors comunes se prueban unitariamente como funciones normales
describe('al comprobar si existe una colección', () => {
	describe('y no hay una colección con el id dado', () => {
		it('should return false', () => {
			expect(collectionExists(state, "anotherId")).toBe(false)
		})
	})

	describe('y hay una colección con el id dado', () => {
		it('should return true', () => {
			expect(collectionExists(state, collection.id)).toBe(true)
		})
	})
})

describe('al obtener las collections mejoradas', () => {
	let enhacements: Record<string, Enhacement>

	describe('y no hay datos para mejorarlas', () => {
		beforeEach(() => {
			enhacements = {}
		})

		// Los selectors memoizados deberían probarse unitariamente usando resultFunc
		it('should retrieve all the collections without enhancing them', () => {
			expect(getEnhancedCollections.resultFunc(collections, enhacements)).toEqual([
				collection
			])
		})
	})

	describe('y hay datos para mejorarlas', () => {
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

## Pruebas de action creators

Los action creators se encargan de crear las actions que más tarde serán procesadas en los reducers. Aunque los action creators suelen ser simples, DEBEN probarse para asegurar que hacen lo que esperamos.

**Describe**s y **it**s DEBEN escribirse como se describe en la [Describir y construir contexto](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context) sección.

### Ejemplo de action creators

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

### Ejemplo de prueba de action creators

```tsx
describe('al crear la action que señala el inicio de una solicitud de obtención de tiers', () => {
  it('debería devolver una action que señala el inicio de la obtención de tiers', () => {
    expect(fetchThirdPartyItemTiersRequest()).toEqual({ type: FETCH_THIRD_PARTY_ITEM_TIERS_REQUEST })
  })
})

describe('al crear la action que señala la obtención exitosa de tiers', () => {
  it('debería devolver una action que señala el éxito de la obtención de tiers', () => {
    expect(fetchThirdPartyItemTiersSuccess([thirdPartyItemTier])).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_SUCCESS,
      payload: { tiers: [thirdPartyItemTier], error: undefined, meta: undefined }
    })
  })
})

describe('al crear la action que señala la obtención fallida de tiers', () => {
  it('debería devolver una action que señala el fallo de la obtención de tiers', () => {
    expect(fetchThirdPartyItemTiersFailure(defaultError)).toEqual({
      type: FETCH_THIRD_PARTY_ITEM_TIERS_FAILURE,
      payload: { error: defaultError }
    })
  })
})
```

## Pruebas de componentes de UI

Para probar componentes de UI estamos usando [React Testing Library](https://testing-library.com/docs). Estamos usando esta biblioteca en lugar de otras opciones como enzyme, ya que nos permite probar componentes teniendo en cuenta cómo interactuará el usuario con ellos en lugar de la propia implementación. No interactuaremos con instancias de componentes de React; en su lugar, interactuaremos con elementos del DOM del modo en que se renderizan.

### Qué probar

El propósito de esta prueba de componentes de UI es probar el comportamiento del componente y cómo interactúa el usuario con él. En muchos casos puede haber componentes conectados al Redux store para obtener algunas propiedades y llamar a algunas actions. Normalmente manejamos esto creando un `container` archivo aparte que interactuará con el store. Estos archivos están fuera del alcance de este tipo de pruebas. Estamos probando cómo se comporta el componente con distintos valores de props; no debería importarnos el origen de esos valores (redux store, context, parent component), sino cómo afectan al render final.

### Renderizado

Cada prueba debería comenzar llamando a la función render. Esto renderizará no solo el componente que enviamos como parámetro, sino también a todos los children. Deberíamos evitar hacer esto en la configuración de la prueba (`beforeEach`) ya que puede causar problemas al hacer que el DOM tree tenga más componentes de los que queremos. Deberíamos guardar el resultado en una propiedad llamada **screen**. Hacer esto nos permitirá obtener todas las propiedades que necesitamos en la prueba sin tener que cambiar constantemente el destructuring.

```jsx
it('debería renderizar el componente Hello World', () => {
	const screen = render(<HelloWorld />)
})
```

Si el documento tendrá múltiples pruebas y el componente tiene múltiples props, podemos crear una función en la parte superior del archivo que se encargue de establecer props predeterminadas y renderizar el componente.

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

Las consultas son los métodos que Testing Library te ofrece para encontrar elementos en la página. Existen diferentes [tipos de consultas](https://testing-library.com/docs/queries/about/#priority) que podemos usar para acceder a los elementos. Deberíamos intentar usar siempre las consultas que sean `accesibles para todos` ya que son las que reflejan la experiencia de todos los usuarios (visual/ratón y usando tecnología de asistencia).

Esas consultas son `getByRole`, `getByLabelText`, `getByPlaceholderText`, `getByText`, `getByDisplayValue`. Si podemos acceder al elemento usando estas consultas, probablemente significa que el componente no es accesible.

### Pruebas de eventos de usuario

No podremos acceder a las funciones dentro del componente, así que deberíamos imitar el comportamiento del usuario que lleva a que se llamen esas funciones. Para ello usamos `userEvent` biblioteca.

```jsx
const screen = renderSelectedFilters({
      browseOptions: raritiesOptions,
      onBrowse: onBrowseMock
    })
const commonPill = screen.getByTestId('pill-common')
await userEvent.click(within(commonPill).getByRole('button'))
```

### Depuración

Para depurar la prueba y ver cuál es el estado actual del DOM en un momento específico, podemos usar `debug` la función debug, que es una de las propiedades devueltas al llamar a render

```jsx
const { debug } = render(<HelloWorld />)

debug()
```

### Ejemplo completo de prueba de componente de UI

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

describe('filtro de rarezas', () => {
  it('debería renderizar las rarezas', () => {
    const raritiesOptions = { rarities: [Rarity.COMMON, Rarity.EPIC] }
    const screen = renderSelectedFilters({
      browseOptions: raritiesOptions
    })
    expect(screen.getByText(Rarity.COMMON)).toBeInTheDocument()
    expect(screen.getByText(Rarity.EPIC)).toBeInTheDocument()
  })

  it('debería llamar a onBrowse sin la rareza eliminada', async () => {
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

## Estructura de directorios

Las pruebas DEBEN colocarse junto a los archivos o módulos que ejercitarán, con el mismo nombre, pero con la extensión `spec.ts` en lugar de `ts`.

La estructura de archivos DEBE verse así:

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
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-uis.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
