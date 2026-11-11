> Reaja ao movimento do rato do jogador em tempo real, para acionar gestos de arrastar, girar objetos ou controlar câmeras personalizadas.

# Movimento do rato

Sua scene pode ler o movimento bruto do mouse do player em cada frame e usá-lo para conduzir interações em tempo real: gestos de arrastar e deslizar, girando ou deslizando objetos enquanto o player os arrasta, ou controlando uma câmera personalizada como em um first-person shooter.

Para fazer isso, leia o `screenDelta` propriedade do `PrimaryPointerInfo` componente em `engine.RootEntity`. Esta propriedade é um `Vector2` que informa quantos pixels o mouse se moveu desde o último frame. Valores positivos de `x` significam que o mouse se moveu para a direita, valores positivos de `y` significam que ele se moveu para baixo, correspondendo à origem da tela no canto superior esquerdo — o mesmo espaço de coordenadas de `UiTransform` e `UiCanvasInformation`. Em frames em que o mouse não se move, ambos os valores são 0.

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta) return
	console.log(`movimento do mouse: ${delta.x}, ${delta.y}`)
})
```

Como `screenDelta` só contém o movimento de um único frame, sempre o leia dentro de um [system](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/), para que nenhum movimento passe despercebido entre frames.


> [!WARNING]
> **📔 Nota**: Evite referir-se ao `engine.RootEntity` na carga inicial da scene, porque isso pode resultar em erros se as entities ainda não estiverem inicializadas. Para evitar esse problema, sempre referencie a entity dentro de um system. Ela sempre estará disponível, porque a primeira execução do system acontece quando a scene já está devidamente inicializada.



> [!WARNING]
> **📔 Nota**: O que é descrito neste documento é relevante apenas para jogadores no desktop. No [aplicativo móvel](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), o input é baseado em toque e não há cursor livre, então `screenDelta` sempre reporta 0. Veja [Input em dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/) para saber como o input funciona em dispositivos touch, e [Detectar plataforma](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) para oferecer controles alternativos aos jogadores de mobile.


## Movimento do mouse enquanto o cursor está bloqueado

`screenDelta` se comporta de forma diferente das outras propriedades de `PrimaryPointerInfo` quando o cursor está [bloqueado](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#lock-or-unlock-the-cursor): `screenCoordinates` fica congelado no centro da tela e `worldRayDirection` sempre reporta o raio no centro da tela, mas `screenDelta` continua reportando o movimento bruto do mouse em cada frame. Isso o torna a única forma de ler o movimento do mouse enquanto o cursor está bloqueado, o que é exatamente o que você precisa para controles personalizados de câmera ou mira.

## Arraste para girar um objeto

O exemplo a seguir permite que o player gire um cube arrastando o mouse lateralmente enquanto mantém o botão do ponteiro pressionado:

```ts
import {
	engine,
	InputAction,
	inputSystem,
	MeshCollider,
	MeshRenderer,
	PrimaryPointerInfo,
	Transform,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const DRAG_SENSITIVITY = 0.5

export function main() {
	const cube = engine.addEntity()
	Transform.create(cube, { position: Vector3.create(8, 1, 8) })
	MeshRenderer.setBox(cube)
	MeshCollider.setBox(cube)

	engine.addSystem(() => {
		// gire apenas enquanto o botão do ponteiro estiver pressionado
		if (!inputSystem.isPressed(InputAction.IA_POINTER)) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta || delta.x === 0) return

		const transform = Transform.getMutable(cube)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromEulerDegrees(0, delta.x * DRAG_SENSITIVITY, 0)
		)
	})
}
```

O mesmo padrão funciona para qualquer interação de arraste: deslize uma entity ao longo de um trilho usando `delta.x` como deslocamento, percorra uma animação ou detecte um swipe rápido verificando valores altos de delta.

## Controles de câmera em mouselook

Como `screenDelta` continua funcionando enquanto o cursor está bloqueado, você pode usá-lo para criar controles de mouselook para uma [virtual camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#using-virtual-cameras). Para isso, acumule o movimento do mouse em ângulos de yaw e pitch e, em seguida, aplique-os à rotação da câmera em cada frame.

No exemplo a seguir, o player clica em uma caixa para alternar para uma virtual camera que pode controlar com o mouse e pressiona o botão secundário (*F* ou clique com o botão direito) para retornar à câmera padrão. Enquanto a virtual camera está ativa, a scene também bloqueia o cursor e congela o avatar com um [input modifier](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#freeze-the-player), para que o player não caminhe às cegas enquanto controla a câmera.

```ts
import {
	engine,
	Entity,
	InputAction,
	InputModifier,
	inputSystem,
	MainCamera,
	MeshCollider,
	MeshRenderer,
	PointerEventType,
	pointerEventsSystem,
	PointerLock,
	PrimaryPointerInfo,
	Transform,
	VirtualCamera,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// graus de rotação da câmera por pixel de movimento do mouse
const SENSITIVITY = 0.15

let cameraEntity: Entity
let cameraActive = false
let yaw = 0
let pitch = 0

export function main() {
	// a entity que a câmera segue
	cameraEntity = engine.addEntity()
	Transform.create(cameraEntity, { position: Vector3.create(8, 3, 8) })
	VirtualCamera.create(cameraEntity, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0.5) },
	})

	// clique nesta caixa para entrar no modo mouselook
	const box = engine.addEntity()
	Transform.create(box, { position: Vector3.create(8, 1, 4) })
	MeshRenderer.setBox(box)
	MeshCollider.setBox(box)
	pointerEventsSystem.onPointerDown(
		{
			entity: box,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Controlar câmera' },
		},
		() => activateCamera(true)
	)

	// controle a câmera com o mouse
	engine.addSystem(() => {
		if (!cameraActive) return
		if (!PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta) return

		yaw += delta.x * SENSITIVITY
		// limite o pitch para que a câmera não possa virar de cabeça para baixo
		pitch = Math.max(-85, Math.min(85, pitch + delta.y * SENSITIVITY))

		Transform.getMutable(cameraEntity).rotation = Quaternion.fromEulerDegrees(pitch, yaw, 0)
	})

	// saia do modo mouselook com o botão secundário
	engine.addSystem(() => {
		if (!cameraActive) return
		if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) {
			activateCamera(false)
		}
	})
}

function activateCamera(active: boolean) {
	cameraActive = active

	// atribua ou libere a virtual camera
	MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: active ? cameraEntity : undefined,
	})

	// congele o avatar enquanto a câmera estiver sendo controlada
	InputModifier.createOrReplace(engine.PlayerEntity, {
		mode: InputModifier.Mode.Standard({ disableAll: active }),
	})

	// bloqueie o cursor, para que o mouse controle a câmera imediatamente
	PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: active })
}
```

Algumas coisas a notar sobre este exemplo:

* O system soma `delta.y` ao pitch. Mover o mouse para cima reporta um valor negativo `delta.y`, que reduz o pitch e inclina a câmera para cima. Inverta esse sinal se você preferir controles verticais invertidos.
* O pitch é limitado a uma faixa de -85 a 85 graus, então a câmera nunca pode virar para trás.
* O `SENSITIVITY` constante expressa graus de rotação por pixel de movimento do mouse; ajuste-a como preferir.


> [!NOTE]
> **💡 Dica**: O player pode desbloquear o cursor a qualquer momento ao pressionar *Esc* ou clicando com o botão direito, o que faz a câmera parar de responder ao mouse. Sempre ofereça aos players uma maneira clara de sair completamente do modo de câmera, como o botão secundário no exemplo acima. Você também pode bloquear o cursor novamente pelo código; veja [Bloquear ou desbloquear o cursor](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#lock-or-unlock-the-cursor).


## Tópicos relacionados

Para ler a posição absoluta do cursor na tela em vez do seu movimento, use a `screenCoordinates` propriedade do mesmo componente. Veja [Verificar a posição do cursor do player](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#check-the-players-cursor-position).

Para descobrir qual entity está sob o cursor, combine a `worldRayDirection` propriedade com um raycast. Veja [Raycasting](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting/).


> [!NOTE]
> **💡 Dica**: Para exemplos funcionais do estado do ponteiro, veja a [`0,5-primary-cursor-info`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,5-primary-cursor-info) scene de teste, que lê `PrimaryPointerInfo` a cada frame e alimenta `worldRayDirection` em um raycast; [`31,20-pointer-lock-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control), que solicita e libera a captura do cursor escrevendo `PointerLock.isPointerLocked`; e [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), que mostra `screenDelta` ainda reportando movimento enquanto `screenCoordinates` está congelado no centro da tela durante o bloqueio.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/mouse-movement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
