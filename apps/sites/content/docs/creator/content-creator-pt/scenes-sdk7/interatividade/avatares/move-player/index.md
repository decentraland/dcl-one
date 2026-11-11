> Teleportar o jogador, ou fazê-lo andar até uma nova posição dentro da sua scene

# Mover o Player

Para alterar a posição do jogador na cena, use a `movePlayerTo()` função. Use-a para reaparecer jogadores, configurar pontos de verificação ou fazer um jogador caminhar até um local como parte de uma cena de transição.


> [!NOTE]
> **💡 Dica**: A forma mais fácil de mover o jogador é usar o [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/). Use a opção sem código **Mover jogador** ou dos **Mover jogador para aqui** Actions, veja [Torne qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).


Esta função recebe um objeto com as seguintes propriedades:

* `newRelativePosition`: Onde posicionar o jogador, expresso como um Vector3.
* `cameraTarget`: (opcional) A direção para a qual fazer a câmera olhar, expressa como um Vector3 representando as coordenadas de um ponto no espaço para onde olhar. Se nenhum valor for fornecido, a câmera manterá a mesma rotação de antes de se mover.
* `avatarTarget`: (opcional) A direção para a qual fazer o avatar olhar, expressa como um Vector3 representando as coordenadas de um ponto no espaço para onde olhar. Se nenhum valor for fornecido, o avatar manterá a mesma rotação de antes de se mover. Se o jogador estiver no modo de câmera em primeira pessoa, a rotação da câmera e do avatar são as mesmas.
* `duration`: (opcional) Quanto tempo a transição deve levar, em segundos. Se nenhum valor for fornecido, a transição ocorrerá instantaneamente. Se uma duração for fornecida, o avatar caminhará ou correrá até esta nova posição.


> [!WARNING]
> **📔 Nota**: Durante a transição, o avatar não é afetado por colliders, então pode atravessar objetos.


```ts
import { movePlayerTo } from '~system/RestrictedActions'

// criar entidade
const myEntity = engine.addEntity()
MeshRenderer.setBox(myEntity)
MeshCollider.setBox(myEntity)

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

// dar comportamento à entidade
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	function () {
		// reaparecer jogador
		movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
		})
	}
)
```

O movimento do jogador ocorre instantaneamente, sem telas de confirmação ou transições de câmera.


> [!WARNING]
> **📔 Nota**: Os jogadores só podem ser movidos se já estiverem dentro dos limites da cena, e só podem ser movidos para locais que estejam dentro dos limites da cena. Você não pode usar `movePlayerTo()` para transportar um jogador para outra cena. Para mover um jogador para outra cena, veja [Teleports](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/external-links/#teleports).


O `movePlayerTo()` a função pode ser aguardada, então, se o movimento tiver uma duração, você pode usar `await` para esperar que o jogador chegue ao destino.

```ts
import { movePlayerTo } from '~system/RestrictedActions'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	async function () {
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})
	}
)
```


> [!WARNING]
> **📔 Nota**: Sem um `duration`, a promise se resolve assim que o teleporte é enfileirado, não quando o jogador tiver chegado. O código após o `await` é executado enquanto `Transform.get(engine.PlayerEntity)` ainda informa a posição antiga, e o movimento que aterrissa um momento depois pode interromper um emote que você iniciou lá. Se algo tiver de acontecer na chegada, consulte a posição do jogador em vez disso:
> 
> ```ts
> import { engine, Transform } from '@dcl/sdk/ecs'
> import { Vector3 } from '@dcl/sdk/math'
> import { movePlayerTo } from '~system/RestrictedActions'
> 
> function teleportThen(target: Vector3, onArrive: () => void) {
> 	void movePlayerTo({ newRelativePosition: target })
> 
> 	let waited = 0
> 	function waitForArrival(dt: number) {
> 		waited += dt
> 		const pos = Transform.getOrNull(engine.PlayerEntity)?.position
> 		// comparar apenas em X e Z, para que uma diferença de altura não impeça a espera
> 		const arrived = pos !== undefined && Math.hypot(pos.x - target.x, pos.z - target.z) < 0.2
> 		if (!arrived && waited < 1.5) return // desista após 1,5s
> 		engine.removeSystem(waitForArrival)
> 		onArrive()
> 	}
> 
> 	engine.addSystem(waitForArrival)
> }
> ```
> 


Se o jogador tentar se mover durante a duração da transição, a transição será interrompida e a `movePlayerTo` função nunca será resolvida. Você pode impedir esse comportamento desativando todas as entradas enquanto a transição estiver em andamento usando o `InputModifier` : componente, veja [Restringir locomoção](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#restrict-locomotion).

```ts
import { movePlayerTo } from '~system/RestrictedActions'
import {InputModifier, engine} from '@dcl/sdk/ecs'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Click' },
	},
	async function () {

		// desativar todas as entradas
		InputModifier.create(engine.PlayerEntity, {
			mode: InputModifier.Mode.Standard({
				disableAll: true,
			}),
		})

		// mover jogador
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})

		// ativar todas as entradas
		InputModifier.deleteFrom(engine.PlayerEntity)
	}
)
```


> [!WARNING]
> **📔 Nota**: Além disso, o resultado do await pode ser armazenado para avaliar seu `sucesso` propriedade para saber se o movimento foi interrompido (por exemplo, por movimento de entrada) ou concluído com sucesso.



> [!NOTE]
> **💡 Dica**: Para mover um jogador para uma cena diferente, ou para coordenadas diferentes no mapa de Genesis City, veja [Teleports](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/external-links/#teleports).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/move-player.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
