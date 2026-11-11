> Saiba como lidar com cliques de usuário na sua scene.

# Eventos de Clique

Uma scene do Decentraland pode detetar ações de input de todos os botões usados para controlar o Avatar do jogador. Estes incluem cliques de pointer, vários botões de ação e as teclas usadas para mover o Avatar. Os eventos de botão podem vir de um rato e teclado no client de desktop, dos controlos no ecrã no [app mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), de um controlador de VR ou de outros dispositivos de input — todos são interpretados da mesma forma pelo SDK.

Você pode detetar ações de input contra uma Entity. Isso envolve premir um botão enquanto o cursor do jogador está a apontar para o collider dessa Entity. Você também pode detetar *global* evento de input global, que envolve ativar o input a qualquer momento, sem considerar para onde o pointer está a apontar.


> [!WARNING]
> **📔 Nota**Entities precisam ter um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/) para responder a ações de input. `MeshRenderer` modelos também precisam receber um `MeshCollider` component. Modelos de um `GltfContainer` podem ter a sua própria geometria de colisão incorporada, ou podem ser configurados para usar a sua geometria visível; também podem receber um `MeshCollider` componente.


Existem várias maneiras diferentes de lidar com ações de input, dependendo do caso de uso.

* [**Registar um callback**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/): A maneira mais fácil de adicionar interatividade a uma única Entity. Escreva uma única instrução para configurar uma função de callback e feedback ao passar o cursor.
* [**Baseado em System**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/): Ideal para lidar com várias Entities com comportamento semelhante. Use um System para iterar sobre Entities semelhantes e consultar ações de input em cada uma, lidando com todas com a mesma lógica. O feedback ao passar o cursor precisa ser configurado separadamente. Esta abordagem também é necessária para lidar com ações de input globais.
* [**Avançado**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/advanced-button-events/): Leia os dados de resposta brutos em cada Entity, incluindo carimbos de data/hora e um histórico de eventos de input. Isso pode ser útil para definir padrões de interação personalizados.

## Use o Scene Editor no Creator Hub

A forma mais fácil de lidar com eventos de clique numa Entity é usar o Scene Editor. Use o **On Click** ou **On Input Action** Triggers num item para chamar ações ao clicar nele. Ou use **On Global Click**, **On Global Primary** ou **On Global Secondary** Triggers para reagir a eventos de botão globais. Veja [Torne qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).

## Exemplo simples

Para detetar cliques numa Entity, use `pointerEventsSystem.onPointerDown`.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Clique' },
	},
	function () {
		console.log('Entity clicada')
	}
)
```

Consulte [**Registar um callback**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) para mais informações.

## Feedback ao passar o cursor

É importante fazer com que os jogadores percebam que uma Entity é interativa. Caso contrário, podem perder completamente a experiência que construiu. Não é uma boa experiência clicar em todos os objetos na esperança de que um responda.

Quando usa o [**Registar um callback**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) método, são apresentados dois tipos de feedback sempre que o jogador passa o cursor sobre o objeto:

* A borda da Entity é realçada (apenas no client de desktop do Decentraland 2.0). O realce é verde se a Entity estiver perto o suficiente para clicar, vermelho se a Entity estiver demasiado longe.
* É exibida uma dica de hover perto do cursor com texto de UI, indicando o que acontecerá se clicarem.

Ao usar o [**Baseado em System**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) método, pode obter os mesmos resultados adicionando um `PointerEvents` component às Entities clicáveis.

Tanto o realce da Entity como a dica de hover podem ser desativados através de propriedades nestes métodos e componentes.

O realce da borda contorna a Entity sobre cujo collider o cursor está, além de todos os filhos dessa Entity. Nunca contorna a Entity pai nem os irmãos dela. Apenas shapes de um `MeshRenderer` e modelos de um `GltfContainer` ganham um contorno, `TextShape` texto não. Se usar uma caixa invisível como uma área clicável maior, faça com que as partes visíveis sejam filhos dessa Entity de caixa; caso contrário, nada é contornado.

Você também poderia implementar [dicas personalizadas avançadas](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/#advanced-custom-hints), por exemplo, poderia reproduzir um som, fazer com que a Entity mude de cor, gire ou aumente de tamanho enquanto está a ser apontada, etc. O que quer que faça, certifique-se de que é um sinal claro.

## Obstáculos

Os eventos de botão lançam rays que apenas interagem com a primeira Entity no seu caminho que esteja subscrita à collision layer de pointer events. Isso é verdade enquanto a Entity estiver dentro dos seus limites de distância. Se a Entity definir limites de distância tanto para a câmera como para o Avatar, basta que uma das duas esteja suficientemente perto.

Registe o pointer event na mesma Entity que tem o collider. Um pointer event registado numa Entity pai, ou numa Entity filha que não tenha collider próprio, nunca é disparado.

Para que uma Entity seja interceptada pelo raio de um pointer event, é necessário que:

* O modelo deve conter [meshes de collider](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/).
* O `GltfContainer` deve ser configurado para usar a [geometria visível com máscaras de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#colliders-on-3d-models).
* A Entity deve ter um [component MeshCollider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).

Se o collider de outra Entity estiver a bloquear o caminho da Entity com a qual o jogador quer interagir, o jogador não conseguirá clicar na Entity que está atrás, a menos que a Entity não tenha collider, ou que este collider esteja configurado para não responder à collision layer de pointer events.

```ts
// Entity clicável
const clickableEntity = engine.addEntity()
MeshRenderer.setBox(clickableEntity)
MeshCollider.setBox(clickableEntity)
Transform.create(clickableEntity, { position: Vector3.create(8, 1, 8) })

pointerEventsSystem.onPointerDown(
	{
		entity: clickableEntity,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Clique',
		},
	},
	function () {
		console.log('Entity clicada')
		const t = Transform.getMutable(clickableEntity)
		t.scale.y += 0.2
	}
)

// não bloqueia cliques
const nonBlocker = engine.addEntity()
MeshRenderer.setBox(nonBlocker)
MeshCollider.setBox(nonBlocker, ColliderLayer.CL_PHYSICS)
Transform.create(nonBlocker, { position: Vector3.create(10, 1, 8) })

// bloqueia cliques
const blocker = engine.addEntity()
MeshRenderer.setBox(blocker)
MeshCollider.setBox(blocker, ColliderLayer.CL_POINTER)
Transform.create(blocker, { position: Vector3.create(8, 1, 10) })
```


> [!WARNING]
> **📔 Nota**: Para uma Entity não apenas interceptar um pointer event, mas também devolver dados, a Entity também precisa ter um `PointerEvents` component. O `pointerEventsSystem` helpers também tratam desse requisito.


## Botões de pointer

Os seguintes inputs podem ser tratados por qualquer uma das abordagens para detetar eventos de input.

* `InputAction.IA_POINTER`: **botão esquerdo do rato** num computador, **botão de interação** no mobile.
* `InputAction.IA_PRIMARY`: **E** tecla num computador, **Botão E** no mobile.
* `InputAction.IA_SECONDARY`: **F** tecla num computador, **Botão F** no mobile.
* `InputAction.IA_ACTION_3`: **1** tecla num computador, **botão 1** no mobile. *Oculto por padrão no mobile (SDK 7.32.0+) — use-o apenas se a sua scene o mostrar com `TouchScreenControls`.*
* `InputAction.IA_ACTION_4`: **2** tecla num computador, **botão 2** no mobile. *Oculto por padrão no mobile (SDK 7.32.0+) — use-o apenas se a sua scene o mostrar com `TouchScreenControls`.*
* `InputAction.IA_ACTION_5`: **3** tecla num computador, **botão 3** no mobile. *Oculto por padrão no mobile (SDK 7.32.0+) — use-o apenas se a sua scene o mostrar com `TouchScreenControls`.*
* `InputAction.IA_ACTION_6`: **4** tecla num computador, **botão 4** no mobile. *Oculto por padrão no mobile (SDK 7.32.0+) — use-o apenas se a sua scene o mostrar com `TouchScreenControls`.*
* `InputAction.IA_JUMP`: **Espaço** tecla num computador, **botão de salto** no mobile.
* `InputAction.IA_FORWARD`: **W** tecla num computador.
* `InputAction.IA_LEFT`: **A** tecla num computador.
* `InputAction.IA_RIGHT`: **D** tecla num computador.
* `InputAction.IA_BACKWARD`: **S** tecla num computador.
* `InputAction.IA_WALK`: **Control** tecla num computador.
* `InputAction.IA_MODIFIER`: **Shift** tecla num computador.

Cada `InputAction` é abstraído do input literal no teclado para que possa ser mapeado para diferentes inputs dependendo do dispositivo. Pela mesma razão, nem todos os botões do teclado podem ser monitorizados para eventos de botão, apenas os botões usados para movimento e interação. Esta limitação intencional garante que o conteúdo é compatível entre desktop, o [client mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), e futuros dispositivos como controladores de VR e outros comandos de jogo.


> [!WARNING]
> **📱 Mobile**: No mobile [client mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), cada ação de input pode ser encaminhada para um botão no ecrã, mas por predefinição apenas três botões são mostrados: `IA_POINTER` (botão de interação), `IA_PRIMARY` (botão E) e `IA_JUMP`. `IA_SECONDARY` (F) e `IA_ACTION_3`–`IA_ACTION_6` (a `1`/`2`/`3`/`4` botões) começam ocultos (SDK 7.32.0 e posteriores; os SDKs mais antigos mostram os oito, com os mais baixos atrás de um overflow "+"). Ao desenhar para mobile, prefira `IA_POINTER`, `IA_PRIMARY` e `IA_JUMP` para as suas ações-chave. Se precisar de F ou de um botão numerado, mostre-o com [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/). Veja [Input no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/).


## Tipos de pointer events

Cada input pode produzir os seguintes tipos de pointer events. Cada um dos seguintes é um valor no `PointerEventType` enum.

* `PET_DOWN`: O jogador prime um botão específico enquanto o cursor aponta para o collider da Entity.
* `PET_UP`: O jogador solta um botão específico enquanto o cursor aponta para o collider da Entity.
* `PET_HOVER_ENTER`: O cursor do jogador começa a apontar para o collider da Entity.
* `PET_HOVER_LEAVE`: O cursor do jogador deixa de apontar para o collider da Entity.
* `PET_PROXIMITY_ENTER`: O jogador anda para dentro do alcance de proximidade da Entity, independentemente de para onde esteja a olhar.
* `PET_PROXIMITY_LEAVE`: O jogador sai do alcance de proximidade da Entity.

Consulte [**Eventos de proximidade**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/) para saber como usar interações baseadas em proximidade.

## Dados de uma ação de input

Todas as ações de input incluem dados sobre o evento, incluindo coisas como o botão ativado e para onde o pointer estava a apontar no momento.

As seguintes informações podem ser obtidas de qualquer evento de input:

* `analógico`: Número opcional, presente apenas em eventos de um input analógico (como um joystick), armazenando o valor analógico do input.
* `botão`: Qual foi o id do botão premido. O número corresponde ao `InputAction` enum, que lista todos os botões disponíveis.
* `state`: Tipo de pointer event, do enum `PointerEventType`. *0* refere-se a `PointerEventType.PET_UP`, *1* a `PointerEventType.PET_DOWN`, *2* a `PointerEventType.PET_HOVER_ENTER`, *3* a `PointerEventType.PET_HOVER_LEAVE`, *4* a `PointerEventType.PET_PROXIMITY_ENTER`, *5* a `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: Um [carimbo de data/hora lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) para identificar cada evento de botão.

  > Nota: Este carimbo de data/hora não é numerado com base na hora atual. Pense nele como um contador que começa em 0 e é incrementado de 1 em 1 para cada evento.
* `hit`: Um objeto que contém os seguintes dados sobre o evento hit:
  * `entityId`: Número de id da Entity que foi atingida pelo ray.
  * `meshName`: *String* com o nome interno da mesh específica no modelo 3D que foi atingida. Isso é útil quando um modelo 3D é composto por várias meshes.
  * `globalOrigin`: *Vector3* para a posição onde o ray se origina (relativa à scene)
  * `direction`: *Vector3* com o vetor de direção do ray, em coordenadas globais
  * `posição`: *Vector3* para a posição onde o ray intersectou a Entity atingida (relativa à scene)
  * `length`: Comprimento do ray desde a sua origem até à posição onde ocorreu o hit contra a Entity.
  * `normalHit`: *Vector3* com um vetor de direção normalizado, descrevendo o ângulo da normal do hit no espaço do mundo.

Esses dados são acedidos de diferentes formas dependendo da abordagem que está a usar para lidar com ações de input.

Usando a [**Registar um callback**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) abordagem, o primeiro parâmetro passado para a função de callback contém toda esta estrutura de dados.

```ts
pointerEventsSystem.onPointerDown({ entity: myEntity }, function (cmd) {
	console.log(cmd.hit?.entityId)
})
```

Usando a [**Baseado em System**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) abordagem, use `inputSystem.getInputCommand()` para obter estes dados.

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


> [!WARNING]
> **📔 Nota**: Para uma Entity não apenas interceptar um pointer event, mas também devolver dados, a Entity também precisa ter um `PointerEvents` component. O `pointerEventsSystem` helpers também tratam desse requisito.


Usando a [**Avançado**](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/advanced-button-events/) abordagem, o `PointerEventsResult` contém uma lista com um histórico recente de todos os pointer events contra essa Entity.

```ts
engine.addSystem(() => {
	for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
		const pointerEvents = PointerEventsResult.get(entity)

		for (const event of pointerEvents) {
			console.log(event.hit?.entityId)
		}
	}
})
```

## Bloquear ou desbloquear o cursor


> [!NOTE]
> **💡 Nota**: `PointerLock` é um conceito do client de desktop (cursor do rato bloqueado vs. desbloqueado). Não tem efeito no [app mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), onde o input é baseado em toque.


Os jogadores no client de desktop do Decentraland podem alternar entre dois modos distintos de interação com o cursor:

* Cursor bloqueado: mover o rato desloca a câmera e os eventos de clique ocorrem na mira no centro do ecrã
* Cursor desbloqueado: o cursor é livre para se mover independentemente da câmera, e os jogadores podem clicar em qualquer lugar do ecrã para interagir com o espaço 3D ou com a UI

Estes modos de interação são ideais para diferentes tipos de mecânicas de jogo, e pode querer fazer com que os jogadores adotem um ou outro dependendo do que melhor se adequa à sua scene. Também pode querer garantir momentaneamente que o cursor do jogador está desbloqueado para lhes permitir interagir com uma UI; isso é especialmente útil para novos jogadores que não estejam familiarizados com o modo como isto é controlado.

Para alterar o estado do cursor do jogador, use o `PointerLock` component no `engine.CameraEntity` Entity.

```ts
import {PointerLock} from '@dcl/sdk/ecs'
    
PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false})
```


> [!WARNING]
> **📔 Nota**: O engine fornece um `PointerLock` component no `engine.CameraEntity` por predefinição. Use `createOrReplace()` ou `getMutable()` em vez de `create()`, que lança um erro se o component já existir.


Também pode consultar o estado do cursor do jogador lendo o estado do `PointerLock` component.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

const isPointerLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked
```

Use a `.onChange` função para reagir às alterações no estado do pointer. O seguinte exemplo impõe que o cursor esteja sempre desbloqueado.

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


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
