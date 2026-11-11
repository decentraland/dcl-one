> Como comunicar sua scene com servidores externos e APIs.

# Conexões de Rede

Sua cena pode aproveitar serviços externos que expõem APIs; você pode usar isso para obter dados de preços atualizados, dados meteorológicos ou qualquer outro tipo de informação exposta por uma API.

Você também pode configurar seu próprio servidor externo para ajudar sua cena e servir para sincronizar dados entre seus jogadores. Isso pode ser feito com um servidor que exponha uma REST API ou com um servidor que use WebSockets.

## Chamar uma REST API

O código da sua cena pode enviar chamadas para uma REST API para buscar dados.

Como o servidor pode demorar para enviar sua resposta, você deve executar este comando como uma [função assíncrona](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/), usando `executeTask()`.

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('não foi possível acessar a URL')
	}
})
```

O comando fetch também pode incluir um segundo argumento opcional que agrupa cabeçalhos, método HTTP e corpo HTTP em um único objeto.

* **url**: Endereço para enviar a solicitação
* **init**: Um `RequestInit` objeto que pode conter:
  * **method** : Método HTTP a ser usado (GET, POST, DELETE, etc)
  * **body**: Conteúdo do corpo da solicitação. Ele deve ser enviado como um objeto JSON serializado em string.
  * **headers**: Cabeçalhos adicionais a incluir na solicitação.
  * **redirect**: Estratégia de redirecionamento ('follow' | 'error' | 'manual')
  * **timeout**: Quanto tempo esperar por uma resposta antes que a solicitação falhe. Por padrão, 30000 milissegundos (30 segundos).

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl, {
			headers: { 'Content-Type': 'application/json' },
			method: 'POST',
			body: JSON.stringify(myBody),
		})
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('não foi possível acessar a URL')
	}
})
```

O comando fetch retorna um `objeto de resposta` com os seguintes dados:

* `headers`: Um `objeto ReadOnlyHeaders. Chame o método` get() `para obter um cabeçalho específico, ou o método` has() `para verificar se um cabeçalho está presente.` presente.
* `ok`: Boolean
* `redirected`: Boolean
* `status`: Número do código de status
* `statusText`: Texto do código de status
* `type`: Terá um dos seguintes valores: *basic*, *cors*, *default*, *error*, *opaque*, *opaqueredirect*
* `url`: URL que foi enviada
* `json()`: Obtenha o corpo no formato JSON.
* `text()`: Obtenha o corpo como texto.


> [!WARNING]
> **📔 Nota**: `json()` e `text()` são mutuamente exclusivos. Se você obtiver o corpo da resposta em um dos dois formatos, não poderá mais obter o outro da `objeto de resposta` objeto.



> [!WARNING]
> **📔 Nota**: Cada cena do Decentraland só pode executar um `fetch` comando por vez. Isso não tem efeito sobre como o código da cena deve ser estruturado, pois as solicitações são enfileiradas internamente. Se a sua cena exigir o envio de várias solicitações para diferentes endpoints, lembre-se de que cada solicitação só é enviada quando a anterior tiver recebido resposta.


## Requisições assinadas

Você pode empregar uma medida extra de segurança para certificar que uma requisição está se originando de uma sessão de jogador dentro do Decentraland. Você pode enviar suas requisições com uma assinatura adicional, assinada usando uma chave efêmera que a sessão do Decentraland gera para cada jogador com base no endereço do jogador. O servidor que recebe a requisição pode então verificar se a mensagem assinada realmente corresponde a um endereço que está atualmente ativo no mundo.

Esses tipos de medidas de segurança são especialmente valiosos quando pode haver um incentivo para um jogador abusar do sistema, para farmear tokens ou pontos em um jogo.

Para enviar uma requisição assinada, tudo o que você precisa fazer é usar a `signedFetch()` função, exatamente da mesma forma como você usaria a `fetch()` .

```ts
import { signedFetch } from '~system/SignedFetch'

executeTask(async () => {
	try {
		let response = await signedFetch({
			url: callUrl,
			init: {
				headers: { 'Content-Type': 'application/json' },
				method: 'POST',
				body: JSON.stringify(myBody),
			},
		})

		if (!response.ok) {
			throw new Error('Resposta inválida')
		}

		let json = JSON.parse(response.body)

		console.log('Resposta recebida: ', json)
	} catch {
		console.log('não foi possível acessar a URL')
	}
})
```

A requisição inclui uma série adicional de cabeçalhos, contendo uma mensagem assinada e um conjunto de metadados para interpretá-la. A mensagem assinada consiste em todo o conteúdo da requisição criptografado usando a chave efêmera do jogador.

O `signedFetch()` difere da `fetch()` função, pois a resposta é uma Promise de uma mensagem HTTP completa, expressa como um `FlatFetchResponse` objeto. Isso inclui as seguintes propriedades:

* `body`
* `headers`
* `ok`
* `status`
* `statusText`

O **body** é sempre uma string, que você pode analisar como no exemplo acima.

### Validando uma requisição assinada

Para fazer uso de requisições assinadas, o servidor que as recebe deve validar se as assinaturas correspondem ao restante da requisição e se o timestamp codificado dentro da mensagem assinada é atual.

Você pode encontrar um exemplo simples de um servidor executando essa tarefa na seguinte scene de exemplo:

[Validar a autenticidade do jogador](https://github.com/decentraland-scenes/validate-player-authenticity)

## Tempo limite da requisição

Se uma requisição HTTP demorar demais para receber resposta, ela falha para que outras requisições possam ser enviadas. Para `fetch()`, o limite padrão de timeout é de 30 segundos, mas você pode definir um valor diferente em cada requisição configurando a `timeout` propriedade timeout. O valor de `timeout` é expresso em milissegundos.

```ts
fetch('https://some-url.com', { timeout: 1000 })
```

## Usar WebSockets

Você também pode enviar e obter dados de um servidor WebSocket, desde que esse servidor use uma conexão segura com *wss*.

```ts
var socket = new WebSocket('url')

socket.onmessage = function (event) {
	console.log('Mensagem do WebSocket recebida:', event)
}
```

A sintaxe para usar WebSockets não é diferente da implementada nativamente pelo JavaScript. Veja a documentação da [Mozilla Web API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket) para saber os detalhes sobre como capturar e enviar mensagens por WebSockets.


> [!NOTE]
> **💡 Dica**: Uma biblioteca que simplifica o uso de conexões websocket e que comprovadamente funciona muito bem com o Decentraland é a [Colyseus](https://colyseus.io/). Várias outras bibliotecas websocket não são compatíveis com o SDK do Decentraland.
> 
> Ela cria uma camada de abstração sobre as conexões websocket que torna super fácil reagir a mudanças e armazenar remotamente no servidor um estado de jogo consistente. Você pode vê-la em ação nestes exemplos:
> 
> * [Cube Jumper](https://github.com/decentraland-scenes/cube-jumper-colyesus-sdk7)
> * [Space Traitor](https://github.com/decentraland-scenes/Space-Traitor)
> * [AI NPC](https://github.com/decentraland-scenes/inworld-ai-sdk7)


## Depuração de requisições de rede

Você pode depurar requisições de rede abrindo o Painel de Debug.

Para abrir o Painel de Debug, você pode clicar no ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) ícone no canto superior direito. Depois, selecione a **Solicitações Web** aba e clique **Abrir o Chrome Devtools**.

Isso abrirá uma nova janela do Chrome com a aba Network aberta.

Veja [Depurar no Preview](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview/#web-requests) para mais detalhes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/rede/network-connections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
