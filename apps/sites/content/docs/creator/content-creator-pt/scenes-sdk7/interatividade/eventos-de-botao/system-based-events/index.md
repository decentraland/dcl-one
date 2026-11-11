> Saiba como lidar com cliques de usuário na sua scene.

# Eventos Baseados em System

Se a sua scene tiver várias entities semelhantes que são todas ativadas usando a mesma lógica, você pode escrever um único system para iterar sobre todas elas e descrever esse comportamento apenas uma vez. Esta também é a opção de melhor desempenho e mais [orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/) abordagem.

Você também pode usar um system para detectar [eventos globais de input](#global-input-events), para que a scene reaja sempre que uma tecla for pressionada, sem considerar para onde o cursor do player está apontando.

Se tudo o que você quer fazer é clicar ou pressionar um botão em uma única entity para ativá-la, a maneira mais fácil é usar o [Registrar um callback](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) abordagem.

Para definir uma lógica customizada mais específica, talvez você queira lidar com os dados brutos e usar o [Avançado](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/advanced-button-events/) abordagem.

Para uma entity ser interativa, ela deve ter um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/). Veja [obstáculos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#obstacles) para mais detalhes. Isso também se aplica a eventos de clique por proximidade.

## Usando um system

Verifique eventos de botão executando uma das funções auxiliares no input `inputSystem` namespace a cada tick dentro de um [system](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/).

Por exemplo, o system a seguir usa a `inputSystem.isTriggered()` função para verificar se o pointer foi clicado. A cada tick, ele verifica se o botão foi pressionado. Se `inputSystem.isTriggered()` retorna *true*, o system executa uma lógica customizada em resposta.

```ts
engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
		// Lógica em resposta ao pressionamento do botão
	}
})
```

As seguintes funções auxiliares estão disponíveis no `inputSystem` namespace, e podem ser chamadas da mesma forma a cada tick:

* `inputSystem.isTriggered`: Retorna true se uma ação de input ocorreu desde o último tick.
* `inputSystem.isPressed`: Retorna true se um input estiver atualmente pressionado. Ele retornará true a cada tick até que o botão seja solto novamente.
* `inputSystem.getInputCommand`: Retorna um objeto com dados sobre a ação de input.

Veja as seções abaixo para mais detalhes sobre cada um.

Ao lidar com eventos de botão em uma entity, sempre forneça feedback ao player, para que o player saiba que uma entity pode ser interagida. Se você adicionar um `PointerEvents` component a uma entity, os players verão uma dica enquanto passam o cursor sobre essa entity. Veja [Mostrar feedback](#show-feedback) para aprender como adicionar hover hints em entities interativas.

### Eventos globais de input

Use `inputSystem.isTriggered` para detectar eventos de button down e button up em qualquer um dos inputs rastreados pelo SDK.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
	) {
		// Lógica em resposta ao pressionamento do botão
	}
})
```

O exemplo acima verifica se o botão foi pressionado, independentemente de onde o pointer esteja ou de quais entities possam estar em seu caminho.

`inputSystem.isTriggered()` retorna *true* somente se o botão indicado foi pressionado no tick atual do game loop. Se o botão não tiver sido pressionado, ou já estivesse pressionado no tick anterior, ele retorna *false*.


> [!WARNING]
> **📔 Nota**: O player precisa estar dentro dos limites da scene para que o pointer event seja detectado. O cursor do player também precisa estar travado; botões pressionados com o cursor livre não são detectados.
> 
> Toda entity com a qual você queira interagir deve ter tanto um [`PointerEvents` componente](#show-feedback) e uma [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).


A `inputSystem.isTriggered` A function recebe os seguintes argumentos obrigatórios:

* `InputAction`: Qual input ouvir, como um valor do `InputAction` enum. Veja [Botões do pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções suportadas.
* `PointerEventType`: Que tipo de evento ouvir, como um valor do `PointerEventType` enum. Veja [Tipos de eventos de pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#types-of-pointer-events) para opções suportadas.

### Ativar uma entity

Para detectar eventos de botão enquanto aponta para uma entity específica, passe um terceiro argumento opcional para `inputSystem.isTriggered` para especificar contra qual entity verificar.

```ts
// Dê à entity um component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				showFeedback: false,
			},
		},
	],
})

// criar system
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Lógica em resposta ao pressionamento do botão em myEntity
	}
})
```

O exemplo acima verifica a cada tick se uma única entity codificada diretamente foi pressionada com o pointer button (botão esquerdo do mouse).

A `inputSystem.isTriggered` A function recebe os seguintes argumentos:

* `InputAction`: Qual input ouvir, como um valor do `InputAction` enum. Veja [Botões do pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções suportadas.
* `PointerEventType`: Que tipo de evento ouvir, como um valor do `PointerEventType` enum. Veja [Tipos de eventos de pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#types-of-pointer-events) para opções suportadas.
* `Entity` *(opcional)*: Em qual entity verificar esses eventos. Se nenhum valor for fornecido, ele verificará pressionamentos globais do botão, independentemente de para onde o cursor do player estava apontando.

Observe que, neste exemplo, também estamos adicionando um `PointerEvents` component à entity com a qual queremos interagir. Esta etapa é necessária; sem esse component, a entity não poderá ser detectada por nenhuma das functions de `inputSystem`. veja [Mostrar feedback](#show-feedback) para mais detalhes sobre o `PointerEvents` componente.


> [!WARNING]
> **📔 Nota**: e um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/). Veja [obstáculos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#obstacles) para mais detalhes.


Se houver várias entities com as quais o player possa interagir da mesma forma, considere usar `inputSystem.getInputCommand`. Este command retorna informações sobre um comando global de clique, incluindo o entity ID; você pode usar isso para executar uma única function que possa lidar com todas elas.

```ts
engine.addSystem(() => {
	const result = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN
	)
	if (result) {
		if (result.hit?.entityId === myEntity) {
			// tratar clique
		}
	}
})
```


> [!WARNING]
> **📔 Nota**: Toda entity com a qual você queira interagir deve ter tanto um [`PointerEvents` componente](#show-feedback) e uma [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).


Veja [Dados da ação de input](#data-from-input-action) para mais informações. Este método também oferece dados mais detalhados sobre o hit do pointer event.

### Botão de input liberado

Você também pode verificar eventos de pointer up de maneira muito semelhante, usando `PointerEventType.PET_UP`.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_UP,
			myEntity
		)
	) {
		// Lógica em resposta à liberação do botão enquanto aponta para myEntity
	}
})
```


> [!WARNING]
> **📔 Nota**: Ao verificar eventos de pointer up em relação a uma entity específica, não se leva em consideração para onde o cursor estava apontando quando o botão foi pressionado. Só considera para onde o cursor está apontando quando o botão é liberado.
> 
> Lembre-se também de que a entity deve ter tanto um [`PointerEvents` componente](#show-feedback) e uma [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).


### Verificar botões pressionados

Verifique se um botão está sendo pressionado no momento usando `inputSystem.isPressed()` dentro de um system.

```ts
engine.addSystem(() => {
	if (inputSystem.isPressed(InputAction.IA_POINTER)) {
		// Lógica em resposta ao botão mantido pressionado
	}
})
```

`inputSystem.isPressed()` retorna *true* se o botão estiver sendo mantido pressionado no momento, independentemente de quando ele foi pressionado e de para onde o cursor do player está apontando. Caso contrário, ele retorna *false*.

A `inputSystem.isPressed` A function recebe um único argumento:

* `InputAction`: Qual input ouvir, como um valor do `InputAction` enum. Veja [Botões do pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções suportadas.

### Lidar com várias entities

Se sua scene tiver várias entities afetadas por pointer events da mesma forma, faz sentido escrever um system que itere sobre todas elas.

```ts
engine.addSystem(() => {
	const activatedEntites = engine.getEntitiesWith(PointerEvents)
	for (const [entity] of activatedEntites) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			// Lógica em resposta à interação com uma entity
		}
	}
})
```

Este exemplo usa uma [component query](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/) para iterar sobre todas as entities com um `PointerEvents` component. Em seguida, verifica cada uma dessas entities com `inputSystem.isTriggered`, iterando sobre elas uma por uma. Se uma ação de input for detectada em qualquer uma dessas entities, ele executa uma lógica customizada.

Em vez de iterar sobre *todos* as entities com um `PointerEvents` component em um único system, você pode querer escrever systems diferentes para lidar com entities que devem se comportar de maneiras diferentes. A abordagem recomendada é marcar diferentes tipos de entities com [custom components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/), e iterar sobre eles em systems separados.

```ts
engine.addSystem(() => {
	const activatedDoors = engine.getEntitiesWith(IsDoor, PointerEvents)
	for (const [entity] of activatedDoors) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			openDoor(entity)
		}
	}
})

engine.addSystem(() => {
	const pickedUpGems = engine.getEntitiesWith(IsGem, PointerEvents)
	for (const [entity] of pickedUpGems) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			pickUp(entity)
		}
	}
})
```

Este exemplo tem um system que itera sobre todas as entities que têm um custom component chamado `IsDoor` e outro que itera sobre todas as entities que têm um custom component chamado `isGem`. Em ambos os systems, ele verifica cada entity correspondente para ver se foram ativadas com o pointer button.

Essa forma de organizar o código da sua scene é muito [orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/) e deve resultar em um uso muito eficiente dos recursos de memória.

\## Vários botões em uma entity

Usando auxiliares como `pointerEventsSystem.onPointerDown`, cada entity fica limitada a ter um único tipo de evento. Você não pode registrar dois eventos de botão diferentes. Você não tem essa limitação ao usar events baseados em system. Por exemplo, aqui está uma entity com a qual os players podem interagir tanto pressionando E quanto F, cada um acionando funcionalidades diferentes.

```ts
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            // reagir ao E
        }
        if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN, myEntity)) {
            // reagir ao F
        }
    })
```

## Mostrar feedback

Para exibir dicas de UI enquanto aponta para uma entity, use as properties no `PointerEvents` componente.

```ts
// criar entity
const myEntity = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
			},
		},
	],
})
```

Sempre que o cursor do player apontar para os colliders desta entity, a UI exibirá uma hover hint para indicar que a entity pode ser interagida. Veja as seções abaixo para detalhes sobre o que você pode configurar.


> [!WARNING]
> **📔 Nota**: O `PointerEvents` O component cuida apenas da exibição do hover feedback. Para lidar com os próprios eventos de botão com lógica customizada, veja [Usando um system](#using-a-system).


A `PointerEvents` component requer pelo menos uma definição de pointer event. Cada definição de pointer event pode ser configurada com o seguinte:

* `eventType`: Que tipo de evento ouvir, como um valor do `PointerEventType` enum. Veja [Tipos de eventos de pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#types-of-pointer-events) para opções suportadas.
* `eventInfo`: Um objeto que pode conter os seguintes campos:
  * `button` (*ficheiros*): Qual input ouvir, como um valor do `InputAction` enum. Veja [Botões do pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções suportadas.
  * `hoverText` *(opcional)*: Qual string exibir na dica de hover feedback. "Interact" por padrão.
  * `showFeedback` *(opcional)*: Se false, oculta tanto a dica de hover quanto o edge highlight desta entity. *true* por padrão.
  * `showHighlight` *(opcional)*: Se true, os players verão o edge highlight ao passar o cursor sobre a entity. *true* por padrão. Este valor só é considerado se `showFeedback` estiver *true*.
  * `maxDistance` *(opcional)*: A que distância o **avatar** do player pode estar da entity e ainda interagir com ela, em metros. O padrão é *10 metros*. Veja [Distância máxima](#max-distance).
  * `maxCameraDistance` *(opcional)*: A que distância a **câmera ativa** pode estar da entity e ainda interagir com ela, em metros. Não é definido por padrão. Veja [Distância máxima](#max-distance).
  * `priority` *(opcional)*: Qual entity vence quando várias se sobrepõem. Valores mais altos têm precedência. *0* por padrão.

Um único `PointerEvents` component pode conter várias definições de pointer events, que podem detectar eventos diferentes para botões diferentes. Cada entity pode ter apenas *um* `PointerEvents` component, mas esse component pode incluir vários objetos em seu `pointerEvents` array, um para cada evento ao qual responder.

```ts
PointerEvents.create(NPCEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Greet',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Choke',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Stop Choke',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_SECONDARY,
				hoverText: 'Give gem',
			},
		},
	],
})
```

Os players verão vários rótulos, um para cada pointer event, exibidos radialmente ao redor do cursor.

O exemplo abaixo combina o uso de `PointerEvents` para mostrar hover hints, junto com um system que de fato lida com a ação do player com lógica customizada.

```ts
// criar entity
const myEntity = engine.addEntity()

// dar feedback de hover à entity
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})

// lidar com eventos de clique na entity
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Lógica customizada em resposta a uma ação de input
	}
})
```

### Hover Feedback

Quando um player passa o cursor sobre um item com um `PointerEvents` component, ele vê:

* Um edge highlight na entity
* Uma hover hint perto do cursor com um ícone do botão que precisam pressionar e uma string que diz "Interact".

Esses elementos podem ser alternados e personalizados.

O hover feedback na UI exibe um ícone diferente dependendo do input que você selecionar no `button` field. No PC, ele exibe um ícone com um `E` para `InputAction.IA_PRIMARY`, um `F` para `InputAction.IA_SECONDARY`, e um mouse para `InputAction.IA_POINTER`.

Altere a string mudando o `hoverText` valor. Mantenha essa string curta, para que seja rápida de ler e não seja muito intrusiva na tela.

```ts
// criar entity
const chest = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open',
			},
		},
	],
})
```

No exemplo acima, o pointer event inclui um valor para `hoverText`hoverText *Interact*.


> [!NOTE]
> **💡 Dica**: O `hoverText` A string deve descrever a ação que acontece ao interagir. Por exemplo `Abrir`, `Ativar`, `Pegar`, `Selecione`. Essas strings devem ser o mais curtas possível, para evitar roubar demais o foco do player.


Se uma entity tiver vários pointer events, as hover hints de cada um deles serão exibidas radialmente ao redor do cursor.

A `hoverText` de um `.UP` pointer event é exibido apenas enquanto o player já estiver mantendo pressionada a tecla correspondente e apontando para a entity.

Se uma entity tiver tanto um `DOWN` pointer event e um `UP` pointer event, a dica da `DOWN` action é mostrada enquanto o botão não está sendo pressionado. A dica muda para a do `UP` evento somente quando o botão é pressionado e permanece pressionado.

```ts
// criar entity
const entity = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(entity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drag',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Drop',
			},
		},
	],
})
```

Para ocultar a hover hint, mas manter o edge highlight, defina o valor do `hoverText` para "".

```ts
// criar entity
const chest = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '',
			},
		},
	],
})
```

Para ocultar o edge highlight, mas manter a hover hint, defina `showHighlight` para *false*.

```ts
// criar entity
const chest = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Open door',
				showHighlight: false,
			},
		},
	],
})
```

Para ocultar tanto a hover hint quanto o edge highlight, defina `showFeedback` para *false*. Ao fazer isso, o cursor não mostra nenhum ícone, texto ou edge highlight. Você também pode simplesmente remover o `PointerEvents` component da entity.

```ts
// criar entity
const chest = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				showFeedback: false,
			},
		},
	],
})
```

### Distância máxima

Algumas entities podem ser intencionalmente interativas apenas a curta distância. Se um player estiver muito longe de uma entity, a hover hint não será exibida ao lado do cursor.

Por padrão, as entities só podem ser clicadas quando o **avatar** está dentro de *10 metros* da entity. Você pode alterar esse alcance definindo a `maxDistance` property de um pointer event.

```ts
// criar entity
const myEntity = engine.addEntity()

// dê à entity um component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 6,
			},
		},
	],
})

// implementar um system para lidar com o pointer event
engine.addSystem(() => {
	// buscar dados sobre o pointer event
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// verificar se o clique estava perto o suficiente
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// fazer algo
	}
})
```

O exemplo acima define a distância máxima para hover hints como *6 metros*. Certifique-se de que a lógica para lidar com as input actions também siga as mesmas regras. Veja [Dados da ação de input](#data-from-input-action) como obter a distância de uma input action.

`maxDistance` é medida a partir do **avatar**, então se comporta da mesma forma em primeira e terceira pessoa.

Para limitar a interação pela distância em que o **câmera ativa** está, use `maxCameraDistance`. Definir ambos significa que passar em qualquer uma das verificações é suficiente. Veja [Limites de distância](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/#distance-limits) para o conjunto completo de regras.


> [!WARNING]
> **📔 Nota**: `maxPlayerDistance` é um alias descontinuado de `maxDistance`. Ambos medem a distância a partir do avatar, então use `maxDistance` em novas scenes.


## Dicas customizadas avançadas

A `PointerEvents` O component adiciona facilmente dicas de UI quando o cursor do player começa a passar sobre uma entity. Em geral, é bom que as dicas se comportem de forma consistente com o que os players estão acostumados a ver em outras scenes de Decentraland. No entanto, em alguns casos você pode querer sinalizar que algo é interativo de uma forma customizada. Por exemplo, você poderia reproduzir um som sutil quando o player começa a passar o cursor sobre a entity. Você também poderia mostrar um destaque brilhante ao redor da entity enquanto passa o cursor, e ocultá-lo quando não estiver mais passando. Isso também poderia ser usado para mecânicas específicas de gameplay.

Use a `inputSystem.isTriggered()` function junto com os `PointerEventType.PET_HOVER_ENTER` e `PointerEventType.PET_HOVER_LEAVE` events para executar comportamentos customizados sempre que o cursor do player começa a apontar para o collider da entity e sempre que o cursor para de apontar para ele.

O exemplo abaixo aumenta o tamanho das entities para *1.5* quando o cursor começa a apontar para o collider delas, e as devolve a um tamanho de *1* quando o cursor as deixa.

```ts
engine.addSystem(() => {
	const meshEntities = engine.getEntitiesWith(MeshRenderer)
	for (const [entity] of meshEntities) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_ENTER,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1.5, 1.5, 1.5)
		}

		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_LEAVE,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1, 1, 1)
		}
	}
})
```


> [!WARNING]
> **📔 Nota**: Toda entity com a qual você queira interagir deve ter tanto um [`PointerEvents` componente](#show-feedback) e uma [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).


## Eventos de proximidade

Verifique pressionamentos de botão por proximidade independentemente de para onde o cursor esteja apontando usando `PET_DOWN` em uma entity que usa `InteractionType.PROXIMITY`. Defina o `interactionType` campo na definição do pointer event, ao lado de `eventInfo`, para marcar o evento como baseado em proximidade.

Veja [**Eventos de Proximidade**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/) para mais detalhes, incluindo as funções auxiliares disponíveis.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Pressione E',
				maxDistance: 5
			},
			interactionType: InteractionType.PROXIMITY,
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
		console.log('O jogador pressionou o botão perto da entity')
	}
})
```

Com helpers como `pointerEventsSystem.onProximityDown`, uma entity pode manter um cursor handler e um proximity handler ao mesmo tempo, e ambos os callbacks são executados. O que ela não pode fazer é registrar dois botões diferentes para o mesmo handler. Eventos baseados em system não têm essa limitação: você define uma entrada por comportamento. Por exemplo, aqui está uma entity que pode ser interagida tanto apontando um cursor e pressionando E, quanto andando até perto dela e pressionando E sem apontar para ela com o cursor.

```ts
	PointerEvents.create(myEntity, {
        pointerEvents: [
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_POINTER,
                    hoverText: 'Clique',
                    maxDistance: 15,
                },
                interactionType: InteractionType.CURSOR,
            },
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_PRIMARY,
                    hoverText: 'Pressione E',
                    maxDistance: 15,
                },
                interactionType: InteractionType.PROXIMITY,
            },
        ],
    })
    
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN, myEntity)) {
            console.log('O jogador clicou na entity')
        }
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            console.log('O jogador pressionou o botão perto da entity')
        }
    })
```

Você também pode usar `inputSystem.isTriggered()` com o `PET_PROXIMITY_ENTER` e `PET_PROXIMITY_LEAVE` tipos de evento para reagir quando um jogador entra ou sai do alcance de proximidade de uma entity. Isso é útil para exibir feedback extra para entities ativadas por proximidade, por exemplo tocando um som ou uma animação.

Para isso funcionar, a entity precisa de um `PointerEvents` component com o correspondente tipo de evento de proximidade definido.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_PROXIMITY_ENTER,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
			},
		},
		{
			eventType: PointerEventType.PET_PROXIMITY_LEAVE,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
				showFeedback: false,
			},
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_ENTER, myEntity)) {
		console.log('O jogador entrou no proximity')
	}
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_LEAVE, myEntity)) {
		console.log('O jogador saiu do proximity')
	}
})
```

## Dados da ação de input

Obtenha dados de uma input action, como o botão que foi pressionado, a entity que foi atingida, a direção e o comprimento do raio, etc. Veja ([Veja a documentação](https://github.com/decentraland/docs/tree/main/README.md)) para uma descrição de todos os dados disponíveis.

Para obter estes dados, use `inputSystem.getInputCommand`. Esta função retorna a estrutura completa de dados com informações sobre o input event.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```

Se não houver nenhuma input action que corresponda à consulta, então `inputSystem.getInputCommand` retorna *null*. Certifique-se de tratar esse cenário na sua lógica.

### Distância máxima de click

Para impor uma distância máxima, de modo que uma entity só possa ser clicada a curta distância, obtenha `hit.length` propriedade dos dados do evento.

```ts
// implementar um system para lidar com o pointer event
engine.addSystem(() => {
	// buscar dados sobre o pointer event
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// verificar se o clique estava perto o suficiente
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// fazer algo
	}
})
```


> [!WARNING]
> **📔 Nota**: Se você ignorar quaisquer events que estejam longe, certifique-se de definir o `maxDistance` parâmetro no `PointerEvents` component para se comportar de forma consistente.


### Diferentes meshes dentro de um model

Muitas vezes, *.glTF* os modelos 3D são compostos por múltiplos meshes, cada um com um nome interno individual. Todos os button events incluem a informação de qual mesh específica foi clicada, então você pode usar essas informações para acionar diferentes comportamentos de click em cada caso.

Para ver como os meshes dentro do model são nomeados, você deve abrir o modelo 3D com uma ferramenta de edição, como [Blender](https://www.blender.org/) por exemplo.

![](/docs/_remote/5f167d2485c1df5d331a16b6.webp)


> [!NOTE]
> **💡 Dica**: Você também pode descobrir o nome do mesh clicado registrando-o e lendo-o no console.


Você acessa a propriedade `meshName` como parte do `hit` objeto, que é retornado pelo evento de click.

No exemplo abaixo, temos um modelo de casa que inclui um mesh nomeado `firePlace`. Queremos ligar a lareira apenas quando o mesh correspondente for clicado.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd && cmd.hit?.meshName === 'firePlace') {
		// acender a lareira
	}
})
```


> [!WARNING]
> **📔 Nota**: Isso só se aplica a eventos baseados em pointer, e não a eventos baseados em proximity, já que nesse caso todos os meshes do model poderiam estar em proximity do player ao mesmo tempo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
