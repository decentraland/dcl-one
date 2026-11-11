> Obtenha dados do contexto em que sua scene está sendo executada e da própria scene.

# Dados de Runtime

## Obter a hora de Decentraland

Decentraland segue um ciclo de dia/noite que leva 2 horas para ser concluído, então há 12 ciclos completos todos os dias. Os players também podem alterar as configurações para experimentar um horário fixo específico do dia, por exemplo, para sempre ver Decentraland com um céu noturno às 22h. Por esse motivo, a hora de Decentraland pode variar de um player para outro.

Usa `getWorldTime()` para obter a hora do dia que o player está vivenciando dentro de Decentraland.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```


> [!NOTE]
> **💡 Dica**: O `getWorldTime()` a função é assíncrona. Veja [Funções assíncronas](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/) se você não estiver familiarizado com elas.


`getWorldTime()` retorna um objeto com uma `seconds` propriedade. Essa propriedade indica quantos segundos se passaram (no tempo de Decentraland) desde o início do dia, assumindo que o ciclo completo dura 24 horas. Divida o valor de seconds por 60 para obter os minutos e por 60 novamente para obter as horas desde o início do dia. Por exemplo, se o `seconds` valor for *36000*, isso corresponde a *10h da manhã*.

No tempo de Decentraland, o sol sempre nasce às 6:15 e se põe às 19:50.

Você poderia usar essa informação para alterar a cena de acordo, por exemplo, para tocar sons de pássaros quando houver luz do dia e grilos quando estiver escuro, ou para ligar os materiais emissores nos postes de rua quando estiver escuro.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
  if (time.seconds < 6.25 * 60 * 60 || time.seconds > 19.85 * 60 * 60) {
    // horário noturno
    console.log('reproduzindo sons de grilos')
  } else {
    // horário diurno
    console.log('reproduzindo sons de pássaros')
  }
})
```

## Obter dados do realm

Os players em Decentraland existem em vários *realms*separados. Players em realms diferentes não conseguem se ver, interagir ou conversar entre si, mesmo que estejam em parcelas iguais. Dividir os players dessa forma permite que Decentraland lide com um número ilimitado de players sem encontrar nenhuma limitação. Isso também agrupa players que estão em regiões próximas, para garantir que os tempos de ping entre players que interagem sejam aceitáveis.

Se a sua cena enviar dados para um [servidor de terceiros](/docs/creator/content-creator-pt/scenes-sdk7/rede/third-party-servers/) para sincronizar alterações entre players em tempo real, então muitas vezes é importante que as alterações sejam sincronizadas apenas entre players que estejam no mesmo realm. Você deve tratar todas as alterações que pertencem a um realm como separadas das que pertencem a outro realm. Caso contrário, os players verão as coisas mudarem de um jeito estranho, sem que ninguém faça a alteração.

```ts
import { getRealm } from '~system/Runtime'

executeTask(async () => {
  const { realmInfo } = await getRealm({})
  if (!realmInfo) return
  console.log(`Você está no realm: `, realmInfo.realmName)
})
```


> [!NOTE]
> **💡 Dica**: O `getRealm()` a função é assíncrona. Veja [Funções assíncronas](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/) se você não estiver familiarizado com elas.


Decentraland lida com as comunicações entre players (incluindo posições dos players, chat, mensagens do messageBus e alterações de estado de smart items) por meio de uma rede descentralizada de servidores de comunicação; cada um desses servidores é chamado de **Realm**. Cada um desses servidores pode suportar múltiplas **salas** (também chamadas de **ilhas**), cada uma agrupando um conjunto diferente de players que estão próximos uns dos outros no mapa de Decentraland.

A `getRealm()` A função retorna as seguintes informações:

* `baseUrl`: *(string)* O domínio do servidor do realm
* `realmName`: *(string)* O nome do servidor do realm
* `networkId`: *(number)* A rede Ethereum
* `commsAdapter`: *(string)* Adaptador de comunicações, removendo todos os parâmetros de consulta (credenciais)
* `isPreview`: *(boolean)* Verdadeiro se a cena estiver sendo executada como uma pré-visualização local, em vez de publicada em Decentraland.
* `isConnectedSceneRoom`: *(boolean)* Verdadeiro se o usuário estiver conectado à sala da cena.


> [!WARNING]
> **📔 Nota**: O `layer` propriedade está obsoleta e deve ser evitada.


À medida que os players se movem pelo mapa, eles podem trocar de salas para serem agrupados com os players que agora estão mais próximos deles. As salas também deslocam suas bordas dinamicamente para se ajustarem a um grupo administrável de pessoas, então, mesmo que um player fique parado, à medida que os players entram e saem do mundo, esse player pode acabar em outra sala. Players em uma mesma `sala` se comunicam e compartilharão mensagens pelo MessageBus mesmo que estejam longe demais para se ver. Players no mesmo servidor, mas em salas diferentes, atualmente não se comunicam, mas podem passar a se comunicar à medida que se movem pelo mapa e trocam de sala.

Para reagir a mudanças relacionadas ao realm ou à sala do player, use a `onChange` função no `RealmInfo` componente, que o engine adiciona ao `engine.RootEntity`. Esse componente mantém os mesmos campos retornados por `getRealm()`.

```ts
import { engine, RealmInfo } from '@dcl/sdk/ecs'

export function main() {
	RealmInfo.onChange(engine.RootEntity, (realmInfo) => {
		if (!realmInfo) return
		console.log('Realm alterado: ', realmInfo.realmName)
	})
}
```


> [!WARNING]
> **📔 Nota**: Quando a cena carrega pela primeira vez, pode ser que ainda não haja uma sala atribuída ao player. O Explorer acabará atribuindo uma sala ao player, mas isso às vezes pode acontecer alguns segundos depois de a cena ser carregada.


## Obter plataforma do player

Os players podem acessar Decentraland por várias plataformas, incluindo o aplicativo oficial para desktop, e as versões web e desktop descontinuadas, assim como [clientes experimentais alternativos](https://github.com/decentraland/protocol-squad) construídos para outros engines.

Usa `getExplorerInformation()` para saber em qual plataforma o player atual está executando Decentraland.

```ts
import { getExplorerInformation } from '~system/Runtime';

executeTask(async () => {
  let data = await getExplorerInformation({})
  console.log("PLATAFORMA: ", data.platform, " AGENTE: ", data.agent)
})
```

Ao usar o aplicativo oficial de desktop do Decentraland, essa função deve retornar os seguintes dados:

```
{
    agent: unity-explorer,
    platform: desktop
}
```

Players que usam o aplicativo oficial de desktop do Decentraland provavelmente terão uma experiência muito mais fluida do que aqueles no navegador, já que o navegador impõe limitações de desempenho sobre quanto do poder de processamento da máquina a aba do navegador pode usar. Eles também deixarão de ter muitos recursos, como controle de câmera, luzes dinâmicas, congelamento do movimento do player, melhorias de UI etc.

## Obter idioma do player

Usa `getPlayerLanguage()` para saber para qual idioma o cliente do player está configurado. Ele retorna uma etiqueta [BCP 47](https://en.wikipedia.org/wiki/IETF_language_tag) como `'en'`, `'es'` ou `'pt-BR'`. Use-o para escolher qual tradução dos textos da sua cena mostrar.

```ts
import { getPlayerLanguage, onPlayerLanguageChanged } from '@dcl/sdk/platform'

console.log('IDIOMA: ', getPlayerLanguage())

// Reaja à troca de idioma do player no meio da sessão
onPlayerLanguageChanged.add(({ language }) => {
  console.log('IDIOMA ALTERADO PARA: ', language)
})
```

`onPlayerLanguageChanged` é acionado sempre que o player altera o idioma do cliente enquanto está na sua cena. Clientes mais antigos nunca o emitem.


> [!WARNING]
> **📔 Nota**: `getPlayerLanguage()` retorna `'en'` até que o Explorer tenha informado o idioma de volta para a cena, o que acontece logo após a cena começar. Ele também retorna `'en'` em clientes que não informam um idioma. Ambas as funções exigem `@dcl/sdk` a versão 7.30.0 ou mais recente.


## O componente EngineInfo

A `EngineInfo`componente acompanha dados sobre o ciclo de vida da cena, o que às vezes pode ser útil para saber quando um evento está ocorrendo em relação à inicialização da cena.

Este componente é adicionado ao `engine.RootEntity`.

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo) return

  console.log(
    '--------------' +
      '\nframeNumber: ' +
      engineInfo.frameNumber +
      '\ntickNumber: ' +
      engineInfo.tickNumber +
      '\ntotalRuntime: ' +
      engineInfo.totalRuntime +
      '\nsceneHidden: ' +
      engineInfo.sceneHidden +
      '\n--------------'
  )
})
```

Você pode usar `sceneHidden` para pausar trabalhos que exigem muitos recursos enquanto a cena não estiver visível:

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (engineInfo?.sceneHidden) return

  // A lógica normal de gameplay é executada somente quando a cena está visível
})
```

A `EngineInfo` o componente contém os seguintes dados:

* `frameNumber`: Contador de quadros do engine.
* `totalRuntime`: Tempo total de execução desta cena, em segundos.
* `tickNumber`: Contador de ticks da cena conforme [ADR-148](https://adr.decentraland.org/adr/ADR-148).
* `sceneHidden`: `true` quando a cena fica escondida atrás da UI em tela cheia do Explorer. Isso inclui a tela de carregamento, e também o mapa, a mochila e o menu de configurações. Use isso para silenciar o áudio e pausar trabalhos pesados enquanto o player não consegue ver a cena.


> [!WARNING]
> **📔 Nota**: O `EngineInfo` o componente deve ser importado via
> 
> > `import { EngineInfo } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


### Reaja ao desaparecimento da tela de carregamento

A `campo scene_hidden` diz se o player realmente pode ver a sua cena ou se ela está coberta pela UI em tela cheia do Explorer. Enquanto a tela de carregamento está ativa, `sceneHidden` é `true`. No momento em que a tela de carregamento desaparece e o player vê o mundo pela primeira vez, ele se torna `false`.

Ele se torna `true` novamente mais tarde sempre que uma UI em tela cheia do Explorer cobrir a cena, como o mapa, a mochila ou o menu de configurações. Isso o torna um bom sinal para silenciar [o áudio](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/) e pausar sistemas caros, e então retomar quando ele voltar para `false`.

Esta é a única maneira de uma cena saber quando acontece essa primeira revelação. Use isso para segurar qualquer coisa que, de outra forma, aconteceria atrás da tela de carregamento e seria perdida pelo player: cinematics de introdução, sons de boas-vindas, um tween que só faz sentido se for assistido, uma UI de abertura ou um evento de analytics que só deve contar quando o player realmente estiver lá.

```ts
import { engine, EngineInfo } from '@dcl/sdk/ecs'

function onSceneRevealed() {
  // O player agora está olhando para a cena, inicie a introdução aqui
  console.log('A tela de carregamento acabou de desaparecer')
}

engine.addSystem(function waitForSceneRevealed() {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo || engineInfo.sceneHidden) return

  // Executar apenas uma vez
  engine.removeSystem(waitForSceneRevealed)
  onSceneRevealed()
})
```


> [!WARNING]
> **📔 Nota**: Sua cena continua sendo executada normalmente enquanto `sceneHidden` é `true`, ela só não está sendo exibida. Não use este campo para pausar a lógica da sua cena; use-o para sincronizar o que o player deve presenciar.
> 
> `campo scene_hidden` requer um `@dcl/sdk` e uma versão recente do Decentraland Explorer. Em clientes mais antigos, o campo permanece com seu valor padrão de `false`, então uma cena que espere por ele ainda continua sendo executada — ela simplesmente não ficará sincronizada com o desaparecimento da tela de carregamento.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/runtime-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
