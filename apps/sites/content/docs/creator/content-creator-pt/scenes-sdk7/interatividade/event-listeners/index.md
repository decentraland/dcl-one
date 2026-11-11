> Eventos que a scene pode acompanhar, relacionados às ações do jogador e às mudanças da scene.

# Event Listeners

Existem vários eventos aos quais a cena pode se inscrever para saber as ações do jogador enquanto ele está na cena ou perto dela.

Para os eventos de botão e clique realizados pelo jogador, consulte [Eventos de botão](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/).

## Jogador entra ou sai da cena

Sempre que um avatar entra ou sai dos parcels de terra que compõem a sua cena, ou teleporta para dentro ou para fora, isso cria um evento que você pode escutar.

Este evento é acionado por todos os avatares, incluindo o do jogador.

```ts
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

Na `onEnterScene` evento, a função pode acessar todos os dados retornados por [obter dados do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data) por meio da `player` propriedade. No `onLeaveScene` evento, a função só tem acesso ao ID do jogador.

### Somente o jogador atual

Você pode filtrar os eventos acionados para reagir apenas ao avatar do jogador, em vez de outros avatares que possam estar por perto.

```ts
import { getPlayer, onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	let myPlayer = getPlayer()

	onEnterScene((player) => {
		if (!player) return
		console.log('ENTROU NA CENA', player)

		if (myPlayer && player.userId == myPlayer.userId) {
			console.log('EU ENTREI')
		}
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('SAIU DA CENA', userId)

		if (myPlayer && userId == myPlayer.userId) {
			console.log('EU SAÍ')
		}
	})
}
```

Este exemplo primeiro obtém o id do jogador, depois se inscreve nos eventos e compara o `userId` retornado pelo evento com o do jogador.

### Consultar todos os jogadores na cena

Percorra a lista completa de jogadores que estão atualmente na sua cena iterando por todas as entities com um `PlayerIdentityData` componente.

```ts
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'

export function main() {
	for (const [entity, data, transform] of engine.getEntitiesWith(
		PlayerIdentityData,
		Transform
	)) {
		console.log('PLAYER: ', { entity, data, transform })
	}
}
```

## Jogador muda o modo da câmera

Saber o modo da câmera pode ser muito útil para ajustar finamente as mecânicas da sua cena para melhor se adequar ao que é mais confortável ao usar esse modo. Por exemplo, alvos pequenos são mais difíceis de clicar na terceira pessoa.

O trecho a seguir usa a `onChange` função para disparar um evento toda vez que a câmera muda. Ela também dispara um evento quando a cena carrega, com o modo de câmera inicial do jogador.

```ts
import { engine, CameraMode } from '@dcl/sdk/ecs'

export function main() {
	CameraMode.onChange(engine.CameraEntity, (cameraComponent) => {
		if (!cameraComponent) return
		console.log('Modo da câmera alterado', cameraComponent?.mode)
		// 0 = primeira pessoa
		// 1 = terceira pessoa
	})
}
```

Veja [Verifique o modo de câmera do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#check-the-players-camera-mode).

## Jogador reproduz animação

Use a `onChange` função no `AvatarEmoteCommand` componente para disparar um evento toda vez que o jogador reproduz uma emote. Isso inclui tanto emotes básicos (dança, palmas, aceno, etc.) quanto emotes de tokens.

```ts
import { AvatarEmoteCommand } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return
		console.log('Emote reproduzida: ', emote.emoteUrn)
	})
}
```

O evento inclui as seguintes informações:

* `emoteUrn`: Nome da emote executada (isto é: *wave*, *clap*, *kiss*)
* `loop`: Se a emote está em loop ou sendo reproduzida uma vez
* `timestamp`: Quando a emote foi acionada.
* `state`: Um valor do `EmoteState` enum que descreve o evento do ciclo de vida: `ES_STARTED` (a emote começou, e também o valor quando o campo está ausente), `ES_FINISHED` (uma emote sem loop foi reproduzida até o fim), ou `ES_INTERRUPTED` (a emote foi interrompida por movimento, teleporte, outra emote, uma parada explícita ou ao sair da cena). Veja [Detectar quando uma emote termina](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#detect-when-an-emote-finishes).

Você também pode detectar emotes de outros jogadores na cena passando uma referência para o outro jogador em vez de `engine.PlayerEntity`.


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional de eventos de reprodução de emote, veja a [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) cena de teste, que registra cada `AvatarEmoteCommand` entrada anexada à entidade do jogador como `STARTED`, `FINISHED` ou `INTERRUPTED` — reproduza uma emote até o fim para ver `FINISHED`, ou afaste-se no meio da reprodução para ver `INTERRUPTED`.


## Jogador muda o perfil

Use a `onChange` função no `AvatarEquippedData` componente para disparar um evento toda vez que o jogador altera um dos seus wearables ou suas emotes listadas na roda de acesso rápido. Da mesma forma, use o `onChange` função no `AvatarBase` para disparar um evento toda vez que o jogador altera as propriedades base do avatar, como cor do cabelo, cor da pele, forma do avatar ou nome.

```ts
import { AvatarEquippedData, AvatarBase } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('Nova lista de wearables: ', equipped.wearableUrns)
		console.log('Nova lista de emotes: ', equipped.emoteUrns)
	})

	AvatarBase.onChange(engine.PlayerEntity, (body) => {
		if (!body) return
		console.log('Nova cor dos olhos: ', body.eyesColor)
		console.log('Nova cor da pele: ', body.skinColor)
		console.log('Nova cor do cabelo: ', body.hairColor)
		console.log('Novo URN do corpo: ', body.bodyShapeUrn)
	})
}
```

O evento em `AvatarEquippedData` inclui as seguintes informações:

* `wearableUrns`: A lista de wearables que o jogador tem atualmente equipados.
* `emoteUrns`: A lista de emotes que o jogador tem atualmente equipadas na roda de acesso rápido.

O evento em `AvatarBase` inclui as seguintes informações:

* `name`: O nome do jogador.
* `bodyShapeUrn`: Os ids correspondentes ao tipo de corpo masculino ou feminino.
* `skinColor`: Cor da pele do jogador como um `Color3`
* `eyesColor`: Cor dos olhos do jogador como um `Color3`
* `hairColor`: Cor do cabelo do jogador como um `Color3`
* `talking`: *true* enquanto o jogador está falando no chat de voz, *false* quando ele está em silêncio, `undefined` quando o cliente não está informando atividade de voz.

Porque `talking` fica em `AvatarBase`, este callback também é acionado toda vez que o jogador começa ou para de falar. Se você só se importa com a aparência, compare os campos de que precisa em vez de reagir a cada mudança. Veja [Detectar quando um jogador está falando](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#detect-when-a-player-is-speaking).

Você também pode detectar mudanças em wearables ou avatars de outros jogadores na cena, basta passar uma referência para o outro jogador em vez de `engine.PlayerEntity`.

Você também pode detectar mudanças nos perfis de outros jogadores na cena, basta passar uma referência para o outro jogador em vez de `engine.PlayerEntity`.

## Jogador bloqueia ou desbloqueia o cursor

Os jogadores podem alternar entre dois modos de cursor: *cursor bloqueado* modo para controlar a câmera ou *cursor desbloqueado* modo para mover o cursor livremente pela UI.

Os jogadores desbloqueiam o cursor clicando no *Botão direito do mouse* ou pressionando a *Esc* tecla, e bloqueiam o cursor novamente clicando em qualquer lugar na tela.

Adicione um `PointerLock` componente à `engine.CameraEntity` entity na sua cena, e use o `onChange` função no `PointerLock` componente para disparar um evento toda vez que o jogador alterna entre os dois modos de cursor.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    // Mostrar dica sobre o modo do cursor
		   }
	})
}
```

Você pode usar essas informações para orientar sutilmente o jogador, como mostrar um popup de UI dizendo que este jogo é melhor aproveitado com o cursor desbloqueado. Ou você também pode forçar o modo de cursor do jogador alterando o `isPointerLocked` no componente. O exemplo a seguir sempre define o modo do cursor como desbloqueado:

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false});
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    PointerLock.getMutable(engine.CameraEntity).isPointerLocked = false
		   }
	})
}
```

## Abrir painéis do Explorer

Sua cena pode abrir os painéis integrados do Explorer (mapa, configurações, mochila etc.) usando a `openExplorerUi` ação restrita. A maioria deles ocupa a tela inteira, mas nem todos. Isso é útil para direcionar os jogadores a recursos específicos do Explorer a partir da UI da sua cena.

O jogador precisa solicitar isso. Chame `openExplorerUi` de dentro de um manipulador de clique, e não por conta própria:

```ts
// Abrir o mapa quando o jogador clica em um cubo
import { openExplorerUi } from '~system/RestrictedActions'
import { engine, ExplorerUi, InputAction, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const mapButton = engine.addEntity()
Transform.create(mapButton, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(mapButton)
MeshCollider.setBox(mapButton)

pointerEventsSystem.onPointerDown(
	{ entity: mapButton, opts: { button: InputAction.IA_POINTER, hoverText: 'Abrir mapa' } },
	() => {
		void openExplorerUi({ ui: ExplorerUi.EU_MAP })
	}
)
```

O `ExplorerUi` enum tem os seguintes valores:

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)
* `EU_ITEM_PURCHASE` (7): o modal de confirmação de compra do item. O Explorer atualmente responde `REJECTED_FEATURE_DISABLED` para ele.


> [!WARNING]
> **Observação:** Assim como outras ações restritas (`movePlayerTo`, `triggerEmote`), `openExplorerUi` só funciona enquanto o jogador estiver dentro dos parcels da cena.


### Somente uma ação do jogador pode abrir um painel

Um painel só abre quando a chamada é uma resposta direta a algo que o jogador acabou de fazer. Uma chamada que não seja isso é recusada.

**Funciona:**

* Clicar em uma entity 3D, com `pointerEventsSystem.onPointerDown` ou `pointerEventsSystem.onPointerUp`.
* Clicar em um elemento da UI da sua cena, com `onMouseDown`.
* Pressionar ou soltar um dos botões de ativação, `E`, `F` ou `1` até `4`, detectado com `inputSystem.isTriggered` e `PointerEventType.PET_DOWN` ou `PointerEventType.PET_UP` (`IA_PRIMARY`, `IA_SECONDARY`, `IA_ACTION_3` até `IA_ACTION_6`).

**Não funciona:**

* Chamando a partir de `main()`, então um painel não pode dar as boas-vindas ao jogador no carregamento.
* Chamando a partir de um timer, ou depois que a ação do jogador já passou.
* Chamando após um `await` em algo que leva tempo, como um `fetch` ou outra ação restrita. A ação do jogador conta por cerca de um frame apenas. Faça o trabalho primeiro e depois abra o painel.
* Passar o mouse. `PET_HOVER_ENTER` e `PET_HOVER_LEAVE` não são ações do jogador.
* Teclas de movimento. Andar, correr e pular não contam; apenas os botões de ativação listados acima.

`openExplorerUi` é resolvido com um `openResult` que informa o que aconteceu:

| Valor                            | Significado                                                                                                             |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `UNSPECIFIED` (0)                | Nenhum resultado foi informado.                                                                                         |
| `OPENED` (1)                     | O painel foi aberto.                                                                                                    |
| `WAS_ALREADY_OPEN` (2)           | Outro painel já estava na tela. Os painéis compartilham um único slot, então apenas um deles pode estar aberto por vez. |
| `REJECTED_NOT_CURRENT_SCENE` (3) | O jogador não está na sua cena.                                                                                         |
| `REJECTED_FEATURE_DISABLED` (4)  | O Explorer tem esse painel desativado, ou o `ui` valor não é um painel que ele conhece.                                 |
| `REJECTED_NO_USER_GESTURE` (5)   | A chamada não foi uma resposta a uma ação do jogador.                                                                   |


> [!WARNING]
> **Observação:** Se nada abrir e você não vir erro, verifique `openResult`. `REJECTED_NO_USER_GESTURE` (5) significa que sua chamada foi executada fora de uma ação do jogador, geralmente porque ela está em `main()`, em um timer, ou após um `await`.


`WAS_ALREADY_OPEN` Isso não se trata apenas de sua cena chamar duas vezes. Todos os painéis ocupam o mesmo slot, então qualquer um deles já estar na tela recusa qualquer outro, e o jogador pode ter aberto aquele manualmente um momento antes, sem envolvimento da sua cena. Trate isso como um resultado normal, e não como um bug no seu código. Com `openExplorerUiAndWait()`descrito abaixo, a mesma situação chega como um resultado `notOpened` .

### Aguarde até o jogador fechar um painel

`openExplorerUiAndWait()` abre um painel e lhe entrega uma promise que é resolvida quando o jogador o fecha. Use isso quando a sua cena só deve continuar depois que o jogador terminar, por exemplo, após levá-lo à mochila para equipar um wearable.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Abrir a mochila e continuar quando o jogador a fechar
async function openBackpack() {
	try {
		const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_BACKPACK })

		if (outcome.$case === 'closed') {
			console.log('O jogador fechou a mochila')
		}
	} catch (error) {
		console.log('A espera falhou:', error)
	}
}
```

Chame `openBackpack()` a partir de um manipulador de clique, da mesma forma que `openExplorerUi`. A espera `await` acontece dentro da função, depois que o painel é solicitado, então a ação do jogador ainda conta.

O objeto que você recebe de volta tem um campo `$case` que informa como a espera terminou:

* `closed`: o jogador fechou o painel que a sua cena abriu. Este é o resultado normal.
* `notOpened`: nada foi aberto. O `openResult` field
* `timedOut`: o limite de tempo que você definiu expirou enquanto o painel ainda estava aberto.
* `matched`: uma `until` condição encerrou a espera antecipadamente, então o painel ainda pode estar na tela.

Quando o painel não consegue abrir, a promise é resolvida imediatamente com `notOpened`. A sua Scene nunca fica à espera de um Panel que não está lá.

Esses quatro resultados cobrem todos os finais normais, mas a promise ainda pode ser rejeitada: a request ao Explorer pode falhar completamente, dois canais podem ser registados com o mesmo nome, ou pode chegar um evento que o helper não consiga atribuir a nenhuma das calls em execução. É por isso que o exemplo acima está envolvido em um `try`/`catch`. Uma rejeição dentro de uma `async` function chamada a partir de um Pointer handler não é reportada em lugar nenhum, por isso, sem o `catch` a espera simplesmente nunca volta e nada lhe diz porquê.


> [!WARNING]
> **📔 Nota**: `openExplorerUiAndWait()` requer `@dcl/sdk` versão 7.30.0 ou superior. Vem do `@dcl/sdk/explorer-ui` módulo, que também reexporta `ExplorerUi`.


#### Defina um limite de tempo

Não há limite de tempo por padrão. Um player pode deixar um Panel aberto durante vários minutos, e a promise fica pendente todo esse tempo. Passe `timeoutMs` quando a sua Scene precisar avançar de qualquer forma.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Pare de esperar após 30 segundos, esteja ou não o mapa ainda aberto
async function openMap() {
	const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_MAP }, { timeoutMs: 30000 })

	if (outcome.$case === 'timedOut') {
		console.log('O player ainda está a ver o mapa')
	}
}
```

#### Pare num evento em vez disso

O `until` opção termina a espera assim que um evento chega num canal, resolvendo com `matched` em vez de `closed`. Os canais são exportados do mesmo módulo: `ExplorerUiEvents` para eventos de abertura e fecho de Panel, e `ItemPurchase` para o resultado de um fluxo de compra. Adicione o mesmo canal à `collect` opção se também quiser os próprios eventos, que chegam no array `events` .

Passe o canal por si só para parar em qualquer um dos seus eventos, ou envolva-o em `variant()` para parar apenas num tipo de evento:

```ts
import { ExplorerUi, ExplorerUiEvents, openExplorerUiAndWait, variant } from '@dcl/sdk/explorer-ui'

// Continue assim que a backpack estiver na tela, sem esperar que o player a feche
async function openBackpack() {
	const outcome = await openExplorerUiAndWait(
		{ ui: ExplorerUi.EU_BACKPACK },
		{ until: variant(ExplorerUiEvents, 'opened'), collect: [ExplorerUiEvents] }
	)

	if (outcome.$case === 'matched') {
		console.log('A backpack está aberta, recolheu', outcome.events.length, 'eventos')
	}
}
```

O `$case` valores `variant()` aceita são os que os eventos do canal transportam: `opened` e `closed` em `ExplorerUiEvents`.

### Observe quando os Panels abrem ou fecham

Use a `ExplorerUiEventsResult` Component para seguir os Panels que a sua própria Scene abriu. Este é um Component apenas de crescimento em `engine.RootEntity`, semelhante a `PointerEventsResult`. Cada Panel que a sua Scene abre com `openExplorerUi` adiciona um evento quando aparece, e outro quando desaparece, incluindo quando o player o dispensa com *Esc*.


> [!WARNING]
> **Observação:** Isto não é uma visão da UI do Explorer em toda a Scene. Um Panel que o player abre manualmente, a partir dos próprios menus do Explorer, não produz eventos aqui. Uma Scene que nunca chama `openExplorerUi` nunca vê nenhum.


`get()` entrega-lhe o conjunto inteiro de eventos acumulados até agora, até aos 100 mais recentes, não os que chegaram desde o último frame. Um System que o percorre às cegas reage a todos os eventos passados em cada tick. Compare a timestamp de cada evento com o último tick que tratou: `timestamp` contra o último tick que tratou:

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

let lastHandledTick = -1

engine.addSystem(() => {
	let newestTick = lastHandledTick

	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		// O conjunto mantém todos os eventos, por isso salte os ticks já tratados
		if (event.timestamp <= lastHandledTick) continue
		if (event.timestamp > newestTick) newestTick = event.timestamp

		if (event.event?.$case === 'opened') {
			console.log('Panel aberto:', event.ui)
		}
		if (event.event?.$case === 'closed') {
			console.log('Panel fechado:', event.ui)
		}

		// Verifique qual Panel específico
		if (event.ui === ExplorerUi.EU_MAP && event.event?.$case === 'closed') {
			console.log('O player fechou o mapa')
		}
	}

	// Só depois do loop, para que vários eventos no mesmo tick sejam todos tratados
	lastHandledTick = newestTick
})
```

`ExplorerUiEventsResult.onChange()` em `engine.RootEntity` é a outra forma de ler o stream: o callback executa uma vez por cada novo evento, por isso não há manutenção de estado para acertar.

Cada evento contém:

* `ui`: Qual Panel é o objeto do evento (um `ExplorerUi` valor).
* `timestamp`: O tick da Scene em que o evento aconteceu.
* `requestId`: O id que a sua Scene passou na `openExplorerUi` chamada que abriu este Panel, ou `0` se a chamada não passou nenhum. Use-o para distinguir várias chamadas em execução.
* `event`: Ou `{ $case: 'opened' }` ou `{ $case: 'closed' }`.


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional de tudo o que foi descrito acima, veja a [`80,-6-explorer-ui-events`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/80,-6-explorer-ui-events) test scene. Imprime todos os veredictos e todos os eventos no seu próprio HUD, e cobre o gesture gate, um Panel já aberto, `requestId` correlação e cada resultado de `openExplorerUiAndWait()`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
