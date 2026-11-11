> Saiba como controlar a câmera do jogador

# Câmera

Como criador, pode ter controlo total sobre a câmara do jogador. Por predefinição, os jogadores têm liberdade para escolher entre um modo de câmara em 1.ª ou 3.ª pessoa ao explorar a sua scene, mas pode impor uma modalidade de câmara diferente.

As câmaras virtuais podem ser estáticas, podem rodar para olhar sempre para o jogador ou para outra Entity, ou podem ser anexadas ao jogador ou a outra Entity para estarem sempre a acompanhar.


> [!WARNING]
> **📔 Nota**: Para alternar entre as câmaras predefinidas de 1.ª e 3.ª pessoa, consulte [Áreas modificadoras de câmara](#1st-and-3rd-person-camera-modes).


## modos de câmara em 1.ª e 3.ª pessoa

Normalmente, os jogadores são livres de alternar entre a câmara em primeira e terceira pessoa premindo V no teclado. Use uma `CameraModeArea` para forçar o modo de câmara para 1.ª ou 3.ª pessoa em todos os jogadores que estejam dentro de uma área específica na sua scene.

```ts
const entity = engine.addEntity()

CameraModeArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	mode: CameraType.CT_FIRST_PERSON,
})
```

Se o modo de câmara atual de um jogador não corresponder ao da `CameraModeArea`, eles transitarão para esse modo de câmara. Aparece um toast no ecrã para esclarecer que esta alteração se deve à scene. Enquanto estiverem dentro, os jogadores não podem alterar o seu modo de câmara. Quando um jogador sair da `CameraModeArea`, o seu modo de câmara é restaurado para o que tinha antes de entrar.

Use `CameraModeArea` em regiões onde os jogadores teriam uma experiência significativamente melhor ao usar um modo de câmara específico. Por exemplo, a primeira pessoa é ideal se o jogador precisar de clicar num objeto pequeno, ou a terceira pessoa pode ser útil para os jogadores notarem alguma Entity que a sua scene tenha anexada acima da cabeça. Não assuma que os jogadores sabem como alternar os modos de câmara; muitos jogadores de primeira vez podem não saber que têm a opção, ou podem não se lembrar da tecla para o fazer.


> [!WARNING]
> **📔 Nota**: As áreas modificadoras de câmara são afetadas pelo *posição* e *rotação* do componente Transform da sua Entity anfitriã, mas não são afetadas pelo *escala*.



> [!WARNING]
> **📔 Nota**: Se sobrepuser várias áreas modificadoras de câmara, a última a ser instanciada pelo código da sua scene terá prioridade sobre as outras.


Ao criar um `CameraModeArea` component, deve fornecer o seguinte:

* `area`: Tamanho da área modificadora
* `mode`: Qual modo de câmara forçar nesta área, a partir do `CameraType` enum.

Os modos de câmara suportados são:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

### Consultar o modo de câmara

Pode consultar o modo de câmara do jogador usando o `CameraMode` componente na `engine.CameraEntity`.

```ts
const cameraMode = CameraMode.get(engine.CameraEntity)
if (cameraMode.mode === CameraType.CT_FIRST_PERSON) {
	console.log('O jogador está a usar a câmara em 1.ª pessoa')
} else {
	console.log('O jogador está a usar a câmara em 3.ª pessoa')
}
```

Também pode subscrever alterações no modo de câmara usando o `onChange` função na `CameraMode` componente.

```ts
CameraMode.onChange(engine.CameraEntity, (cameraMode) => {
	if (!cameraMode) return
	console.log('O modo de câmara do jogador mudou para', cameraMode.mode)
})
```

## Usar câmaras virtuais

Para usar um comportamento de câmara personalizado na sua scene, precisa de duas coisas:

* Criar uma Câmara Virtual: Crie uma Entity na sua scene e dê-lhe um `VirtualCamera`.
* Atribuir essa câmara virtual: Adicione uma `MainCamera` componente à [entidade reservada](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities) `engine.CameraEntity`, com uma referência para a Entity com o `VirtualCamera` componente.

A câmara ficará então anexada à Entity com o `VirtualCamera` componente. Se a Entity se mover ou rodar, a câmara move-se com ela.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

Neste exemplo, a câmara estará sempre numa posição fixa na scene, desde que o jogador permaneça dentro dos limites da scene. Assim que o jogador sair dos limites da scene, o comportamento de câmara predefinido será restaurado.

### Campo de visão

Defina o campo de visão (FOV) para uma câmara virtual com a `fov` propriedade, em graus. Aplica-se apenas enquanto esta câmara virtual estiver ativa.

```ts
const cinematicCamera = engine.addEntity()
Transform.create(cinematicCamera, {
	position: Vector3.create(8, 3, 2),
})
VirtualCamera.create(cinematicCamera, {
	fov: 45,
})
```

Um FOV mais estreito (valor mais baixo) dá zoom, útil para cutscenes ou para apontar. Um FOV mais amplo (valor mais alto) mostra mais da scene, útil para vistas panorâmicas.


> [!WARNING]
> **📔 Nota**: Defina `fov` em todas as câmaras virtuais que criar. Deixá-lo de fora não lhe dá uns neutros 60 graus: o client reutiliza instâncias de câmara do pool sem repor a lente, por isso uma câmara sem `fov` fica com qualquer campo de visão que uma câmara anterior tenha deixado nessa instância. Uma câmara destinada a mostrar uma ampla vista aérea pode abrir já com zoom máximo.


A sua scene pode incluir tantas entities com um `VirtualCamera`component como quiser, e alternar dinamicamente entre várias câmaras virtuais à medida que o jogador se move, ou à medida que executa certas ações. Apenas uma câmara virtual está ativa em qualquer momento, isto é atribuído pelo `MainCamera` componente na `engine.CameraEntity`.

Para voltar ao comportamento de câmara predefinido, defina o valor como `undefined` no `MainCamera.virtualCameraEntity`. O jogador fica então livre para alternar entre as câmaras em 1.ª e 3.ª pessoa. Se quiser que o jogador use apenas um destes dois modos, pode usar uma [Áreas modificadoras de câmara](#1st-and-3rd-person-camera-modes) para forçar um dos dois.


> [!WARNING]
> **📔 Nota**: Solte a câmara com `undefined`, nunca com `0`. Só um valor ausente devolve o controlo ao jogador. Um valor de `0` é `engine.RootEntity`, que não tem `VirtualCamera`, por isso o engine mantém a última câmara virtual ativa e o jogador fica preso nessa vista.
> 
> Isto deteta código que guarda e restaura a câmara, porque `virtualCameraEntity` é opcional e lê-se como `undefined` numa nova `MainCamera`. Um `?? 0` ou `|| 0` fallback transforma isso num `0` que é escrito de volta mais tarde. Verifique a Entity antes de a restaurar:
> 
> ```ts
> function restoreCamera(previous: Entity | undefined) {
> 	const camera = MainCamera.getMutableOrNull(engine.CameraEntity)
> 	if (!camera) return
> 	// Only restore a camera that really exists, otherwise release control
> 	camera.virtualCameraEntity = previous && VirtualCamera.has(previous) ? previous : undefined
> }
> ```
> 



> [!WARNING]
> **📔 Nota**: As áreas modificadoras de câmara só têm efeito no jogador se não houver câmaras virtuais ativas. Se a scene estiver atualmente a usar uma câmara virtual e o jogador entrar numa área modificadora de câmara, nada acontece.
> 
> Se um modelo 3D incluir um `câmera` node como parte do seu conteúdo, isso não pode ser usado pelo SDK. Tem de criar todas as câmaras como entities com o SDK.


```ts
function main() {
	// câmara virtual personalizada
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})

	// cubo clicável
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Reset camera' },
		},
		() => {
			// repor a câmara para o comportamento predefinido
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity = undefined
		}
	)
}
```


> [!NOTE]
> **💡 Dica**: Quando a câmara se afasta do avatar, é frequentemente boa prática também congelar os movimentos do avatar. Dessa forma, o jogador não se move às cegas contra obstáculos. Veja [Input Modifiers](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#freeze-the-player)


Para permitir ao jogador controlar uma câmara virtual com o rato, leia o `screenDelta` propriedade do `PrimaryPointerInfo` componente para ver o quanto o cursor se moveu em cada frame, e depois aplique esse movimento à rotação da câmara. Isto funciona mesmo enquanto o cursor está bloqueado. Veja [Movimento do rato](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/mouse-movement/) para um exemplo completo de mouselook.

## Vista aérea

Pode usar uma câmara virtual para ver a scene de uma vista de cima para baixo; isto pode ser uma variação divertida da perspetiva normal do seu avatar e permitir diferentes mecânicas de jogo.

Deve evitar colocar uma câmara a olhar numa direção perfeitamente vertical para baixo. Em vez disso, incline sempre ligeiramente a câmara; até 1 grau é suficiente. Isto porque os controlos de direção do jogador baseiam-se na perspetiva da câmara, não na orientação do avatar. Se a câmara estiver a olhar de uma vista aérea perfeita, não há uma definição clara de qual direção é qual. Uma inclinação impercetível de 1 grau é suficiente para estabelecer uma direção para a frente.

```ts
function main() {
	// câmara de vista aérea
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(8, 5, 8),
		rotation: Quaternion.fromEulerDegrees(91, 0, 0),
		// Note que a rotação é 91º, não 90º
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Campo de Visão

Pode definir um campo de visão (FOV) personalizado numa câmara virtual com a `fov` propriedade. O valor é em graus. Defina-o sempre: não há predefinição para recorrer, e uma câmara sem `fov` herda a lente deixada por uma câmara anterior que usou a mesma instância do pool.

```ts
VirtualCamera.create(myCustomCamera, {
	fov: 90,
})
```

Um FOV mais amplo (valor mais alto) mostra mais da scene de uma vez e cria uma sensação de velocidade, o que pode ser útil para jogos de corrida. Um FOV mais estreito (valor mais baixo) dá zoom e é útil para mirar ou para planos cinematográficos.

### Alterar o nível de zoom

O client lê `fov` apenas no momento em que uma câmara se torna ativa. Alterar `fov` na câmara que já está ativa não faz nada.

Para fazer zoom, crie uma câmara virtual por nível de zoom e alterne `MainCamera.virtualCameraEntity` entre elas:

```ts
import { engine, MainCamera, Transform, VirtualCamera } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Duas câmaras no mesmo local, uma ampla e outra com zoom
const position = Vector3.create(8, 2, 8)

const wideCamera = engine.addEntity()
Transform.create(wideCamera, { position })
VirtualCamera.create(wideCamera, { fov: 70 })

const zoomedCamera = engine.addEntity()
Transform.create(zoomedCamera, { position })
VirtualCamera.create(zoomedCamera, { fov: 30 })

// Alternar entre elas para fazer zoom
function zoomIn() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: zoomedCamera })
}

function zoomOut() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: wideCamera })
}
```

Voltar a atribuir a câmara que já está ativa também não tem efeito, por isso mude para uma Entity diferente de cada vez.

## Transições de câmara

Sempre que a scene alterna entre câmaras virtuais, ou entre o comportamento de câmara predefinido e câmaras virtuais, os jogadores veem uma transição. A posição, a rotação e quaisquer outros parâmetros da câmara virtual mudam suavemente ao longo de um período de tempo.

As definições de transição numa câmara virtual determinam como transita *para* essa câmara, a partir de qualquer outra câmara na scene, incluindo a predefinida. Não afetam como transita *out* a partir dessa câmara.

Ao voltar para a própria câmara do jogador, quando limpa a `virtualCameraEntity` da `MainCamera`, toma sempre uma transição fixa de 0,5 segundos, com aceleração e desaceleração suaves. O `defaultTransition` da câmara virtual que deixa não a altera.

```ts
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})
```


> [!NOTE]
> **💡 Dica**: Para evitar ter uma transição e mudar instantaneamente para uma câmara, defina o tempo ou a velocidade da transição como 0.


Dependendo do seu caso de uso, pode preferir definir a velocidade da transição em vez da duração:

* **Tempo fixo**: Defina a duração da transição; a câmara mover-se-á tão depressa quanto necessário para completar o percurso nesse período de tempo.
* **Velocidade fixa**: Defina a rapidez com que quer que a câmara virtual se mova durante a transição; a duração dependerá da distância. O valor usado para a velocidade é interpretado como **metros por segundo**.

Abaixo estão exemplos para ambos estes modos de transição:

```ts
// duração fixa
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})

// velocidade fixa
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Speed(3) },
})
```

Abaixo segue um exemplo completo com duas câmaras virtuais e transições entre elas:

```ts
function main() {
	// câmara virtual personalizada 1
	const myCustomCamera1 = engine.addEntity()
	Transform.create(myCustomCamera1, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera1, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(1) },
	})

	// câmara virtual personalizada 2
	const myCustomCamera2 = engine.addEntity()
	Transform.create(myCustomCamera2, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera2, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(3) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera1,
	})

	// cubo clicável
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Reset camera' },
		},
		() => {
			// repor a câmara para o comportamento predefinido
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity =
				mainCamera.virtualCameraEntity == myCustomCamera1
					? myCustomCamera2
					: myCustomCamera1
		}
	)
}
```

As transições movem-se sempre em linha reta, sem considerar quaisquer obstáculos no percurso. Em vez disso, pode criar manualmente uma transição usando outra câmara virtual como intermediária; dessa forma, teria controlo total sobre os seus movimentos. Esta câmara virtual intermédia poderia executar um [Tween](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/#move-between-two-points) desde a posição da primeira câmara até à posição da segunda câmara, ou seguir um percurso mais personalizado que contorne obstáculos ou faça um desvio cinematográfico.

## Seguimento de câmara

Pode configurar uma câmara virtual para que ela esteja sempre virada na direção do jogador, ou de alguma Entity específica na scene. A posição da câmara permanecerá estática, mas a sua rotação mudará para manter essa Entity sempre centrada.

Isto pode ser conseguido com a `lookAtEntity` propriedade no `VirtualCamera` componente. Para seguir o jogador, use o [entidade reservada](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities) `engine.PlayerEntity`.

```ts
const myCustomCamera1 = engine.addEntity()
Transform.create(myCustomCamera1, {
	position: Vector3.create(1, 2, 1),
})
VirtualCamera.create(myCustomCamera1, {
	lookAtEntity: engine.PlayerEntity,
})
```

Se uma Entity estiver a ser seguida pela câmara, isso só alterará a rotação, não a posição da câmara.

À medida que a câmara roda, o Transform da Entity com o `VirtualCamera` componente não muda. No entanto, pode ler a rotação da câmara a partir do Transform em `engine.CameraEntity`. A rotação e a posição desta Entity serão absolutas; não serão condicionadas pelas da Entity com o `VirtualCamera` componente. A rotação deste transform é afetada como pelo `lookAtEntity` comportamento.


> [!WARNING]
> **📔 Nota**: Se configurar a câmara virtual com um `lookAtEntity` que referencie a mesma Entity que contém a câmara virtual, ou a `engine.CameraEntity` Entity, o comportamento resultante será o mesmo que não atribuir qualquer Entity de todo.


## Anexar ao jogador

Outro uso da câmara virtual é seguir o jogador a partir de uma distância ou ângulo personalizados, anexando uma câmara virtual à Entity do jogador. Note que o jogador não pode alterar livremente a rotação da câmara, por isso neste caso a rotação da câmara ficará fixa à da câmara virtual. Isto pode ser útil, por exemplo, para jogos de corrida, onde se espera que o jogador olhe sempre para a frente.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(0, 1, 5),
		parent: engine.PlayerEntity,
	})
	VirtualCamera.create(myCustomCamera, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(2) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Câmaras e colliders

Quando a câmara de um jogador se move em modo de 3.ª pessoa, a câmara pode ficar bloqueada por colliders ou não, dependendo das collision layers atribuídas às entities. Tenha isto em conta ao desenhar a sua scene; talvez queira impedir que a câmara atravesse paredes ou outras entities.

Consulte [Colliders](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#cameras-and-colliders) para mais detalhes sobre como configurar colliders para a sua scene.

## Modo de observação (câmara de observador)

Pode construir um modo de observação que muda o jogador do movimento normal do avatar para uma câmara de roaming livre ou de seguimento do jogador. Isto é útil para funções de observador em jogos competitivos, câmaras de realização para eventos em direto ou sistemas de repetição.

O padrão combina várias funcionalidades do SDK:

| Funcionalidade                 | SDK API                                   | Objetivo                             |
| ------------------------------ | ----------------------------------------- | ------------------------------------ |
| Vista de câmara personalizada  | `VirtualCamera` + `MainCamera`            | Substitui a câmara do jogador        |
| Congelar o movimento do avatar | `InputModifier` (`disableAll: true`)      | Liberta WASD para controlar a câmara |
| Acompanhar jogadores na scene  | `onEnterScene` / `onLeaveScene`           | Constrói uma lista de alvos a seguir |
| Controlos da câmara            | `inputSystem.isPressed(InputAction.IA_*)` | WASD para pitch/yaw, E/F para zoom   |
| Mouselook                      | `PrimaryPointerInfo.screenDelta`          | Rodar a câmara com o rato            |

### Arquitetura do rig de câmara

Use um **rig de duas entities** para que yaw e pitch permaneçam independentes:

```
rigRoot (entity)           -- posição no mundo + rotação yaw
└── rigCamera (child)      -- rotação pitch + offset orbital
    └── VirtualCamera
```

`rigRoot` mantém o yaw (viragem esquerda/direita) e é interpolado na direção do alvo a seguir ou de um pivot de free-cam. `rigCamera` trata do pitch (inclinação para cima/baixo) e da distância orbital a partir da raiz. Dividir yaw e pitch por dois Transforms mantém a matemática de Euler simples.

### Ativar e desativar

```ts
// Ativar o modo de observação
const rigRoot = engine.addEntity()
Transform.create(rigRoot, {
  position: Vector3.create(8, 8, 8),
  rotation: Quaternion.fromEulerDegrees(0, 0, 0),
})

const rigCamera = engine.addEntity()
Transform.create(rigCamera, { parent: rigRoot })
VirtualCamera.create(rigCamera, {})

MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: rigCamera })
InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: true }),
})
```

```ts
// Desativar o modo de observação
// IMPORTANTE: limpe o MainCamera ANTES de remover a Entity VirtualCamera.
// Se remover a Entity primeiro, o engine continua a associar-se a uma Entity morta
// e a vista cai para os pés do jogador.
const mainCamera = MainCamera.getMutableOrNull(engine.CameraEntity)
if (mainCamera) mainCamera.virtualCameraEntity = undefined

engine.removeEntity(rigCamera)
engine.removeEntity(rigRoot)

InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: false }),
})
```


> [!CAUTION]
> **Aviso:** Limpe sempre `MainCamera.virtualCameraEntity` antes de remover a Entity da câmara. Remover a Entity primeiro deixa o engine a apontar para uma referência morta, fazendo com que a vista se quebre.


### Limites da câmara

O engine desativa `VirtualCamera` entities que saem para fora dos limites dos parcels da sua scene. Se a câmara sair da área da scene, deixa de funcionar silenciosamente. Faça o clamp da posição da câmara para que permaneça dentro da caixa delimitadora alinhada aos eixos (AABB) da scene em cada frame, com uma pequena margem.

```ts
// Limites de exemplo para uma scene de 1x1 parcel (16m x 16m)
const BOUNDS_MIN = Vector3.create(0, 0, 0)
const BOUNDS_MAX = Vector3.create(16, 20, 16)
const BOUNDS_MARGIN = 0.5
```

Para scenes maiores, defina os limites para corresponderem aos seus `scene.json` parcels. O limite de altura da scene é 330 metros independentemente do número de parcels. Uma scene de 4x4 parcels poderia usar `Vector3.create(64, 330, 64)`.

### Acompanhar jogadores

Use `onEnterScene` e `onLeaveScene` para construir uma lista dinâmica de jogadores na sua scene. Os jogadores podem então alternar entre alvos a seguir com teclas (por exemplo, `IA_ACTION_3` e `IA_ACTION_4`). Ao seguir um jogador, a raiz do rig faz lerp em direção à posição do Transform do jogador seguido, e a câmara filha orbita a uma distância configurável.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

const playerEntities = new Map<string, Entity>()
let playerIds: string[] = []

onEnterScene((player) => {
  if (!player) return
  playerIds.push(player.userId)
  playerEntities.set(player.userId, player.entity)
})

onLeaveScene((userId) => {
  if (!userId) return
  playerIds = playerIds.filter((id) => id !== userId)
  playerEntities.delete(userId)
})
```

Para seguir um jogador em cada frame, faça lerp da raiz do rig em direção à sua posição:

```ts
const followEntity = playerEntities.get(followTargetId)
if (followEntity) {
  const targetPos = Transform.get(followEntity).position
  const rootTransform = Transform.getMutable(rigRoot)
  rootTransform.position = Vector3.lerp(
    rootTransform.position,
    Vector3.add(targetPos, Vector3.create(0, 1, 0)),
    0.1
  )
}
```

Consulte [Jogador entra ou sai da cena](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-enters-or-leaves-scene) para mais sobre `onEnterScene` e `onLeaveScene`.

### Mouselook ao assistir

Ler `PrimaryPointerInfo.screenDelta` a cada frame para girar a câmera com o mouse. Isso funciona enquanto o pointer está bloqueado. Veja [Movimento do rato](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/mouse-movement/) para um exemplo completo de mouselook.

```ts
const MOUSE_SENSITIVITY = 0.15 // graus por pixel
let yaw = 0
let pitch = 45

function spectateMouseLook() {
  const isLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  if (!isLocked) return

  const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
  if (!delta) return

  yaw = (yaw + delta.x * MOUSE_SENSITIVITY) % 360
  // delta.y cresce para baixo, então mouse para cima é negativo: adicioná-lo inclina a câmera para cima; limite para evitar inversão
  pitch = Math.max(-25, Math.min(80, pitch + delta.y * MOUSE_SENSITIVITY))
}
```


> [!WARNING]
> **Nota:** `screenDelta` é apenas para desktop. Em dispositivos móveis ele sempre retorna `0`. Se sua scene tiver como alvo dispositivos móveis, crie uma alternativa baseada em toque.


### Implementação completa de referência

O [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) test scene contém um módulo de spectate autônomo em `src/spectate.ts`. Ele suporta modos free-cam e follow-cam, controles WASD + mouselook, alternância entre players, zoom de distância de órbita, limitação de limites e um HUD na tela mostrando os controles e o target de follow atual.

Para usá-lo no seu próprio projeto:

1. Copie `src/spectate.ts` para sua scene.
2. Atualize `PIVOT`, `BOUNDS_MIN`e `BOUNDS_MAX` no topo do arquivo para corresponder aos parcels da sua scene. Esse é o erro de integração mais comum.
3. Conecte `toggleSpectate()` a qualquer trigger: uma entity clicável, um botão de UI ou uma tecla.


> [!NOTE]
> **💡 Dica**: Para exemplos funcionais de controle de câmera, veja o [`2,22-virtual-cameras`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) test scene, que alterna várias `VirtualCamera` entities por `MainCamera` incluindo `lookAtEntity` apontada para o player e uma câmera com tween; [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), que controla uma câmera mouselook enquanto o pointer está bloqueado; [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode), que implementa uma câmera completa de spectate/observer com modos follow-cam e free-cam; e [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas), que força um modo de câmera dentro de um volume com `CameraModeArea`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
