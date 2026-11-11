> Funções legadas

# Funções Obsoletas

As seguintes funções são todas legadas e devem ser evitadas. Elas ainda funcionam, mas no futuro podem deixar de ser suportadas. Todos estes exemplos incluem links para formas alternativas de obter as mesmas informações ou alcançar os mesmos resultados.

## Jogador entra ou sai da cena


> [!WARNING]
> **📔 Observação**: O `onEnterSceneObservable` e `onLeaveSceneObservable` eventos estão obsoletos no SDK 7.x. Use `onEnterScene` em vez disso, veja [Jogador entra ou sai da cena](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-enters-or-leaves-scene).


Sempre que um avatar entra ou sai das parcelas de terreno que compõem sua cena, ou teleporta para dentro ou para fora, isso cria um evento que você pode escutar. Esse evento é disparado por todos os avatares, incluindo o do jogador.

```ts
import {
	onEnterSceneObservable,
	onLeaveSceneObservable,
} from '@dcl/sdk/observables'

onEnterSceneObservable.add((player) => {
	console.log('jogador entrou na cena: ', player.userId)
})

onLeaveSceneObservable.add((player) => {
	console.log('jogador saiu da cena: ', player.userId)
})
```


> [!WARNING]
> **📔 Observação**: Este evento responde apenas aos jogadores que estão sendo renderizados localmente no momento. Em cenas grandes, em que o tamanho da cena excede o alcance visual, jogadores que entram no canto oposto podem não ser registrados. Se o número de jogadores na região exceder as capacidades de uma ilha nos servers da Decentraland, jogadores que não compartilham a mesma ilha não ficam visíveis e também não são rastreados por estes eventos.


Os observables `onEnterScene` e `onLeaveScene` importados de `'@dcl/sdk/observables'` também estão obsoletos. As funções corretas têm o mesmo nome, mas são importadas de `'@dcl/sdk/src/players'` em vez disso. No exemplo abaixo você pode ver as duas variações, primeiro a versão obsoleta, depois a correta.

```ts
// OBSOLETO - importado de observables
import { onEnterScene, onLeaveScene } from '@dcl/sdk/observables'

onEnterScene.add((player) => {
	console.log('jogador entrou na cena: ', player.userId)
})

onLeaveScene.add((player) => {
	console.log('jogador saiu da cena: ', player.userId)
})

// ATUAL - importado como uma função do jogador
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTROU NA CENA', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('SAIU DA CENA', userId)
	})
}
```

## Jogador conecta ou desconecta


> [!WARNING]
> **📔 Observação**: O `getConnectedPlayers` função e o `onPlayerConnectedObservable` e `onPlayerDisconnectedObservable` eventos estão obsoletos no SDK 7.x. Use `onEnterScene` em vez disso, veja [Jogador entra ou sai da cena](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-enters-or-leaves-scene). Agora cada cena é uma ilha de comunicação distinta, tornando equivalentes estar conectado e estar na mesma cena.


Obtenha a lista completa dos jogadores atualmente conectados com `getConnectedPlayers`.

```ts
import { getConnectedPlayers } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getConnectedPlayers({})
	connectedPlayers.players.forEach((player) => {
		console.log('o jogador já estava aqui: ', player.userId)
	})
})
```

Sempre que outro jogador começa ou para de ser renderizado pelo engine local, isso cria um evento que você pode escutar. Os jogadores podem ou não estar na mesma cena que você, mas precisam estar dentro do alcance visual (não necessariamente no campo de visão). O `onPlayerConnectedObservable` detecta tanto quando um jogador se conecta pela primeira vez nas proximidades quanto quando chega perto o suficiente para estar dentro do alcance visual; de forma semelhante, o `onPlayerDisconnectedObservable` detecta quando um jogador encerra a sessão ou se afasta muito.

```ts
import {
	onPlayerConnectedObservable,
	onPlayerDisconnectedObservable,
} from '@dcl/sdk/observables'

onPlayerConnectedObservable.add((player) => {
	console.log('jogador entrou: ', player.userId)
})

onPlayerDisconnectedObservable.add((player) => {
	console.log('jogador saiu: ', player.userId)
})
```

Lembre-se de que, se outros jogadores já estiverem sendo renderizados nos arredores antes de o jogador ter carregado sua cena, este evento não notificará a cena recém-carregada sobre os jogadores já existentes. Se você precisar acompanhar todos os jogadores atuais, pode consultar os jogadores existentes ao carregar a cena e, depois, escutar este evento para atualizações.

## Consultar todos os jogadores na cena


> [!WARNING]
> **📔 Observação**: O `getPlayersInScene` função está obsoleta no SDK 7.x. Em vez disso, percorra todos os jogadores com um `PlayerIdentityData` componente. Veja [Buscar todos os jogadores](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#fetch-all-players).


Você também pode obter a lista completa de jogadores que estão atualmente na sua cena e sendo renderizados chamando `getPlayersInScene()`.

```ts
import { getPlayersInScene } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getPlayersInScene({})
	connectedPlayers.players.forEach((player) => {
		console.log('o jogador já estava aqui: ', player.userId)
	})
})
```

## Jogador executa animação


> [!WARNING]
> **📔 Observação**: O `onPlayerExpressionObservable` evento está obsoleto no SDK 7.x. Use o `AvatarEmoteCommand` componente em vez disso, veja [Jogador executa animação](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-plays-animation).


Sempre que o jogador executa um emote (dance, clap, wave etc.), você pode detectar este evento.

```ts
import { onPlayerExpressionObservable } from '@dcl/sdk/observables'

onPlayerExpressionObservable.add(({ expressionId }) => {
	console.log('Expressão: ', expressionId)
})
```

O evento inclui as seguintes informações:

* expressionId: Nome do emote executado (ou seja: *wave*, *clap*, *kiss*)


> [!WARNING]
> **📔 Observação**: Este evento é disparado sempre que o jogador faz um emote e a cena está carregada. O jogador pode estar em uma scene próxima quando isso acontecer.


## Jogador altera o perfil


> [!WARNING]
> **📔 Observação**: O `onProfileChanged` evento está obsoleto no SDK 7.x. Use o `AvatarEquippedData` componente em vez disso, veja [Jogador altera o perfil](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-changes-profile).


Sempre que o jogador faz uma alteração no perfil, o `onProfileChanged` evento é chamado. Essas alterações podem incluir vestir diferentes wearables, mudar o nome, a descrição, ativar experiências portáteis etc.

```ts
import { onProfileChanged } from '@dcl/sdk/observables'

console.log(playerRealm.currentRealm?.domain)
	console.log('Os dados do meu perfil são ', profileData)
})
```

Os dados do evento incluem apenas o ID do jogador e um número de versão para o perfil desse avatar, de acordo com o servidor catalyst. Cada vez que uma alteração é propagada, o número de versão aumenta em 1.

Quando este evento é disparado, você pode então usar a [getUserData()](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data) função para buscar a versão mais recente dessas informações, incluindo a lista de wearables que o jogador está usando. Pode ser necessário adicionar um pequeno atraso antes de chamar `getUserData()` para garantir que a versão que essa função retorna esteja atualizada.


> [!NOTE]
> **💡 Dica**: Ao testar em preview com o Explorer web legado, para evitar usar um avatar aleatório, execute a cena no navegador conectado à sua wallet Metamask.



> [!WARNING]
> **📔 Observação**: Este evento é disparado apenas por alterações no jogador atual, não por alterações nos perfis de outros jogadores próximos.


## Cena terminou de carregar


> [!WARNING]
> **📔 Observação**: O `onSceneReadyObservable` evento está obsoleto no SDK v7.x. Esta função não é mais relevante. Você pode garantir que algo seja executado após a cena terminar de carregar executando isso dentro da função `Main()` Veja [Ciclo de vida da cena](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#scene-lifecycle)


Quando a cena termina de carregar, o `onSceneReadyObservable` é chamado. Isso funciona tanto se o jogador entrar diretamente na cena quanto se o jogador caminhar até a cena a partir de outro lugar. Quando todo o conteúdo da cena concluir o carregamento inicial, incluindo modelos pesados etc., este evento é chamado.

```ts
import { onSceneReadyObservable } from '@dcl/sdk/observables'

onSceneReadyObservable.add(() => {
	console.log('CENA CARREGADA')
})
```

## Métodos obsoletos de dados do jogador


> [!WARNING]
> **📔 Observação**: O `getUserData()` e `getPlayerData()` funções estão obsoletas desde o SDK v7.4.x. Use `getPlayer()` em vez disso. Veja [Dados do usuário](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data).


Para obter informações do jogador atual que está executando a cena, use `getUserData()`.

O exemplo abaixo importa o `~system/UserIdentity` namespace e executa `getUserData()`.

```ts
import { getUserData } from '~system/UserIdentity'

executeTask(async () => {
	let userData = await getUserData({})
	console.log(userData.data)
})
```

Você pode obter dados de outros jogadores que estão próximos chamando `getPlayerData()`, passando o id de uma conta Decentraland.

```ts
import { getPlayerData } from '~system/Players'

executeTask(async () => {
	let userData = await getPlayerData({ userId: '0x….' })
	console.log(userData)
})
```

Ambos `getUserData()` e `getPlayerData()` retornam a mesma estrutura de dados disponível pela API de conteúdo. Veja [Dados de qualquer jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#data-from-any-player)

`getPlayerData()` só podem buscar dados de jogadores que estejam atualmente por perto. Eles não precisam necessariamente estar na mesma cena, mas dentro do alcance visual; isso acontece porque essas informações estão sendo buscadas no engine local que está renderizando esses avatares. Para testar isso em preview, abra uma segunda aba e faça login com uma conta diferente.


> [!WARNING]
> **📔 Observação**: Os IDs de usuário devem estar sempre em minúsculas. Se copiar um endereço de wallet, certifique-se de que todos os caracteres estejam em minúsculas.


Os `getUserPublicKey()` e `getUserAccount()` funções também estão obsoletas. Use `getPlayer()` em vez disso. Veja [Dados do usuário](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data).

## Obter tempo do skybox


> [!WARNING]
> **📔 Observação**: O `getDecentralandTime()` função está obsoleta no SDK v7.x.x. Use `getWorldTime()` em vez disso. Veja [Obter hora de Decentraland](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/runtime-data/#get-decentraland-time).


```ts
import { getDecentralandTime } from '~system/EnvironmentApi'

executeTask(async () => {
	let time = await getDecentralandTime({})
	console.log(time)
})
```

## Obter realm


> [!WARNING]
> **📔 Observação**: O `getCurrentRealm()` função está obsoleta no SDK v7.x.x. Use `getRealm()` em vez disso. Veja [Obter dados do realm](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/runtime-data/#get-realm-data).


```ts
import { getCurrentRealm } from '~system/EnvironmentApi'

async function fetchPlayerData() {
	const playerRealm = await getCurrentRealm({})

	console.log(playerRealm.currentRealm?.domain)
}

fetchPlayerData()
```

## Está no modo preview


> [!WARNING]
> **📔 Observação**: O `isPreviewMode()` função está obsoleta no SDK v7.x.x. Use `getRealm()` em vez disso, que contém uma `preview` propriedade. Veja [Obter dados do realm](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/runtime-data/#get-realm-data).


```ts
import { isPreviewMode } from '~system/EnvironmentApi'

executeTask(async () => {
	const { isPreview } = await isPreviewMode({})

	if (isPreview) {
		console.log('Executando em preview')
	}
})
```

## Jogador clica em outro jogador

Sempre que o jogador clica em outro jogador, você pode detectar um evento.

```ts
import { onPlayerClickedObservable } from '@dcl/sdk/observables'

onPlayerClickedObservable.add((clickEvent) => {
	console.log('Clicado em ', clickEvent.userId, ' detalhes: ', clickEvent.ray)
})
```


> [!WARNING]
> **📔 Observação**: O `onPlayerClickedObservable` evento está obsoleto no SDK v7.x. Versões futuras permitirão uma abordagem mais [orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/), baseada em consultar dados regularmente em vez de eventos.
> 
> Como alternativa, você pode anexar um Collider invisível ao jogador e detectar cliques contra ele.



> [!WARNING]
> **📔 Observação**: Tanto o jogador que executa o clique quanto o jogador que é clicado devem estar dentro das parcelas da cena. Este listener detecta apenas eventos do jogador atual clicando em outros jogadores, não aqueles de cliques realizados por outros jogadores.


O evento inclui os seguintes dados:

* `userId`: O id do jogador clicado
* `ray`: Dados sobre o raio traçado pelo clique
  * `direction`: *Vector3* Um Vector3 normalizado que representa a direção do ponto de origem do clique até o ponto atingido pelo clique.
  * `distance`: *number* A distância em metros do ponto de origem até o ponto atingido.
  * `origin`: *Vector3* O ponto de origem do clique, a posição do jogador que realizou o clique, relativa à cena.


> [!NOTE]
> **💡 Dica**: O comportamento padrão de clicar em outro jogador é abrir o passaporte do jogador, onde você pode ver informações adicionais sobre esse jogador, adicioná-lo como amigo etc. Você pode desativar a abertura desta UI para que ela não atrapalhe a experiência que você quer construir adicionando uma [Avatar Modifier Area](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/modifier-areas/).


## Jogador trava/destrava o cursor


> [!WARNING]
> **📔 Observação**: O `onPointerLockedStateChange` evento foi removido no SDK v7.x, ele não está mais disponível. Veja [Event listeners](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-locks-or-unlocks-cursor) para a alternativa atual.


Os jogadores podem alternar entre dois modos de cursor: *cursor travado* modo para controlar a câmera ou *cursor destravado* modo para mover o cursor livremente pela UI.

Os jogadores destravam o cursor clicando no *botão direito do mouse* ou pressionando a *Esc* tecla, e travam o cursor novamente clicando em qualquer lugar da tela.

Para reagir toda vez que um jogador alterna entre esses dois modos, use a `onChange` função no `PointerLock` componente, veja [Event listeners](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-locks-or-unlocks-cursor).

## Jogador muda de realm ou ilha

Jogadores em Decentraland existem em *realms*separados e em *islands* separadas dentro de cada realm. Jogadores em realms ou islands diferentes não podem se ver, interagir ou conversar entre si, mesmo que estejam sobre as mesmas parcelas.

Cada vez que o jogador muda de realm ou ilha, o `onRealmChangedObservable` evento é chamado.

```ts
import { onRealmChangedObservable } from '@dcl/sdk/observables'

onRealmChangedObservable.add((realmChange) => {
	console.log('O JOGADOR MUDOU DE ILHA PARA ', realmChange.room)
})
```


> [!WARNING]
> **📔 Observação**: O `onRealmChangedObservable` evento está obsoleto no SDK v7.x. Versões futuras permitirão uma abordagem mais [orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/), baseada em consultar dados regularmente em vez de eventos.


Este evento inclui os seguintes campos:

* **serverName**: *string*; O nome do servidor catalyst.
* **room**: *string*; O nome da ilha.
* **displayName**: *string*; O nome do servidor catalyst seguido de um *-* e o nome da ilha. Por exemplo `unicorn-x011`.
* **domain**: *string*; A URL do servidor catalyst em uso.

À medida que os jogadores se movem pelo mapa, eles podem trocar de ilha para serem agrupados com os jogadores que agora estão mais próximos deles. As ilhas também deslocam dinamicamente suas fronteiras para acomodar um grupo gerenciável de pessoas em cada uma. Então, mesmo que um jogador fique parado, ele pode ter sua ilha alterada conforme outros entram e saem das scenes ao redor.

Se a sua cena depende de um [servidor de terceiros](/docs/creator/content-creator-pt/scenes-sdk7/rede/third-party-servers/) para sincronizar alterações entre jogadores em tempo real, então talvez você queira compartilhar dados apenas entre jogadores agrupados no mesmo realm+island, então é uma boa prática alterar rooms no servidor de terceiros sempre que os jogadores mudarem de ilha.

## Funções de cripto


> [!WARNING]
> **📔 Observação**: As funções `requirePayment()`, `signMessage()`, `convertMessageToObject()` estão obsoletas. Use a função `sendAsync()` em vez disso. Veja [Operações de blockchain da cena](/docs/creator/content-creator-pt/scenes-sdk7/blockchain/scene-blockchain-operations/). Também existem bibliotecas que podem ajudar a simplificar alguns casos de uso comuns com essas funções.


## Eventos de vídeo


> [!WARNING]
> **📔 Observação**: O `onVideoEvent` evento está obsoleto no SDK v7.x. Veja [Event listeners](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#video-events) para uma alternativa não obsoleta.


Quando um vídeo muda seu status de reprodução, o `onVideoEvent` observable recebe um evento.

```ts
onVideoEvent.add((data) => {
	console.log('Novo evento de vídeo ', data)
})
```

A entrada de um evento de vídeo contém as seguintes propriedades:

* `videoClipId` ( *string*): O ID da entidade que mudou de status.
* `componentId` (*string*): O ID da entidade que mudou de status.
* `currentOffset` (*number*): O valor atual da propriedade `seek` do vídeo. Este valor mostra os segundos após o início original do vídeo. *-1* por padrão.
* `totalVideoLength` (*number* ): A duração em segundos de todo o vídeo. *-1* se a duração for desconhecida.
* `videoStatus`: O valor para o novo status de vídeo da `VideoTexture`, expresso como um valor do enum `VideoStatus` . Este enum pode conter os seguintes valores possíveis:
* `VideoStatus.NONE` = 0,
* `VideoStatus.ERROR` = 1,
* `VideoStatus.LOADING` = 2,
* `VideoStatus.READY` = 3,
* `VideoStatus.PLAYING` = 4,
* `VideoStatus.BUFFERING` = 5


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/deprecated-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
