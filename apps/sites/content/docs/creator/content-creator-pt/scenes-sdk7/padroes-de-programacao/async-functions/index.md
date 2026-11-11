> Saiba quando e como executar funções assíncronas no código da sua scene.

# Funções Assíncronas

## Visão geral

A maior parte do código na sua cena é executada de forma síncrona usando uma única thread. Isso significa que os comandos são executados sequencialmente, linha por linha. Cada comando precisa primeiro esperar que o comando anterior termine de ser executado antes de poder começar.

Até mesmo as funções nos systems da sua cena são executadas uma por uma, seguindo uma [ordem de prioridade](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/#system-execution-order).

Executar código de forma síncrona garante consistência, pois você pode sempre ter certeza de que saberá a ordem em que os comandos no seu código são executados.

Por outro lado, sua cena precisa ser atualizada muitas vezes por segundo, construindo o próximo frame. Se uma parte do seu código demora demais para responder, então a thread principal inteira fica presa e isso resulta em taxas de frame lentas.

É por isso que, em alguns casos, você quer que alguns comandos sejam executados de forma assíncrona. Isso significa que você pode iniciar uma tarefa e, enquanto isso, a cena pode continuar executando as próximas linhas de código. A cena ainda executa tudo em uma única thread, mas enquanto a tarefa assíncrona aguarda uma resposta, o restante do código não fica bloqueado.

Isso é especialmente útil para tarefas que dependem de serviços externos que podem demorar para responder, pois você não quer que esse tempo ocioso de espera por essa resposta bloqueie outras tarefas.

Por exemplo:

* Ao recuperar dados de uma REST API
* Ao realizar uma transação na blockchain


> [!WARNING]
> **📔 Nota**: Tenha em mente que vários frames da sua cena podem ser renderizados antes que a tarefa termine de ser executada. Certifique-se de que o código da sua cena seja flexível o suficiente para lidar com os cenários intermediários enquanto a tarefa assíncrona está sendo concluída.


## Executar uma função async

Marque qualquer função como `async` para que ela seja executada de forma assíncrona, sem bloquear o restante do código da cena enquanto espera.

```ts
// declarar função async
async function myAsyncTask() {
	// executar etapas da função
}

// chamar função async
myAsyncTask()

// o restante do código continua sendo executado
```

## A função executeTask

A `executeTask()` função executa uma função lambda de forma assíncrona. `executeTask()` permite-nos declarar e executar a função tudo na mesma instrução.

```ts
executeTask(async () => {
	let data = await myAsyncTask()
	console.log(data)
})

// o restante do código continua sendo executado
```

## A função then

A `then` função recebe uma função lambda como argumento, que só é executada depois que a instrução anterior termina. Essa função lambda pode opcionalmente ter entradas mapeadas a partir do que a instrução anterior retorna.

```ts
myAsyncTask().then((data) => {
	console.log(data)
})
```


> [!WARNING]
> **📔 Nota**: Em geral, é melhor usar a `executeTask` abordagem em vez da `then` função. Se você depender demais da `then` função em vários níveis aninhados, você pode acabar com o que é conhecido como "callback hell", onde o código pode se tornar muito difícil de ler e manter.


## Funções PointerEvents e RayCast

Quando sua cena usa um `PointerEvent` ou um `RayCast` componente, os cálculos de colisões são realizados de forma async no engine. O engine então retorna um evento de resultados para a cena, que pode chegar um ou vários ticks do game loop depois de quando o evento foi invocado.

Você então precisa criar um sistema para processar esses resultados no frame em que eles chegarem.


> [!WARNING]
> **📔 Nota**: Se você lidar com cliques por meio da [**Registrar um callback**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) abordagem, você não precisa criar explicitamente um sistema para lidar com isso, mas o mesmo ocorre em segundo plano.


Veja [eventos de clique](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/) e [raycasting](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting/).


> [!NOTE]
> **💡 Dica**: Se o processamento dos resultados de um raycast exigir muitos cálculos (como executar um algoritmo de path-finding), talvez você queira executar esse cálculo em uma função assíncrona.


## A instrução await

Uma `await` instrução força a execução a esperar por uma resposta antes de avançar para a próxima linha de código. `await` instruções só podem ser usadas dentro de um bloco de código async.

```ts
// declarar função
async function myAsyncTask() {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('failed to reach the URL')
	}
}

// chamar função
myAsyncTask()

// O restante do código continua sendo executado
```

O exemplo acima executa uma função que inclui uma `fetch()` operação para recuperar dados de uma API externa. A `fetch()` operação é assíncrona, pois não podemos prever quanto tempo o servidor levará para responder. No entanto, a próxima linha precisa que a saída dessa operação esteja pronta antes de podermos analisá-la como JSON. A `await` instrução aqui garante que a próxima linha só será executada quando essa operação tiver retornado um valor. Da mesma forma, a `response.json()` função também é assíncrona, mas a próxima linha precisa que o JSON seja analisado antes de poder registrá-lo. A segunda `await` instrução força a próxima linha a ser chamada apenas quando a análise do JSON terminar, por mais tempo que isso leve.

## Definir um timeout para uma chamada de função

Usa `setTimeout` para esperar algum tempo antes que certas linhas de código sejam executadas. Isso requer dois argumentos:

* A função a executar
* A quantidade de milissegundos a esperar antes de executar essa função

O exemplo abaixo espera 1000 milissegundos (equivalente a 1 segundo) antes de executar uma função simples que registra uma mensagem no console.

```ts
import { timers } from '@dcl/sdk/ecs'

console.log('This gets printed right away')

timers.setTimeout(() => {
	// função a ser executada após o atraso
    console.log('This gets printed after 1 second')
}, 1000)
```

A `clearTimeout` pode ser usado para cancelar a execução de uma `setTimeout` função que ainda está aguardando para ser executada. `setTimeout` retorna um id do timer (um número), que você pode então passar para `clearTimeout`. Nesse caso, a variável `timeoutId` é obtida ao fazer o `setTimeout`, e então passada para `clearTimeout` para cancelá-lo.

```ts
import { timers } from '@dcl/sdk/ecs'

const timeoutId = timers.setTimeout(() => {
    console.log('Wait 1 second until executing this function')
}, 1000)

timers.clearTimeout(timeoutId)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
