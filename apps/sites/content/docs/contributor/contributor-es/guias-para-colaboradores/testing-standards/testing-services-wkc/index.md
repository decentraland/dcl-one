# Pruebas de servicios (WKC)

Los servicios de components bien conocidos son servidores construidos con la [component bien conocido](https://well-known-components.github.io/documentation/) arquitectura, donde cada parte del servicio se modulariza y encapsula en un component. Estos components, junto con la capacidad de ser fáciles de intercambiar, tienen la capacidad de ser fáciles de probar unitariamente, ya que todas sus dependencias se inyectan al crearlos.

Las pruebas de estos servicios DEBEN hacerse usando dos tipos distintos de prueba, **unitarias** y **de integración**. Los components que contengan lógica DEBEN probarse usando pruebas unitarias y los components que tengan principalmente interacciones con servicios externos DEBEN probarse usando pruebas de integración. Es decir, un component adaptador de base de datos que no contenga ninguna lógica y que contenga principalmente consultas a la base de datos DEBE probarse de forma integrada.

## Pruebas unitarias de components bien conocidos

Todos los components DEBEN tener pruebas unitarias y DEBEN escribirse siguiendo la sección definida aquí sobre cómo escribir pruebas.

Para mostrar cómo se prueban estos components, aquí hay un component sencillo con solo un método y dos dependencias, **settings** y **friends**:

```tsx
// Component de voz

export function createVoiceComponent(dependencies: Pick<AppComponents, 'settings' | 'friends'>) {
	const { settings, friends } = dependencies

  async function canCallEachOther(caller: string, callee: string): Promise<boolean> {
	  const [callerSettings, calleeSetting] = Promise.all([settings.getPrivacySetting(caller), settings.getPrivacySetting(callee)])
	  const areFriends = await friends.areUsersFriends(caller, callee)
		return areFriends || (callerSettings !== Settings.ONLY_FRIENDS && calleeSettings !== Settings.ONLY_FRIENDS)
  }
  
  return {
	  createVoiceChat
  }
}
```

Para probar este component, se deben construir mocks adecuados, es decir, mocks que tengan los tipos correctos para interactuar con el component que se va a probar y que sean flexibles para permitirnos crear todo tipo de pruebas. Estos mocks también deben ser inmutables entre pruebas, preservando la idea de que cada prueba se ejecuta en su propio contexto, aislado de los demás.

Estos mocks DEBEN crearse usando funciones creadoras de mocks de component y DEBEN nombrarse como create**ComponentName**MockedComponent. Tomando como ejemplo el component creado arriba, aquí hay un ejemplo de cómo DEBERÍAN verse:

```tsx
// Colocado en el archivo /test/mocks/settings.ts
export function createSettingsMockedComponent(overrides?: Partial<jest.Mocked<ISettingsComponent>>): jest.Mocked<ISettingsComponent> {
  return {
    getPrivacySettings: overrides?.getPrivacySettings ?? jest.fn()
  }
}

// Colocado en el archivo test/mocks/friends.ts
export function createFriendsMockedComponent(overrides?: Partial<jest.Mocked<IFriendsComponent>>): jest.Mocked<IFriendsComponent> {
	return {
	  // Tenemos la flexibilidad de definir comportamiento sobre cualquiera de los métodos exportados por el component.
	  areUsersFriends: overrides?.areUsersFriends ?? jest.fn(),
	  // Los métodos que no se usen pueden omitirse en el parámetro overrides, convirtiéndolos en un mock de función simple.
	  hasBlockedUser: overrides?.hasBlockedUser ?? jest.fn()
	}
}
```

Usando estos mocks cuidadosamente colocados dentro de un `beforeEach`, podemos asegurarnos de que los mocks estén definidos correctamente y aislados de diferentes ejecuciones. Aquí hay un ejemplo de cómo se pueden usar estos mocks:

```tsx
// Define las funciones mockeadas en el contexto para que podamos cambiar su mock.
let getPrivacySettingsMock: jest.MockedFn<ISettingsComponent['getPrivacySettings']>
let areUsersFriendsMock: jest.MockedFn<IFriendsComponent['areUsersFriends']>
let voice: IVoiceComponent

beforeEach({
  getPrivacySettingsMock = jest.fn()
  areUsersFriendsMock = jest.fn()
  // Construye el mock
  const settings = createSettingsMockedComponent({ getPrivacySettings: getPrivacySettingsMock })
  // Inicializa el mock del component con solo los métodos que se van a usar en las pruebas
  const friends = createFriendsMockedComponent({ areUsersFriends: areUsersFriendsMock })
  // Crea el component que se va a probar usando los mocks
  voice = createVoiceComponent({ settings, friends })
})
```

Esta forma de definir los mocks no solo nos permite aislar correctamente los contextos de las pruebas entre ejecuciones, sino que también sienta las bases para una inicialización adecuada del contexto, ya que al tener las funciones mockeadas ya definidas, resulta bastante fácil construir cada contexto:

```tsx
// Definido después del bloque de código anterior.

describe('cuando se comprueba si dos usuarios pueden llamarse entre sí', () => {
	const calleeAddress = '0xd7D746d39D142b6bE752efd7626cE28F245a25D1'
	const callerAddress = '0x2e8b4De1230f827082202aa53d489A26163aace0'

  describe('y el callee solo acepta llamadas de friends', () => {
    beforeEach(() => {
      // Define el mock para el contexto, haciendo que el callee 
      getPrivacySettingsMock.mockImplementation((address: string) => {
	      switch(address) {
	        case caleeAddress:
		        return Settings.ONLY_FRIENDS
		      case callerAddress:
			      return Settings.ALL
			    default:
				    throw new Error("Mock incorrecto")
	      }
      })
    })
    
    describe('y el calle y el caller son friends', () => {
	    beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(true)
	    })
	    
	    it('debería resolverse en true', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(true)
	    })
    })
    
    describe('y el callee y el caller no son friends', () =>{
		  beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(false)
	    })
	    
	    it('debería resolverse en false', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(false)
	    })
    })
  })
  
  //... otros contextos
})
```

## Pruebas de integración

Todos los endpoints, llamadas WS RPC, jobs o tareas DEBEN probarse íntegramente y DEBEN escribirse siguiendo la sección definida aquí sobre cómo escribir pruebas. Las pruebas de integración deben colocarse bajo el `test/integration` directorio y deben nombrarse de acuerdo con el entry point que queremos probar.

Las pruebas de integración deben limitarse a probar ya sea condiciones específicas de integración que no pueden probarse con pruebas unitarias (consultas SQL, operaciones con Redis, etc.) y la integración entre nuestros components de lógica y el protocolo (HTTP, WS).

Para mostrar un ejemplo de cómo deben hacerse las pruebas de integración, vamos a probar una simple petición HTTP que recupera los friends de un usuario.

```tsx
export async function getFriendHandler(
  context: Pick<
    HandlerContextWithPath<'logs' | 'communities', '/v1/users/:user/friends'>,
    'url' | 'components' | 'params' | 'verification'
  >
): Promise<HTTPResponse<AggregatedCommunityWithMemberData>> {
  const {
    components: { friends },
    params: { id },
    verification
  } = context

  try {
    const userAddress = verification!.auth.toLowerCase()

    return {
      status: 200,
      body: {
        data: await friends.getFriends(userAddress)
      }
    }
  } catch (error) {
  
	  if (error instanceof InvalidFriendshipRequest) {
			throw InvalidRequest(isErrorWithMessage(error) ? error.message : 'Error desconocido')
	  }

    return {
      status: 500,
      body: {
        message
      }
    }
  }
}
```

Y un component friends que tenga lo siguiente:

```tsx
async function getFriends(addres: string) {
	friendsDb.getFriends()
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
