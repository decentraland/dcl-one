> Como mover, girar e dimensionar uma entity gradualmente ao longo do tempo, com mudanças incrementais.

# Mover Entities

Para mover, rotacionar ou redimensionar uma entity na sua scene ao longo de um período de tempo, use o `Tween` component. O engine executa a transformação desejada suavemente, mostrando atualizações em cada frame até que a duração especificada termine. Além disso, os `Transform` valores do component da entity afetada são atualizados em tempo real caso seja necessário fazer verificações de proximidade no código da scene.


> [!NOTE]
> **💡 Dica**: No [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), você pode mover entities de forma sem código por meio de **Ações**, veja [Transforme qualquer item em smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).


O component Tween tem as seguintes funções:

* `setMove`: Mover entre dois pontos
* `setRotate`: Rotacionar entre duas direções
* `setScale`: Redimensionar entre dois tamanhos
* `setMoveRotateScale`: Transicionar simultaneamente nos três parâmetros
* `setMoveContinuous`: Mover constantemente na mesma direção
* `setRotateContinuous`: Rotacionar constantemente na mesma direção
* `setTextureMove`: Deslocar a textura de um Material entre duas posições
* `setTextureMoveContinuous`: Deslocar a textura de um Material constantemente na mesma direção

## Mover entre dois pontos

Para mover uma entity entre dois pontos, crie um `Tween` component com o `setMove` função.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)
```

O tween de movimento recebe as seguintes informações:

* `entity`: A entity a mover
* `start`: Um Vector3 para a posição inicial
* `end`: Um Vector3 para a posição final
* `duração`: Quantos milissegundos leva para mover entre as duas posições

Este outro parâmetro opcional também está disponível:

* `easingFunction`: Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens)

## Rotacionar entre duas direções

Para rotacionar uma entity entre dois pontos, crie um `Tween` component com o `setRotate` função.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setRotate(myEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

O tween de rotação recebe as seguintes informações:

* `start`: Uma Quaternion para a rotação inicial
* `end`: Uma Quaternion para a rotação final
* `duração`: Quantos milissegundos leva para mover entre as duas posições

Este outro parâmetro opcional também está disponível:

* `easingFunction`: Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens)

### Rotacionar com um ponto de pivô

Ao rotacionar uma entity, a rotação é sempre em referência à coordenada central da entity. Para rotacionar uma entity usando outro conjunto de coordenadas como ponto de pivô, crie uma segunda entity (invisível) com o ponto de pivô como sua posição e faça dela a parent da entity que você quer rotacionar.

Ao rotacionar a parent entity, seus children serão todos rotacionados usando a posição da parent como ponto de pivô. Note que o `position` da child entity está em referência à da parent entity.

```ts
const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)

Tween.setRotate(pivotEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

Note que, neste exemplo, o system está rotacionando a `pivotEntity` entity, que é uma parent da `childEntity` entity.

## Redimensionar entre dois tamanhos

Para alterar a escala de uma entity entre dois tamanhos, crie uma `Tween` component com o `setScale` função.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)	

```

O tween de escala recebe as seguintes informações:

* `start`: Um Vector3 para o tamanho inicial
* `end`: Um Vector3 para o tamanho final
* `duração`: Quantos milissegundos leva para mover entre as duas posições

Este outro parâmetro opcional também está disponível:

* `easingFunction`: Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens)

## Mover a partir da posição atual

Enquanto um tween está ativo, o engine atualiza os `Transform` valores do component da entity em tempo real. Isso significa que você pode ler a posição atual da entity a partir do seu `Transform` component a qualquer momento e usar esse valor como o `start` de um novo tween. A entity então se move de onde quer que esteja atualmente até o alvo, não importa como tenha chegado lá.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover' },
	},
	() => {
		Tween.setMove(myEntity, 
			Transform.get(myEntity).position, 
			Vector3.create(8, 1, 8), 
			2000,
			EasingFunction.EF_EASEOUTQUAD
		)
	}
)
```


> [!WARNING]
> **📔 Nota**: Passe sempre um `start` valor explícito. Se o `start` parâmetro for deixado sem definição, ele é tratado como *(0, 0, 0)*, então a entity teleporta para a origem da scene antes de mover. Da mesma forma, se você passar um hardcoded `start` que não corresponda à posição atual da entity, a entity teleporta visivelmente para essa posição antes de mover. Leia a posição atual da `Transform` component, como nos exemplos desta seção, sempre que quiser que o movimento comece de onde a entity está atualmente.


### Redirecionar durante o trajeto

Chamar `setMove` (ou qualquer uma das outras funções tween) em uma entity que já tem um tween em execução substitui o tween em execução pelo novo. Como a `start` é lido a partir da posição ao vivo da entity, isso redireciona a entity suavemente no meio do trajeto: ela muda de curso de onde quer que esteja naquele momento, sem nenhum salto.

O exemplo abaixo move uma entity para o pad em que o player clicar. Se o player clicar em outro pad enquanto a entity ainda estiver viajando, a entity muda de direção suavemente em direção ao novo alvo.

```ts
function moveTo(target: Vector3) {
	Tween.setMove(shipEntity, 
		Transform.get(shipEntity).position, 
		target, 
		2000,
		EasingFunction.EF_EASEOUTQUAD
	)
}

pointerEventsSystem.onPointerDown(
	{
		entity: padA,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover para cá' },
	},
	() => moveTo(Vector3.create(2, 1, 2))
)

pointerEventsSystem.onPointerDown(
	{
		entity: padB,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover para cá' },
	},
	() => moveTo(Vector3.create(14, 1, 14))
)
```

Para executar lógica quando a entity alcançar seu destino, veja [Ao concluir o tween](#on-tween-finished).


> [!WARNING]
> **📔 Nota**: Redirecione dessa maneira apenas em resposta a eventos ocasionais, como um clique ou alcançar um ponto de passagem. Se o alvo mudar constantemente, por exemplo uma entity que segue o player, não crie um novo tween a cada frame: a entity vai tremer visivelmente. Veja [Seguir um alvo em movimento](#follow-a-moving-target).


## Tweens não lineares

Tweens podem seguir diferentes **Funções de easing** que afetam a taxa de variação ao longo do tempo. Uma **linear** função, significa que a velocidade da mudança é constante do início ao fim. Há muitas opções para escolher, que desenham curvas com formatos diferentes dependendo se o começo e/ou o fim começam devagar, e de quanto. Um **easeinexpo** curva começa devagar e termina rápido, aumentando a velocidade exponencialmente; ao contrário, uma **easeoutexpo** curva começa rápido e termina devagar.


> [!NOTE]
> **💡 Dica**: Experimente diferentes curvas de movimento. As diferenças costumam ser sutis, mas interpretamos inconscientemente informações a partir de como as coisas se movem, como peso, fricção ou até personalidade.


```ts
Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000,
	EasingFunction.EF_EASEOUTBOUNCE
)

```

O parâmetro opcional `easingFunction` recebe seu valor do `EasingFunction` enum, que oferece as seguintes opções:

* `EF_EASEBACK`
* `EF_EASEBOUNCE`
* `EF_EASECIRC`
* `EF_EASECUBIC`
* `EF_EASEELASTIC`
* `EF_EASEEXPO`
* `EF_EASEINBACK`
* `EF_EASEINBOUNCE`
* `EF_EASEINCIRC`
* `EF_EASEINCUBIC`
* `EF_EASEINELASTIC`
* `EF_EASEINEXPO`
* `EF_EASEINQUAD`
* `EF_EASEINQUART`
* `EF_EASEINQUINT`
* `EF_EASEINSINE`
* `EF_EASEOUTBACK`
* `EF_EASEOUTBOUNCE`
* `EF_EASEOUTCIRC`
* `EF_EASEOUTCUBIC`
* `EF_EASEOUTELASTIC`
* `EF_EASEOUTEXPO`
* `EF_EASEOUTQUAD`
* `EF_EASEOUTQUART`
* `EF_EASEOUTQUINT`
* `EF_EASEOUTSINE`
* `EF_EASEQUAD`
* `EF_EASEQUART`
* `EF_EASEQUINT`
* `EF_EASESINE`
* `EF_LINEAR`

## Rotação constante

Para fazer uma entity rotacionar constantemente, use o `Tween` component com o `setRotateContinuous` função.

```ts
Tween.setRotateContinuous(myEntity, 
	Quaternion.fromEulerDegrees(0, -1, 0), 
	45
)
```

O tween de rotação contínua recebe as seguintes informações:

* `entity`: A entity a rotacionar
* `direção`: Uma Quaternion que determina o eixo ao redor do qual rotacionar. Apenas o eixo dessa rotação importa; o tamanho do seu ângulo é ignorado. Por exemplo, `Quaternion.fromEulerDegrees(0, -1, 0)` e `Quaternion.fromEulerDegrees(0, -90, 0)` comportam-se da mesma forma, ambas rotacionando ao redor do *y* eixo em uma direção negativa.
* `speed`: Quantos graus por segundo a entity vai rotacionar. Um valor negativo rotaciona na direção oposta.

Este outro parâmetro opcional também está disponível:

* `duração`: Quantos milissegundos manter a rotação. Depois desse tempo, a rotação irá parar.

## Movimento constante

Para fazer uma entity mover-se constantemente na mesma direção, use o `Tween` component com o `setMoveContinuous` função.

```ts
Tween.setMoveContinuous(myEntity, 
	Vector3.create(0, 0, 1), 
	0.7
)
```

O tween de movimento contínuo recebe as seguintes informações:

* `entity`: A entity a mover
* `direção`: Um Vector3 para o movimento
* `speed`: Quantos metros por segundo a entity vai se mover

Este outro parâmetro opcional também está disponível:

* `duração`: Quantos milissegundos manter o movimento. Depois desse tempo, o movimento irá parar.

O tween de movimento contínuo recebe as seguintes informações:

### Seguir um alvo em movimento

`setMoveContinuous` também é a ferramenta certa quando o destino continua mudando, por exemplo, uma entity que segue o player. É tentador criar, em vez disso, um novo `setMove` tween a cada frame, apontado para a posição mais recente do player, mas isso produz um tremor visível.

A razão é que a posição que você lê do `Transform` component de uma entity com tween é a posição que o engine relatou por último para a scene, então ela fica alguns frames atrás de onde a entity realmente está. Quando você passa esse valor como o `start` de um novo tween, o engine o aplica imediatamente, e a entity salta de volta para onde estava alguns frames atrás. Uma dessas correções é imperceptível, mas repeti-la muitas vezes por segundo não é.

`setMoveContinuous` evita o problema por completo, porque você dá a ele uma direção e uma velocidade em vez de um ponto de partida. O engine continua usando a posição real da entity, então substituir o tween para mudar a direção nunca faz a entity saltar.

```ts
const CHASE_SPEED = 3
const STOP_DISTANCE = 1

engine.addSystem(() => {
	const playerPosition = Transform.get(engine.PlayerEntity).position
	const myPosition = Transform.get(myEntity).position

	if (Vector3.distance(myPosition, playerPosition) <= STOP_DISTANCE) {
		// Suficientemente perto: um tween contínuo não tem destino, então você o para manualmente
		if (Tween.has(myEntity)) {
			Tween.deleteFrom(myEntity)
		}
		return
	}

	const direction = Vector3.subtract(playerPosition, myPosition)
	direction.y = 0 // mantenha-se no chão, mesmo que o player pule

	Tween.setMoveContinuous(myEntity, Vector3.normalize(direction), CHASE_SPEED)
})
```


> [!WARNING]
> **📔 Nota**: Um tween contínuo nunca termina sozinho, então a entity continua se movendo até que você remova o `Tween` component com `Tween.deleteFrom()`, ou até que o `duração` acabe. Observe também que reajustar a mira a cada frame envia uma atualização ao engine a cada frame. Para evitar isso, chame `setMoveContinuous` novamente apenas quando a direção tiver mudado o suficiente para fazer diferença.


## Sequências de Tween

Para fazer uma entity reproduzir uma série de tweens em sequência, use o `TweenSequence` component. Este component requer dois campos:

* `sequence`: Um array com várias definições de tween, que serão executadas sequencialmente. O array pode estar vazio, caso em que apenas reproduz o tween atual.
* `loop` *(opcional)*: Se não for fornecido, a sequence é reproduzida apenas uma vez. Se o campo estiver presente, o valor deve ser um valor do `TweenLoop` enum. Os valores aceitos são:
  * `TL_RESTART`: Quando a sequence termina, ela reinicia. Se o último state não corresponder ao primeiro state, a entity salta instantaneamente de um para o outro.
  * `TL_YOYO`: Quando a sequence termina, ela volta, fazendo todos os tweens em ordem inversa até alcançar o início novamente. Depois começa mais uma vez.

### Mover para frente e para trás

Para fazer uma platform mover-se constantemente para frente e para trás entre duas posições, deixe o `sequence` array vazio e defina `loop` como `TweenLoop.TL_YOYO`

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)

TweenSequence.create(myEntity, { sequence: [], loop: TweenLoop.TL_YOYO })
```

A entity se moverá para frente e para trás entre o ponto inicial e o ponto final, com a mesma duração e a mesma easing function em ambas as direções.

### Seguir um caminho

Para fazer uma entity seguir um caminho mais complexo com vários pontos, forneça uma lista de definições de tween no `sequence` de um `TweenSequence` component.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(6.5, 7, 4), 
	Vector3.create(6.5, 7, 12), 
	4000
)

TweenSequence.create(myEntity, {
	sequence: [
		{
			duration: 2000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 7, 12),
				end: Vector3.create(6.5, 10.5, 12),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 12),
				end: Vector3.create(6.5, 10.5, 4),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 4),
				end: Vector3.create(6.5, 7, 4),
			}),
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

Observe que, ao definir um tween dentro de uma TweenSequence, você precisa usar o formato mais detalhado de `Tween.Mode.Move`, ou `Tween.Mode.Rotate`, ou `Tween.Mode.Scale` para definir o tween. Neste formato mais detalhado, você precisa especificar:

* `duração`: Quantos milissegundos leva para mover entre as duas posições
* `easingFunction`: Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens). Neste formato, o valor é obrigatório.
* `mode`: O modo do tween, que pode ser `Tween.Mode.Move`, `Tween.Mode.Rotate`, ou `Tween.Mode.Scale`.

E, dentro do `mode` campo, você precisa especificar:

* `start`: O valor inicial do tween
* `end`: O valor final do tween

## Ao concluir o tween

Use `tweenSystem.tweenCompleted` para detectar quando um tween terminou. Isso pode ser útil para realizar ações quando um tween termina, por exemplo, para abrir a porta de um elevador.

```ts
engine.addSystem(() => {
	const tweenCompleted = tweenSystem.tweenCompleted(myEntity)
	if (tweenCompleted) {
		// reproduzir som
	}
})
```

## Tweens simultâneos

Para mover, rotacionar e redimensionar uma entity entre um estado inicial e um estado final, crie uma `Tween` component com o `setMoveRotateScale` função. Essa função também pode ser usada em qualquer combinação desses três parâmetros.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(14, 1, 2),
})
MeshRenderer.setBox(myEntity)

Tween.setMoveRotateScale(myEntity, {
	position: { start: Vector3.create(14, 1, 2), end: Vector3.create(14, 3, 2) },
	rotation: { start: Quaternion.fromEulerDegrees(0, 0, 0), end: Quaternion.fromEulerDegrees(0, 180, 90) },
	scale: { start: Vector3.One(), end: Vector3.create(2, 0.5, 2) },
	duration: 2000
})
```

O tween de movimento recebe as seguintes informações:

* `entity`: A entity a mover
* `params`: Um objeto com os seguintes parâmetros
  * `position`: *(opcional)* Um objeto com um `start` e `end` valor, ambos como `Vector3`.
  * `rotation`: *(opcional)* Um objeto com um `start` e `end` valor, ambos como `Quaternion`.
  * `scale`: *(opcional)* Um objeto com um `start` e `end` valor, ambos como `Vector3`. Pelo menos um de `position`, `rotation` ou `scale` deve ser fornecido.
  * `duração`: *(obrigatório)* Quantos milissegundos leva para fazer a transição entre os dois conjuntos de valores
  * `easingFunction`: *(opcional)* Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens)

Uma entity pode ter apenas um `Tween` component, e cada component de tween pode executar apenas uma transformação de cada vez. Por meio do `setMoveRotateScale` tipo de tween, você pode fazer uma entity mover-se lateralmente e também rotacionar ao mesmo tempo, mas ambos esses movimentos seguirão a mesma timeline. Se precisar que as transições sejam independentes umas das outras, como solução alternativa, você pode usar entities com parent. Por exemplo, você pode ter uma parent entity invisível que se move lateralmente, com um child visível que rotaciona.

No trecho a seguir, uma parent entity rotaciona enquanto um child aumenta de escala.

```ts
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(parentEntity)
Tween.setRotate(parentEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	5000
)

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(0, 0, 0),
	parent: parentEntity,
})
MeshRenderer.setBox(childEntity)
Tween.setScale(childEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)
```

## Pausar um tween

Para pausar um tween, altere a `playing` propriedade para false. Para retomá-lo, altere-a de volta para true.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: 'pausar' },
	},
	() => {
		let tweenData = Tween.getMutable(myEntity)
		tweenData.playing = !tweenData.playing
	}
)
```

Para encerrar um tween que não precisa continuar, delete o `Tween` component da entity. Se a entity também estava usando um `TweenSequence` component, delete-o também.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: 'pausar' },
	},
	() => {
		if( Tween.has(myEntity)){
			Tween.deleteFrom(myEntity)
		}
		if( TweenSequence.has(myEntity)){
			TweenSequence.deleteFrom(myEntity)
		}
	}
)
```

## Tweens baseados em um sistema

Em vez de usar o componente Tween e deixar o engine lidar com a transformação, você pode preferir fazer essa transição de forma incremental, frame a frame, por meio de um [sistema](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) em sua scene. Movendo a entidade uma pequena quantidade a cada vez que a função é executada.

Por um lado, isso lhe dá mais controle para recalcular movimentos a cada frame. Por outro lado, o código é mais complicado, e jogadores com máquinas menos performáticas podem perceber o tween como travado, notando cada incremento.

### Mover via sistema

A maneira mais fácil de mover uma entidade é modificar gradualmente o *position* valor armazenado no `Transform` component.

```ts
function SimpleMove() {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), 0.05)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

Neste exemplo, estamos movendo uma entidade em 0,05 metros por tick do loop do jogo.

`Vector3.Forward()` retorna um vetor que aponta para a frente e mede 1 metro de comprimento. Neste exemplo, em seguida escalamos esse vetor para 1/20 do seu comprimento com `Vector3.scale()`. Se nossa scene tiver 30 frames por segundo, a entidade está se movendo a uma velocidade de 1,5 metros por segundo.

![](/docs/_remote/90ee9b2fdf4c6fb2a573ce79.webp)

### Rotacionar via sistema

A maneira mais fácil de rotacionar uma entidade é alterar gradualmente os valores no componente Transform de forma incremental, e executar isso como parte da função de um sistema.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

Observe que, para combinar a rotação atual com cada incremento, estamos usando `Quaternion.multiply`. Em matemática de quaternion, você combina duas rotações multiplicando-as, NÃO somando-as. A rotação resultante de multiplicar um quaternion por outro será a rotação final equivalente depois de primeiro realizar uma rotação e depois a outra.

Neste exemplo, estamos rotacionando a entidade em 1 grau na direção para cima a cada tick do loop do jogo.


> [!NOTE]
> **💡 Dica**: Para fazer uma entidade girar sempre para encarar o player, você pode adicionar um [`Billboard` componente](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#face-the-player).


![](/docs/_remote/13d2f2aa20f3bb7131ff9274.webp)

### Rotacionar via sistema em torno de um ponto de pivô

Ao rotacionar uma entity, a rotação é sempre em referência à coordenada central da entity. Para rotacionar uma entity usando outro conjunto de coordenadas como ponto de pivô, crie uma segunda entity (invisível) com o ponto de pivô como sua posição e faça dela a parent da entity que você quer rotacionar.

Ao rotacionar a parent entity, seus children serão todos rotacionados usando a posição da parent como ponto de pivô. Note que o `position` da child entity está em referência à da parent entity.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(pivotEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)
```

Note que, neste exemplo, o system está rotacionando a `pivotEntity` entity, que é uma parent da `childEntity` entity.

![](/docs/_remote/3c145695aa57e42efe942e39.webp)

### Ajustar o movimento ao tempo delta

Suponha que o player visitando sua scene esteja tendo dificuldade para acompanhar o ritmo da taxa de quadros. Isso pode fazer com que o movimento pareça aos solavancos, já que nem todos os frames têm o mesmo intervalo, mas cada um move a entidade pela mesma quantidade.

Você pode compensar esse tempo irregular usando o `dt` parâmetro para ajustar a escala do movimento.

```ts
function SimpleMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), dt)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

O exemplo acima mantém o movimento aproximadamente na mesma velocidade do exemplo de movimento acima, mesmo se a taxa de quadros cair. Ao rodar a 30 frames por segundo, o valor de `dt` é 1/30.

Você também pode suavizar rotações da mesma forma, multiplicando a quantidade de rotação por `dt`.

### Mover entre dois pontos via sistema

Se você quiser que uma entidade se mova suavemente entre dois pontos, use o *lerp* (interpolação linear) algoritmo. Esse algoritmo é muito conhecido no desenvolvimento de games, pois é realmente útil.

O `lerp()` função recebe três parâmetros:

* O vetor para a posição de origem
* O vetor para a posição alvo
* A quantidade, um valor de 0 a 1 que representa qual fração da translação deve ser feita.

```ts
const originVector = Vector3.Zero()
const targetVector = Vector3.Forward()

let newPos = Vector3.lerp(originVector, targetVector, 0.6)
```

O algoritmo de interpolação linear encontra um ponto intermediário no caminho entre ambos os vetores que corresponde à quantidade fornecida.

Por exemplo, se o vetor de origem for *(0, 0, 0)* e o vetor alvo for *(10, 0, 10)*:

* Usar uma quantidade de 0 retornaria *(0, 0, 0)*
* Usar uma quantidade de 0,3 retornaria *(3, 0, 3)*
* Usar uma quantidade de 1 retornaria *(10, 0, 10)*

Para implementar isso `lerp()` em sua scene, recomendamos criar um [componente personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) para armazenar as informações necessárias. Você também precisa definir um sistema que implemente o movimento gradual em cada frame.

```ts
// defina componente personalizado
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// defina sistema
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	}
}

engine.addSystem(LerpMove)

// criar entidade
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

![](/docs/_remote/751a298f237c2c997d52fdec.webp)

### Seguir suavemente um alvo em movimento via sistema

Para suavizar uma entidade em direção a um alvo que continua se movendo, como um pet que segue o player, mova-a uma parte da distância restante a cada frame. Não use uma parte fixa por frame, como `Vector3.lerp(current, target, 0.1)`: o resultado então depende da taxa de quadros, então a entidade alcança o alvo mais rápido em um computador rápido do que em um lento. Calcule a parte a partir de `dt` em vez disso:

```ts
const follower = engine.addEntity()
Transform.create(follower, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(follower)

const RATE = 5 // Valores mais altos alcançam o alvo mais rápido

function FollowSystem(dt: number) {
	const transform = Transform.getMutable(follower)
	const target = Transform.get(engine.PlayerEntity).position
	// O mesmo suavizamento em qualquer taxa de quadros
	const t = 1 - Math.exp(-RATE * dt)
	transform.position = Vector3.lerp(transform.position, target, t)
}

engine.addSystem(FollowSystem)
```

A mesma fórmula funciona para rotações com `Quaternion.slerp()`.

### Rotacionar entre dois ângulos via sistema

Para rotacionar suavemente entre dois ângulos, use o *slerp* (*esférica* algoritmo de interpolação linear. Esse algoritmo é muito semelhante a um *lerp*, mas ele lida com rotações de quaternion.

O `slerp()` função recebe três parâmetros:

* O [quaternion](https://en.wikipedia.org/wiki/Quaternion) ângulo para a rotação de origem
* O [quaternion](https://en.wikipedia.org/wiki/Quaternion) ângulo para a rotação alvo
* A quantidade, um valor de 0 a 1 que representa qual fração da translação deve ser feita.


> [!NOTE]
> **💡 Dica**: Você pode passar valores de rotação em [euler](https://en.wikipedia.org/wiki/Euler_angles) graus (de 0 a 360) usando `Quaternion.fromEulerDegrees()`.


```ts
const originRotation = Quaternion.fromEulerDegrees(0, 90, 0)
const targetRotation = Quaternion.fromEulerDegrees(0, 0, 0)

let newRotation = Quaternion.slerp(originRotation, targetRotation, 0.6)
```

Para implementar isso em sua scene, recomendamos armazenar os dados que entram na `Slerp()` em um [componente personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/). Você também precisa definir um sistema que implemente a rotação gradual em cada frame.

```ts
// defina componente personalizado
const RotateSlerpData = {
	start: Schemas.Quaternion,
	end: Schemas.Quaternion,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const SlerpData = engine.defineComponent('SlerpData', RotateSlerpData)

// defina sistema
function SlerpRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let slerpData = SlerpData.getMutable(myEntity)
	if (slerpData.fraction < 1) {
		slerpData.fraction += dt * slerpData.speed
		transform.rotation = Quaternion.slerp(
			slerpData.start,
			slerpData.end,
			slerpData.fraction
		)
	}
}

engine.addSystem(SlerpRotate)

// criar entidade
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

SlerpData.create(myEntity, {
	start: Quaternion.fromEulerDegrees(0, 0, 0),
	end: Quaternion.fromEulerDegrees(0, 180, 0),
	fraction: 0,
	speed: 0.3,
})
```


> [!WARNING]
> **📔 Nota**: Você poderia, em vez disso, representar a rotação com ângulos euler como `Vector3` valores e usar uma `Lerp()` função, mas isso implicaria uma conversão de `Vector3` como `Quaternion` em cada frame. Os valores de rotação são armazenados internamente como quaternions no `Transform` componente, então é mais eficiente para a scene trabalhar com quaternions.


![](/docs/_remote/e4559d5d1a49351c4ded356f.webp)

Uma abordagem mais simples, mas menos eficiente, para isso aproveita o `Quaternion.rotateTowards` função e evita usar quaisquer componentes personalizados.

```ts
function SimpleRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.rotateTowards(
		transform.rotation,
		Quaternion.fromEulerDegrees(90, 0, 0),
		dt * 10
	)
	if (
		Quaternion.angle(transform.rotation, Quaternion.fromEulerDegrees(90, 0, 0)) < 0.01
	) {
		console.log('done')
		engine.removeSystem(SimpleRotate)
	}
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
	rotation: Quaternion.fromEulerDegrees(0, 0, 90),
})

MeshRenderer.setBox(myEntity)
```

No exemplo acima `Quaternion.rotateTowards` recebe três argumentos: a rotação inicial, a rotação final desejada e o incremento máximo por frame. Neste caso, como o incremento máximo é de `dt * 10` graus, a rotação será executada ao longo de um período de cerca de 9 segundos.

Observe que o sistema também verifica se a rotação está concluída e, se estiver, remove o sistema do engine. Caso contrário, o sistema continuaria fazendo cálculos a cada frame, mesmo depois de a rotação estar concluída.

### Alterar a escala entre dois tamanhos via sistema

Se você quiser que uma entidade mude de tamanho suavemente e sem alterar suas proporções, use o *lerp* (interpolação linear) algoritmo do `objeto Scalar` .

Caso contrário, se você quiser alterar o eixo em proporções diferentes, use `Vector3` para representar a escala de origem e a escala alvo, e depois use a *lerp* função do `Vector3`.

O `lerp()` função do `objeto Scalar` objeto recebe três parâmetros:

* Um número para a escala de origem
* Um número para a escala alvo
* A quantidade, um valor de 0 a 1 que representa qual fração da escala deve ser feita.

```ts
const originScale = 1
const targetScale = 10

let newScale = Scalar.lerp(originScale, targetScale, 0.6)
```

Para implementar essa lerp em sua scene, recomendamos criar um componente personalizado para armazenar as informações necessárias. Você também precisa definir um sistema que implemente a escala gradual em cada frame.

```ts
// defina componente personalizado
const ScaleTransportData = {
	start: Schemas.Number,
	end: Schemas.Number,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const ScaleTransformComponent = engine.defineComponent(
	'ScaleTransformComponent',
	ScaleTransportData
)

// defina sistema
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = ScaleTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const newScale = Scalar.lerp(lerp.start, lerp.end, lerp.fraction)
		transform.scale = Vector3.create(newScale, newScale, newScale)
	}
}

engine.addSystem(LerpMove)

// criar entidade
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

ScaleTransformComponent.create(myEntity, {
	start: 1,
	end: 2,
	fraction: 0,
	speed: 1,
})

Vector3.create(1, 1, 1)
```

![](/docs/_remote/3b128506c9be0c97b7264b06.webp)

### Mover em velocidades irregulares entre dois pontos via sistema

Ao usar o método lerp, você pode tornar a velocidade do movimento não linear. No exemplo anterior, incrementamos a quantidade da lerp por uma quantidade dada a cada frame, mas também poderíamos usar uma função matemática para aumentar o número exponencialmente ou de outras maneiras que lhe deem um ritmo de movimento diferente.

Você também poderia usar uma função que forneça resultados recorrentes, como uma função seno, para descrever um movimento que vai e vem.

Muitas vezes essas transições não lineares podem dar muita vida a uma scene. Um movimento que acelera ao longo de uma curva ou desacelera gradualmente pode dizer muito sobre a natureza de um objeto ou personagem. Você até poderia aproveitar funções matemáticas que adicionam efeitos de quique.

```ts
// defina componente personalizado
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// defina sistema
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const interpolatedValue = interpolate(lerp.fraction)
		transform.position = Vector3.lerp(lerp.start, lerp.end, interpolatedValue)
	}
}

// map the lerp fraction to an exponential curve
function interpolate(t: number) {
	return t * t
}

engine.addSystem(LerpMove)

// criar entidade
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

O exemplo acima é igual ao exemplo de lerp linear que mostramos antes, mas o `fraction` campo mapeado para um valor não linear em cada tick. Esse valor não linear é usado para calcular a `lerp` função, resultando em um movimento que segue uma curva exponencial.

Você também pode mapear uma transição em rotação ou em escala da mesma forma que mostrado acima, mapeando uma transição linear para uma curva.

![](/docs/_remote/95fb7814c57340842f047b62.webp)

### Seguir um caminho via sistema

Você pode fazer com que uma entidade percorra ciclicamente um array de vetores, realizando um movimento de lerp entre cada um para seguir um caminho mais complexo.

```ts
// defina componente personalizado
const PathTransportData = {
	path: Schemas.Array(Schemas.Vector3),
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
	pathTargetIndex: Schemas.Int,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	PathTransportData
)

// defina sistema
function PathMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	} else {
		lerp.pathTargetIndex += 1
		if (lerp.pathTargetIndex >= lerp.path.length) {
			lerp.pathTargetIndex = 0
		}
		lerp.start = lerp.end
		lerp.end = lerp.path[lerp.pathTargetIndex]
		lerp.fraction = 0
	}
}

engine.addSystem(PathMove)

// criar entidade
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(1, 1, 1),
})

MeshRenderer.setBox(myEntity)

const point1 = Vector3.create(1, 1, 1)
const point2 = Vector3.create(8, 1, 3)
const point3 = Vector3.create(8, 4, 7)
const point4 = Vector3.create(1, 1, 7)

const myPath = [point1, point2, point3, point4]

LerpTransformComponent.create(myEntity, {
	path: myPath,
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
	pathTargetIndex: 1,
})
```

O exemplo acima define um caminho 3D composto por quatro vetores 3D. O `PathTransportData` componente personalizado mantém os mesmos dados usados pelo componente personalizado no *lerp* exemplo acima, mas adiciona um `path` array, com todos os pontos em nosso caminho, e um `pathTargetIndex` campo para controlar qual segmento do caminho está sendo usado no momento.

O sistema é muito semelhante ao sistema no *lerp* exemplo, mas quando uma ação de lerp é concluída, ele define os campos `target` e `origin` para novos valores. Se chegarmos ao fim do caminho, retornamos ao primeiro valor do caminho.

![](/docs/_remote/98be0ad1836095ed41c8b3e5.webp)

## Tweens de textura

Para fazer uma textura deslizar suavemente, use o `Tween` component com o `setTextureMove` função.

```ts
Tween.setTextureMove(myEntity, 
	Vector2.create(0, 0), 
	Vector2.create(1, 0), 
	2000
)
```

O tween de textura recebe as seguintes informações:

* `entity`: A entidade da qual mover a textura
* `start`: Um Vector2 para a posição inicial
* `end`: Um Vector2 para a posição final
* `duração`: Quantos milissegundos leva para mover entre as duas posições

Este outro parâmetro opcional também está disponível:

* `movementType`: (opcional), define se o movimento será no campo offset ou tiling. Por padrão, usa offset.
* `easingFunction`: Qual easing function usar. Veja [Tweens não lineares](#non-linear-tweens).

## Movimento constante de textura

Para fazer uma textura deslizar constantemente, use o `Tween` component com o `setTextureMoveContinuous` função.

```ts
Tween.setTextureMoveContinuous(myEntity, 
	Vector2.create(0, 1), 
	0.7
)
```

O tween contínuo de textura recebe as seguintes informações:

* `entity`: A entidade da qual mover a textura
* `direção`: Um Vector2 para o movimento
* `speed`: Quantas unidades por segundo a entidade se moverá

Este outro parâmetro opcional também está disponível:

* `movementType`: define se o movimento será no campo offset ou tiling. Por padrão, usa offset.
* `duração`: Quantos milissegundos manter o movimento. Depois desse tempo, o movimento irá parar.

Leia mais sobre tweens de textura em [Texture Tweens](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#texture-tweens) seção.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
