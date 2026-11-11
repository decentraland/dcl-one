# Testar Services (WKC)

Serviços de componentes bem conhecidos são servidores construídos com a [arquitetura de componentes bem conhecida](https://well-known-components.github.io/documentation/) arquitetura, em que cada parte do serviço é modularizada e encapsulada em um componente. Esses componentes, juntamente com a facilidade de serem trocados, têm a facilidade de serem testados unitariamente, já que todas as suas dependências são injetadas quando são criados.

Testar esses serviços DEVE ser feito usando dois tipos diferentes de teste, **unitário** e **integração**. Componentes que contêm lógica DEVEM ser testados usando testes unitários e componentes que têm principalmente interações com serviços externos DEVEM ser testados usando testes de integração. Ou seja, um componente adaptador de banco de dados que não contém nenhuma lógica e contém principalmente consultas ao banco de dados DEVE ser testado de forma integrada.

## Testes unitários de componentes da arquitetura bem conhecida

Todos os componentes DEVEM ser testados unitariamente e DEVEM ser escritos seguindo a seção definida aqui sobre como escrever testes.

Para mostrar como esses componentes são testados, aqui está um componente simples com apenas um método e duas dependências, **settings** e **friends**:

```tsx
// Componente de voz

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

Para testar esse componente, é preciso construir mocks adequados, ou seja, mocks que tenham os tipos corretos para interagir com o componente a ser testado e que sejam flexíveis para permitir que criemos todos os tipos de testes. Esses mocks também devem ser imutáveis entre testes, preservando a ideia de que cada teste é executado em seu próprio contexto, isolado dos demais.

Esses mocks DEVEM ser criados usando funções criadoras de mocks de componentes e DEVEM ser nomeados como create**ComponentName**MockedComponent. Tomando como exemplo o componente criado acima, aqui está um exemplo de como DEVEM ficar:

```tsx
// Colocado no arquivo /test/mocks/settings.ts
export function createSettingsMockedComponent(overrides?: Partial<jest.Mocked<ISettingsComponent>>): jest.Mocked<ISettingsComponent> {
  return {
    getPrivacySettings: overrides?.getPrivacySettings ?? jest.fn()
  }
}

// Colocado no arquivo test/mocks/friends.ts
export function createFriendsMockedComponent(overrides?: Partial<jest.Mocked<IFriendsComponent>>): jest.Mocked<IFriendsComponent> {
	return {
	  // Temos a flexibilidade de definir comportamento em qualquer um dos métodos exportados pelo componente.
	  areUsersFriends: overrides?.areUsersFriends ?? jest.fn(),
	  // Métodos que não são usados podem ser omitidos no parâmetro overrides, tornando-os um simples mock de função.
	  hasBlockedUser: overrides?.hasBlockedUser ?? jest.fn()
	}
}
```

Ao usar esses mocks cuidadosamente colocados dentro de um `beforeEach`, podemos ter certeza de que os mocks são definidos corretamente e isolados de diferentes execuções. Aqui está um exemplo de como esses mocks podem ser usados:

```tsx
// Define as funções mockadas no contexto para que possamos alterar seu mock.
let getPrivacySettingsMock: jest.MockedFn<ISettingsComponent['getPrivacySettings']>
let areUsersFriendsMock: jest.MockedFn<IFriendsComponent['areUsersFriends']>
let voice: IVoiceComponent

beforeEach({
  getPrivacySettingsMock = jest.fn()
  areUsersFriendsMock = jest.fn()
  // Constrói o mock
  const settings = createSettingsMockedComponent({ getPrivacySettings: getPrivacySettingsMock })
  // Inicializa o mock do componente com apenas os métodos que serão usados nos testes
  const friends = createFriendsMockedComponent({ areUsersFriends: areUsersFriendsMock })
  // Cria o componente a ser testado usando os mocks
  voice = createVoiceComponent({ settings, friends })
})
```

Essa forma de definir os mocks não só nos permite isolar corretamente os contextos dos testes entre execuções, como também cria as bases para uma inicialização adequada do contexto, já que, tendo as funções mockadas já definidas, é bem fácil construir cada contexto:

```tsx
// Definido após o bloco de código anterior.

describe('quando verificar se dois usuários podem ligar um para o outro', () => {
	const calleeAddress = '0xd7D746d39D142b6bE752efd7626cE28F245a25D1'
	const callerAddress = '0x2e8b4De1230f827082202aa53d489A26163aace0'

  describe('e o callee só aceita chamadas de amigos', () => {
    beforeEach(() => {
      // Define o mock para o contexto, tornando o callee 
      getPrivacySettingsMock.mockImplementation((address: string) => {
	      switch(address) {
	        case caleeAddress:
		        return Settings.ONLY_FRIENDS
		      case callerAddress:
			      return Settings.ALL
			    default:
				    throw new Error("Mock errado")
	      }
      })
    })
    
    describe('e o callee e o caller são amigos', () => {
	    beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(true)
	    })
	    
	    it('deve resolver para true', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(true)
	    })
    })
    
    describe('e o callee e o caller não são amigos', () =>{
		  beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(false)
	    })
	    
	    it('deve resolver para false', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(false)
	    })
    })
  })
  
  //... outros contextos
})
```

## Testes de integração

Todos os endpoints, chamadas WS RPC, jobs ou task DEVEM ser testados de forma integral e DEVEM ser escritos seguindo a seção definida aqui sobre como escrever testes. Os testes de integração devem ser colocados no `test/integration` diretório e devem ser nomeados de acordo com o ponto de entrada que queremos testar.

Os testes de integração devem se limitar a testar condições específicas de integração que não podem ser testadas com testes unitários (consultas SQL, operações Redis etc.) e a integração entre nossos componentes de lógica e o protocolo (HTTP, WS).

Para mostrar um exemplo de como os testes de integração devem ser feitos, vamos testar uma solicitação HTTP simples que recupera os amigos de um usuário.

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
			throw InvalidRequest(isErrorWithMessage(error) ? error.message : 'Erro desconhecido')
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

E um componente friends com o seguinte:

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
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
