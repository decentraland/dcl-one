> Aprenda o essencial sobre entities e components em uma scene do Decentraland

# Entities e Components

As scenes de Decentraland são construídas em torno de [*entities*, *components* e *systems*](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system). Este é um padrão comum usado na arquitetura de vários game engines, que permite fácil composabilidade e escalabilidade.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

## Visão geral

*Entities* são a unidade básica para construir tudo nas scenes de Decentraland. Todos os objetos 3D visíveis e invisíveis e os players de áudio na sua scene serão cada um uma Entity. Uma Entity não é nada mais do que um id, que pode ser referenciado por components. A própria Entity não tem propriedades nem métodos próprios; ela serve simplesmente para agrupar vários components juntos.

*Components* definem as características de uma Entity. Por exemplo, um `Transform` component armazena as coordenadas, rotação e escala da Entity. Um `MeshRenderer` component dá à Entity uma forma visível (como um cubo ou uma esfera) quando renderizado na scene, um `Material` component dá à Entity uma cor ou textura. Você também pode criar components personalizados para atender aos dados necessários da sua scene; por exemplo, um custom `saúde` poderia armazenar o valor de saúde restante de uma Entity e adicioná-lo às Entities que representam inimigos não jogáveis em um jogo.

Se você estiver familiarizado com desenvolvimento web, pense em Entities como o equivalente de *Elementos* em um *DOM* árvore *atributos* daqueles elementos.

No [Scene Editor no Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), você pode ver os components que pertencem a uma Entity selecionando-a.

![](/docs/_remote/f49a93029e97b3d8ef464fb3.webp)


> [!WARNING]
> **📔 Nota**: Em versões anteriores do SDK, Entities eram *objetos* que eram instanciados e podiam ser estendidos para adicionar funções. A partir da versão 7.0 do SDK, Entities são apenas um ID. Essa estrutura se encaixa melhor nos princípios de [programação orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/) e pode ajudar no desempenho da scene.


![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

Components como `Transform`, `Material` ou qualquer um dos *shape* components estão intimamente ligados à renderização da scene. Se os valores desses components mudarem, isso por si só já é suficiente para que o engine altere como a scene é renderizada no próximo frame.

O engine é a parte da scene que fica no meio e gerencia todas as outras partes. Ele determina quais Entities são renderizadas e como os players interagem com elas. Também coordena quais funções de [systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) são executadas e quando.

Components servem para armazenar dados sobre sua Entity referenciada. Eles só podem armazenar esses dados; não podem modificá-los por conta própria. Todas as mudanças nos valores dos components são realizadas por [Systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/). Systems são completamente desacoplados dos components e das próprias Entities. Entities e components são agnósticos em relação a quais *systems* estão agindo sobre eles.

## Sintaxe para entities e components

O exemplo abaixo mostra algumas operações básicas para declarar e configurar Entities e components básicos.

```ts
export function main() {
	// Criar uma Entity
	const door = engine.addEntity()

	// Dar à Entity uma posição por meio de um component Transform
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// Dar à Entity uma shape visível por meio de um component GltfContainer
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```


> [!WARNING]
> **📔 Nota**: Em versões anteriores do SDK, era necessário adicionar manualmente uma Entity ao engine para começar a renderizá-la. A partir da versão 7 do SDK, Entities são adicionadas implicitamente ao engine assim que lhes é atribuído um component.


Quando um component é criado, ele sempre é atribuído a uma Entity pai. Os valores do component então afetam a Entity.


> [!NOTE]
> **💡 Dica**: Em vez de criar Entities uma por uma, você pode gerar uma árvore inteira de Entities e components de uma vez a partir de um [composite](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/composites/) arquivo.


## Remover Entities

Para remover uma Entity do engine, use `engine.removeEntity()`. Esta função retorna um `boolean`: `true` se a Entity foi removida, `false` se a remoção foi recusada.

```ts
export function main() {
	// Criar uma Entity
	const door = engine.addEntity()

	// Dar à Entity uma shape visível por meio de um component GltfContainer
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})

	// Remover Entity
	const removed = engine.removeEntity(door)
	console.log('Entity removida:', removed) // true
}
```

Se uma Entity removida tiver quaisquer Entities filhas, elas mudam seu pai de volta para o default `engine.RootEntity` Entity, que fica posicionada na posição base da scene, com escala de *1*.

### Entities reservadas pelo renderer

Alguns ids de Entity são reservados pelo renderer para avatares de players remotos. Você não pode remover essas Entities. Se você chamar `engine.removeEntity()` em uma Entity reservada pelo renderer, ele retorna `false` e deixa todos os components intocados.

A [Entities reservadas nomeadas](#reserved-entities) (`engine.RootEntity`, `engine.PlayerEntity`, `engine.CameraEntity`) são um caso especial: `engine.removeEntity()` ainda retorna `false` para estas (seus ids nunca são liberados), mas seus components **são** são limpos. `Isso significa que você pode limpar seus próprios components delas (por exemplo, removendo um` em `engine.PlayerEntity`InputModifier

```ts
), embora o próprio id da Entity nunca seja liberado.
// Tentando remover uma Entity reservada
console.log(result) // false — o id da Entity não é liberado
// Mas os próprios components da scene em PlayerEntity são limpos
```

Você pode verificar o valor de retorno ao remover Entities para lidar com casos de borda:

```ts
if (!engine.removeEntity(someEntity)) {
	console.log('Entity não pôde ser removida (reservada)')
}
```

### Remover uma Entity com seus filhos

Para remover uma Entity e também todos os seus filhos (e quaisquer filhos dos seus filhos, recursivamente), use o `removeEntityWithChildren()` helper.

```ts
export function main() {
	// Criar entity pai
	const door = engine.addEntity()

	// Criar entity filha
	const doorKnob = engine.addEntity()

	// Dar às Entities uma shape visível
	GltfContainer.create(door, {
		src: 'models/door.glb',
	})
	GltfContainer.create(doorKnob, {
		src: 'models/doorKnob.glb',
	})

	// Pai
	Transform.create(doorKnob, {
		parent: door,
	})

	// Remover tanto o pai quanto os filhos
	removeEntityWithChildren(engine, door)
}
```


> [!WARNING]
> **Nota:** Se uma Entity reservada pelo renderer estiver em qualquer lugar na árvore, `removeEntityWithChildren` remove todos os outros descendentes, mas deixa a Entity reservada no lugar. O `Transform.parent` apontará para uma Entity removida. Isso só acontece se a sua scene colocar uma Entity reservada como filha de uma Entity da scene, o que é incomum.



> [!NOTE]
> **💡 Dica**: Em vez de remover uma Entity do engine, em alguns casos pode ser melhor torná-la invisível, caso você queira carregá-la novamente sem atraso. Veja [Tornar invisível](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/#make-invisible)


### Removendo Entities nos bastidores

Uma Entity é apenas um id referenciado por seus components. Portanto, ao remover uma Entity, você está de fato removendo cada um dos components que referenciam essa Entity. Se você remover manualmente todos os components de uma Entity, para o player parecerá o mesmo que fazer `engine.removeEntity()`. No entanto, `engine.removeEntity()` também realiza alguma contabilidade interna extra, marcando o id da Entity como não estando mais em uso; por isso, essa é sempre a forma recomendada de remover uma Entity.

## Entities aninhadas

Uma Entity pode ter outras Entities como filhas. Graças a isso, podemos organizar Entities em árvores, assim como o HTML de uma página web.

![](/docs/_remote/28a71287061591ece4e52409.webp)

Para definir uma Entity como pai de outra, a Entity filha deve ter um `Transform` component. Você pode então definir o `parent` campo com uma referência à Entity pai.

```ts
export function main() {
	// Criar entities
	const parentEntity = engine.addEntity()

	const childEntity = engine.addEntity()

	// Definir pai
	Transform.create(childEntity, {
		parent: parentEntity,
	})
}
```

Uma vez atribuído um pai, ele pode ser lido na Entity filha a partir do `parent` campo em seu `Transform` component.

```ts
// Obter pai de uma Entity
const parent = Transform.get(childEntity).parent
```

Se uma Entity pai tiver um `Transform` component que afete sua posição, escala ou rotação, suas Entities filhas também são afetadas. Quaisquer valores de posição ou rotação são somados; quaisquer valores de escala são multiplicados.

Se a Entity pai ou a Entity filha não tiver um `Transform` component, os seguintes valores padrão são usados.

* Para **position**o centro do pai é *0, 0, 0*
* Para **rotation** a rotação do pai é o quaternion *0, 0, 0, 1* (equivalente aos ângulos de Euler *0, 0, 0*)
* Para **scale**o pai é considerado como tendo um tamanho de *1*. Qualquer redimensionamento do pai afeta escala e posição proporcionalmente.

Entities sem component de shape são invisíveis na scene. Elas podem ser usadas como wrappers para lidar com e posicionar várias Entities como um grupo.

Para separar uma child Entity do seu pai, você pode atribuir o pai da Entity a `engine.RootEntity`.

```ts
const mutableChildTransform = Transform.getMutable(childEntity)
mutableChildTransform.parent = engine.RootEntity
```


> [!WARNING]
> **📔 Nota**: Ao lidar com Entities aninhadas que estão sincronizadas com outros players, use a `parentEntity()` função em vez da `parent` Entity no Transform. Veja [Entities com parent](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/#parented-entities)


No Scene Editor, você pode ver toda a hierarquia de Entities aninhadas na sua scene no painel esquerdo.

![](/docs/_remote/87863329d94332c21873d9dd.webp)

## Obter uma Entity por ID

Cada Entity na sua scene tem um número único *id*. Você pode recuperar um component que se refere a uma Entity específica do engine com base nesse ID.

```typescript
// buscar um component Transform
Transform.get(1000 as Entity)
```


> [!WARNING]
> **📔 Nota**: Os ids de Entity entre *0* e *511* são reservados pelo engine para Entities fixas, como o avatar do player, a scene base etc.


Por exemplo, se o clique de um player ou um [raycast](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting/) atingir uma Entity, isso retornará o id da Entity atingida, e você pode usar o comando acima para buscar o component Transform da Entity que corresponde a esse id. Você também pode buscar qualquer outro component dessa Entity da mesma maneira.

## Obter uma Entity por nome

Ao adicionar Entities via drag-and-drop no Scene Editor, cada Entity tem um nome único. Use a `engine.getEntityOrNullByName()` função para referenciar uma dessas Entities no seu código. Passe o nome da Entity como uma string, exatamente como escrito na UI do Scene Editor, na visualização em árvore à esquerda.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```


> [!WARNING]
> **📔 Nota**: Certifique-se de usar apenas a `engine.getEntityOrNullByName()` dentro da `main()` função, em funções que executam depois de `main()`, ou em um System. Se usada fora de um desses contextos, as Entities criadas na UI do Scene Editor talvez ainda não estejam instanciadas.


Você pode executar livremente qualquer ação em uma Entity obtida por este método, como adicionar ou remover components, modificar valores de components existentes ou remover a Entity do engine.

```ts
function main() {
	// buscar Entity
	const door = engine.getEntityOrNullByName('door-3')
	// verificar se a Entity existe
	if (door) {
		// adicionar um callback de PointerEvents
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open' },
			},
			function () {
				// abrir porta
			}
		)
	}
}
```

Todas as Entities adicionadas via a UI do Scene Editor têm um `Name` component, você pode iterar sobre todas elas assim:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## Adicionar ou substituir um component

Cada Entity só pode ter um component de um determinado tipo. Por exemplo, se você tentar atribuir um Transform a uma Entity que já tenha um, isso causará um erro.

Para evitar esse erro, você pode usar `.createOrReplace` em vez de `.create`. Este comando sobrescreve quaisquer components existentes do mesmo tipo, se eles existirem; caso contrário, ele cria um novo component assim como `.create`.

```ts
Transform.createOrReplace(door, {
	position: Vector3.create(5, 1, 5),
})
```


> [!WARNING]
> **📔 Nota**: Como `.createOrReplace` executa uma verificação adicional antes de criar o component, é sempre mais eficiente usar `.create`. Se você tiver certeza de que a Entity ainda não tem um component como o que você está adicionando, use `.create`.


## Acessar um component de uma Entity

Você pode acessar os components de uma Entity usando o `.get()` ou os `getMutable()` funções.

```ts
export function main() {
	// Criar Entity
	const box = engine.addEntity()

	// Criar e adicionar um component a essa Entity
	Transform.create(box)

	// Obter versão somente leitura do component
	let transform = Transform.get(box)

	// Obter versão mutável do component
	let transform = Transform.getMutable(box)
}
```

A `get()` função obtém uma referência somente leitura ao component. Você não pode alterar quaisquer valores dessa referência do component.

Se você quiser alterar os valores do component, use a `getMutable()` função em vez disso. Se você alterar os valores na versão mutável do component, estará afetando diretamente a Entity à qual esse component pertence.

Veja [dados mutáveis](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/mutable-data/) para mais detalhes.


> [!WARNING]
> **📔 Nota**: Use `getMutable()` se você realmente for fazer alterações nos valores do component. Caso contrário, use sempre `get()`. Essa prática segue os princípios de [programação orientada a dados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming/)e pode ajudar significativamente no desempenho da scene.


```ts
// Obter versão mutável do component
let transform = Transform.getMutable(box)

// alterar um valor do component
transform.scale.x = 5
```

O exemplo acima modifica diretamente o valor de *x* scale no component Transform.

Se você não tiver certeza absoluta de que a Entity realmente tem o component que está tentando recuperar, use `getOrNull()` ou `getMutableOrNull()`.


> [!WARNING]
> **📔 Nota**: Evite usar `getOrNull()` ou `getMutableOrNull()` quando possível, pois essas funções envolvem verificações adicionais e, portanto, são menos eficientes do que `.get()` e `getMutable()`.


```ts
//  getOrNull
const transformOrNull = Transform.getOrNull(myEntity)

//  getMutableOrNull
const mutableTransformOrNull = Transform.getMutableOrNull(myEntity)
```

Se o component que você está tentando recuperar não existir na Entity:

* `get()` e `getMutable()` retorna um erro.
* `getOrNull()` e `getMutableOrNull()` retorna `Null`.

## Remover um component de uma Entity

Para remover um component de uma Entity, use o `deleteFrom()` método do tipo de component.

```ts
Transform.deleteFrom(myEntity)
```

Se você tentar remover um component que não existe na Entity, essa ação não gerará nenhum erro.


> [!WARNING]
> **📔 Nota**: Para remover todos os components de uma Entity de uma vez, veja [esta seção](#remove-entities)


## Verificar um component

Você pode verificar se uma Entity possui uma instância de um determinado component usando a `has()` função. Essa função retorna *true* se o component estiver presente, e *false* se não estiver. Isso pode ser muito útil para usar em lógica condicional na sua scene.

```ts
const hasTransform = Transform.has(myEntity)
```


> [!NOTE]
> **💡 Dica**: Você também pode [consultar componentes](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/) para obter uma lista completa de components que possuem um component específico, ou um conjunto específico de components. Não percorra manualmente todas as Entities da scene para verificar cada uma com um `has()`, essa abordagem é muito menos eficiente.


## Verificar mudanças em um component

Use o `onChange` função para executar uma função de callback sempre que os valores do component mudarem para uma determinada Entity. Isso funciona em qualquer component e é um ótimo atalho para ajudar a manter seu código legível.

A função de callback pode incluir um parâmetro de entrada que contenha o novo estado do component.

```ts
Transform.onChange(cubeEntity, (newTransform) => {
	if (!newTransform) return
	console.log(
		'Posição do cubo alterada: ',
		newTransform.position,
		newTransform.rotation
	)
})

VisibilityComponent.onChange(cubeEntity, (newVisibilityComponent) => {
	if (!newVisibilityComponent) return
	console.log('Visibilidade do cubo alterada: ', newVisibilityComponent.visible)
})
```

Se o component for removido da Entity, então a função é chamada com uma entrada de `undefined`.


> [!NOTE]
> **💡 Dica**: O `.onChange()` função funciona tanto com components nativos do SDK quanto com [components customizados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) definidos pelo criador.


## Obter child Entities

Para acessar todas as Entities que são child diretas de uma Entity pai, use `getEntitiesWithParent`. Ela recebe como argumentos a `engine` e as `parent` Entity e retorna uma lista de todas as Entities que têm essa Entity específica como pai. Observe que ela retorna apenas child diretas, não filhos de filhos.

```ts
import { getEntitiesWithParent } from '@dcl/sdk/ecs'

const children = getEntitiesWithParent(engine, myEntity)
for (const child of children) {
   // processar cada child Entity
}
```

Para acessar, em vez disso, todos os descendentes de uma Entity, independentemente de quão profundamente aninhados estejam, use a função `getComponentEntityTree()`. Em vez de percorrer manualmente a hierarquia nível por nível, essa função retorna uma lista plana de todos os descendentes, fácil de iterar. Ela também filtra apenas Entities que tenham um component dado ou uma lista de components.

```ts
import { getComponentEntityTree } from '@dcl/sdk/ecs'

export function main() {
	// Criar uma Entity pai com children aninhados
	const parentEntity = engine.addEntity()
	Transform.create(parentEntity, {
		position: Vector3.create(8, 0, 8),
	})

	// ... suponha que o pai tenha vários children e netos

	// Iterar sobre todos os descendentes da Entity pai
	for (const descendantEntity of getComponentEntityTree(
		engine,
		parentEntity,
		Transform
	)) {
		// Acessar cada child descendente
		const transform = Transform.get(descendantEntity)
		console.log('Posição do descendente:', transform.position)
	}
}
```

A `getComponentEntityTree` função recebe três parâmetros:

* `engine`: A instância do engine que está executando as Entities
* `entity`: A Entity raiz a partir da qual começar
* `componente`: O component pelo qual filtrar (normalmente `Transform` para hierarquias espaciais)

A função retorna um generator que produz cada Entity descendente na estrutura em árvore. Somente Entities que tenham o component especificado serão incluídas nos resultados.

Você pode combinar isso com outras verificações de components para encontrar Entities específicas na sua hierarquia:

```ts
// Encontrar todos os descendentes com um nome específico
for (const descendantEntity of getComponentEntityTree(
	engine,
	parentEntity,
	Transform
)) {
	const name = Name.getOrNull(descendantEntity)
	if (name && name.value === 'targetEntity') {
		console.log('Entity alvo encontrada:', descendantEntity)
	}
}
```

## Entities reservadas

Certos ids de Entity são reservados para Entities especiais que existem em toda scene. Eles podem ser acessados pelos seguintes aliases:

* `engine.RootEntity`
* `engine.PlayerEntity`
* `engine.CameraEntity`


> [!WARNING]
> **📔 Nota**: Evite se referir a essas Entities antes de serem inicializadas. Para evitar esse problema, faça referência a essas Entities na `main()` função, ou em um System.


## A Entity raiz

Todas as Entities na scene são filhos da `engine.RootEntity`, direta ou indiretamente.

Essa Entity não possui um component Transform, mas é usada para lidar com vários components que representam configurações mais globais, como [controle de skybox](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control/), [posição do cursor](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#check-the-players-cursor-position), ou [dimensões da tela](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui-positioning/#responsive-ui-size).

## A Entity do player

A `engine.PlayerEntity` Entity representa o avatar do player.

Obtenha o `Transform` component para obter a posição e rotação atuais do player, veja [dados do usuário](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/). O Transform do player é somente leitura; para modificá-lo use a `movePlayerTo()` função, [saiba mais](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/move-player/).

Você também pode anexar objects ao player definindo-os como children dessa Entity, embora o [Attach to Player](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#attach-an-entity-to-an-avatar) seja frequentemente a melhor opção para isso.

## A Entity da câmera

A `engine.CameraEntity` representa a câmera do jogador.

Obtenha o `Transform` component para obter a posição e a rotação da câmera. O Transform desta Entity também é somente leitura. Para modificar o ângulo ou a posição da câmera, use uma [Câmera virtual](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#using-virtual-cameras).

Você também pode obter o `CameraMode` component para saber se o jogador está usando o modo de câmera em 1ª ou 3ª pessoa, veja [modo de câmera](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#check-the-players-camera-mode).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
