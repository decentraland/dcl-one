# Colliders

Saiba mais sobre os diferentes components que dão às entities sua forma 3D e colisão.

Entities que têm colliders ocupam espaço e bloqueiam o caminho do avatar de um player; entities sem colliders podem ser atravessadas pelo avatar de um player.

Colliders também são necessários para tornar uma entity clicável. Os button events se baseiam na forma do collider de uma entity, e não na sua forma visível.

Existem collision layers separadas para interagir com a physics do player ou com pointer events; os colliders podem ser configurados para interagir apenas com uma ou com a outra. Eles também podem ser configurados para interagir com custom layers, que podem ser usadas com [raycasts](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/) para lidar com o que fizer sentido para a scene.


> [!WARNING]
> **📔 Nota**: Os colliders não afetam como outras entities interagem entre si; as entities podem sempre se sobrepor. As configurações de colisão afetam apenas como a entity interage com o avatar do player e os button events. Decentraland não tem uma engine de física nativa, então, se você quiser que as entities caiam, colidam ou quichem, você deve programar esse comportamento na scene ou importar uma library para lidar com isso.


## Use o Scene Editor

A maneira mais fácil de gerenciar os colliders de uma entity é usar o [Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/).

Você pode adicionar um **Mesh Collider** component à sua entity para atribuir uma forma primitiva (cubo, plano, esfera, cilindro ou cone) à sua entity. Depois, você pode escolher [Collision layers](#collision-layers) em um menu suspenso.

Você também pode configurar as collision layers em um **GLTF** component para alterar o padrão [Collision layers](#collision-layers) usado tanto na geometria do collider quanto na geometria visível do model. Veja [Add Components](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components).

![](/docs/_remote/d21ae0005c271ae9c97a8144.webp)

## Colliders em formas primitivas

O `MeshCollider` component dá a uma entity um collider simples baseado em uma forma primitiva (boxes, spheres, planes, cylinders ou cones).

Entities que têm um `MeshRenderer` component para lhes dar uma [forma primitiva](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/#primitive-shapes) não têm colliders por padrão. Você também deve dar à entity um `MeshCollider` component.

As seguintes collider shapes estão disponíveis em `MeshCollider`. Várias shapes incluem campos adicionais opcionais, específicos para essa shape.

* **box**:

  Use `MeshCollider.setBox()`, passando a entity.
* **plane**:

  Use `MeshCollider.setPlane()`, passando a entity.
* **sphere**:

  Use `MeshCollider.setSphere()`, passando a entity.
* **cylinder**:

  Use `MeshCollider.setCylinder()`, passando a entity. Passe `radiusTop` e `radiusBottom` como campos adicionais opcionais, para modificar o cylinder.


> [!NOTE]
> **💡 Dica**: Defina qualquer um deles como 0 para formar um cone. `radiusTop` ou `radiusBottom` to 0 para formar um cone.


Este exemplo define uma box entity que não pode ser atravessada.

```ts
// criar entity
const myCollider = engine.addEntity()

// forma visível
MeshRenderer.setBox(myCollider)

// collider
MeshCollider.setBox(myCollider)
```

A forma usada pelo `MeshCollider` não precisa necessariamente corresponder à usada pelo `MeshRenderer`. Você também pode adicionar um `MeshCollider` a uma entity que tenha um modelo 3D de um `GltfContainer` component, ou a uma entity que não tenha nenhuma forma visível.


> [!WARNING]
> **📔 Nota**: A `MeshCollider` component e `ColliderLayer` devem ser importados via
> 
> `import { MeshCollider, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Colliders em modelos 3D

Modelos 3D podem receber colliders em dois níveis diferentes de geometria:

* `visibleMeshesCollisionMask`: Se refere à geometria visível do model. Por padrão, essa geometria não tem colliders.
* `invisibleMeshesCollisionMask`: se refere às collider meshes, cujos nomes terminam em `_collider`. Por padrão, essa geometria é tratada como um collider tanto para physics quanto para pointer events.

Qualquer mesh incorporada como parte de um modelo 3D cujo nome termina em `_collider` é tratada como parte da `invisibleMeshesCollisionMask` layer e interpretada como um collider por padrão.

Definir a geometria do collider como uma layer invisível separada permite um controle muito maior e exige bem menos do system do que usar a geometria visível, já que o objeto de colisão costuma ser muito mais simples (com menos vertices) do que o modelo original.

Se um model não tiver nenhuma geometria de collider e você quiser fazer com que ele afete os systems de physics ou pointer events, você pode:

* Atribua collision layers diretamente à geometria visível, via o `visibleMeshesCollisionMask`.


> [!WARNING]
> **📔 Nota**: Se a geometria visível do objeto tiver muitos vertices, note que isso pode ter um custo de performance maior.


* Dê à entity um `MeshCollider` component, para lhe dar um collider de forma primitiva.
* Sobreponha uma entity invisível que tenha um `MeshCollider` component.
* Edite o model em uma ferramenta externa como Blender para incluir um *collider mesh*. O collider deve se chamar *x\_collider*, onde *x* é o nome do model. Então, para um model chamado *house*, o collider deve se chamar *house\_collider*.

Talvez você também queira atribuir a collision layer de pointer events à `visibleMeshesCollisionMask` caso queira que as hover hints e os pointer events respondam com mais precisão ao contorno da entity. Note que isso exige mais de performance.


> [!WARNING]
> **📔 Nota**: Certifique-se de não ter a mesma layer (physics, pointer events ou custom layers) atribuída a ambos `visibleMeshesCollisionMask` e `invisibleMeshesCollisionMask`, pois isso seria um uso muito ineficiente dos recursos. Você pode ter layers diferentes em cada um, como physics na layer invisível e pointer events na layer visível.


```ts
// criar entity
const myEntity = engine.addEntity()

// atribuir forma GLTF
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})
```

Veja [modelos 3D](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) para mais detalhes sobre como adicionar geometria invisível de collider a um modelo 3D.


> [!WARNING]
> **📔 Nota**: A `GltfContainer` component e `ColliderLayer` devem ser importados via
> 
> > `import { GltfContainer, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


### modelos animados

Ao definir colliders para usar a geometria visível em um model que inclui [armature-based animations](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/animations/), as animações não são seguidas pelos colliders. As collider meshes mantêm sua forma original. Se uma animação envolver deformar a geometria de uma mesh, as collider meshes retêm a forma sem animação enquanto a animação é reproduzida.

Ao reproduzir animações que envolvem mover meshes inteiras sem alterar sua forma, essas mudanças são refletidas com precisão pelos colliders. Por exemplo, se uma plataforma se move como parte de uma animação, o collider da plataforma também se move com a animação.

## Collision layers

A scene pode lidar com collision layers separadas, que têm comportamentos diferentes.

Você pode configurar um `MeshCollider` component ou o `GltfContainer` component para responder apenas a um tipo de interação, ou a vários deles, ou a nenhum. Para isso, no `MeshCollider` defina o `collisionMask` property, e em `GltfContainer` defina o `visibleMeshesCollisionMask` ou `invisibleMeshesCollisionMask` properties para um ou vários dos seguintes valores:

* `ColliderLayer.CL_PHYSICS`: Bloqueia o movimento do player (paredes, pisos, plataformas da scene). Não afeta pointer events.
* `ColliderLayer.CL_POINTER`: Responde apenas a pointer events. Não bloqueia o movimento do player.
* `ColliderLayer.CL_PLAYER`: Marca o collider como um avatar marker. Raycasts e trigger areas que tenham como alvo `CL_PLAYER` o detectam, mas a capsule do player o atravessa diretamente (sem bloqueio físico). Em meshes de scene, isso é útil para marcar uma mesh como "um alvo no estilo de avatar" apenas para detecção.
* `ColliderLayer.CL_MAIN_PLAYER`: Como `CL_PLAYER`, mas direcionado apenas ao player local. Raycasts e trigger areas com `CL_MAIN_PLAYER` em sua mask o detectam; raycasts/triggers de remote-avatar não.
* `ColliderLayer.CL_CUSTOM1` até `CL_CUSTOM8`: Pode ser usado junto com raycasts e trigger areas para detectar colisões apenas com custom layers específicas.
* `ColliderLayer.CL_NONE`: Não responde a colisões de nenhum tipo.


> [!NOTE]
> **💡 Dica**: `CL_PLAYER` e `CL_MAIN_PLAYER` em uma `MeshCollider` / `GltfContainer` são **apenas de detecção** layers — a capsule do player passa por elas. Se você quiser que a mesh seja TANTO detectável como avatar QUANTO bloqueie fisicamente o player, combine a layer de avatar com `CL_PHYSICS` (ex. `CL_PHYSICS | CL_MAIN_PLAYER`).



> [!WARNING]
> **📔 Nota**: Para desativar colisões de um `MeshCollider` component, exclua o component. Não defina a collision layer como `ColliderLayer.CL_NONE`. Há um problema conhecido com o `MeshCollider` component. Em vez de desativar todas as colisões, ele torna esse valor equivalente ao padrão (`ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`).


```ts
// criar entity
const myEntity = engine.addEntity()
// forma visível
MeshRenderer.setBox(myEntity)

// criar um component MeshCollider que responde apenas à physics do player
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)
```

Uma única collision mask pode responder a várias collision layers. Use o `|` caractere como um *ou*, para incluir quantas layers você precisar. O valor padrão em um MeshCollider é `ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`.

```ts
MeshCollider.setBox(
	myEntity,
	ColliderLayer.CL_CUSTOM1 |
		ColliderLayer.CL_CUSTOM3 |
		ColliderLayer.CL_PHYSICS |
		ColliderLayer.CL_POINTER
)
```

Você pode usar as 8 custom layers diferentes para o que melhor se adequar à sua scene; por exemplo, uma pode ser usada para cálculos de line-of-sight de NPCs, enquanto outra para estimar trajetórias de objetos em queda. Usar layers diferentes para systems diferentes permite usar menos resources, já que, em cada caso, você verificará apenas colisões com as entities relevantes.

Veja [Raycasting](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting/) para saber mais sobre como usar custom collision layers.

### Câmeras e colliders

Quando a câmera de um player se move no modo de terceira pessoa, a câmera pode ser bloqueada por colliders ou não, dependendo das collision layers atribuídas às entities. Tenha isso em mente ao projetar sua scene; talvez você queira impedir que a câmera atravesse paredes ou outras entities.

Para evitar que a câmera atravesse paredes, você deve atribuir tanto a `ColliderLayer.CL_PHYSICS` e a `ColliderLayer.CL_POINTER` layers às entities que você quer que bloqueiem a câmera. É importante que ambas as layers sejam atribuídas à mesma geometry na entity. Então, se você atribuir a `ColliderLayer.CL_PHYSICS` layer à layer visível da entity, você também deve atribuir a `ColliderLayer.CL_POINTER` layer à mesma geometry.

Por exemplo, no Creator Hub, a seguinte combinação de configurações impedirá que a câmera atravesse paredes:

![](/docs/_remote/1b4a254a3e56b4b32b988fb4.webp)

Ambas as `ColliderLayer.CL_PHYSICS` e a `ColliderLayer.CL_POINTER` layers são atribuídas à mesma layer invisível da geometry da entity. Se ambas fossem atribuídas à layer visível, o resultado seria o mesmo. Esse é o comportamento padrão, tanto ao adicionar uma entity via o Creator Hub quanto via código.

![](/docs/_remote/6d4572b10a1972901a7ea232.webp)

Neste segundo exemplo, a câmera pode atravessar a parede, porque a `ColliderLayer.CL_PHYSICS` layer é atribuída à layer invisível da entity, e a `ColliderLayer.CL_POINTER` layer é atribuída à layer visível da entity, mesmo que ambas as geometrias tenham a mesma forma geral.

```ts
// SEM CÂMERA ATRAVESSANDO A PAREDE
// padrão (tanto pointer quanto physics usam a geometria invisível)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// SEM CÂMERA ATRAVESSANDO A PAREDE
// Ambas usam a mesma geometria invisível
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// SEM CÂMERA ATRAVESSANDO A PAREDE
// Ambas usam a mesma geometria visível
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	visibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// SIM, A CÂMERA ATRAVESSA A PAREDE
// physics e pointer estão em layers diferentes
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// SIM, A CÂMERA ATRAVESSA A PAREDE
// physics e pointer estão em layers diferentes
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
	visibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
})
```

### Bloqueio de ponteiro

Apenas shapes que têm colliders podem ser ativadas com [pointer events](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/). Uma entity também precisa ter um collider para bloquear pointer events de passarem por ela e evitar atingir entities atrás dela. Portanto, por exemplo, um player não pode pegar algo que esteja trancado dentro de um chest, se o chest tiver colliders ao redor. Os pointer events do player são afetados apenas por meshes que estejam ativas na `ColliderLayer.CL_POINTER` layer.

Por padrão, um MeshCollider afeta tanto as layers Physics quanto Pointer, mas você pode alterar esse valor para afetar apenas uma, ou nenhuma, e afetar custom layers em vez disso.


> [!WARNING]
> **📔 Nota**: Além dos colliders, uma entity também precisa ter um `PointerEvents` component para responder a pointer events. Os `pointerEventsSystem` helpers também cuidam desse requisito.


```ts
// responde apenas à physics do player
// por exemplo, uma parede invisível pela qual você não pode passar, mas pode clicar através dela
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)

// responde apenas ao pointer do player
// por exemplo, um item que você pode clicar para pegar, mas pode atravessar sem problemas
MeshCollider.setBox(myEntity2, ColliderLayer.CL_POINTER)
```

Por padrão, a geometria visível de um `GltfContainer` não é mapeada para nenhuma collision layer, mas a geometria invisível afeta tanto as layers Physics quanto Pointer. Você pode alterar esse valor para afetar apenas uma, ou nenhuma, e afetar custom layers em vez disso. Você também pode configurar a layer da geometria visível da mesma maneira.

```ts
// padrão (tanto pointer quanto physics usam a geometria invisível)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// a physics do player usa a geometria invisível mais simples
// pointer events usam o contorno completo e detalhado da geometria visível
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// tanto a physics do player quanto pointer events usam o contorno completo e detalhado da geometria visível
// a geometria invisível mais simples é mapeada para ColliderLayer.CL_NONE para evitar calcular ambas
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
	visibleMeshesCollisionMask:
		ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS,
})

// não responde a colisões de nenhum tipo, com a geometria visível ou a invisível:
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
})
```

## Sintaxe avançada de MeshCollider

A sintaxe completa para criar um `MeshCollider` component, sem nenhum helper para simplificá-la, é a seguinte:

```ts
MeshCollider.create(myBox, {
	mesh: {
		$case: 'box',
		box: {},
	},
})

MeshCollider.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: {},
	},
})

MeshCollider.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshCollider.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

É assim que o protocolo base interpreta components MeshCollider. As helper functions abstraem isso e expõem uma sintaxe mais amigável, mas, nos bastidores, elas geram essa sintaxe.

O `$case` field permite especificar um dos tipos permitidos. Cada tipo suporta um conjunto diferente de parâmetros.

Os valores suportados para `$case` são os seguintes:

* `box`
* `plane`
* `sphere`
* `cylinder`

Dependendo do valor de `$case`, é válido definir o objeto para a shape correspondente, passando quaisquer propriedades relevantes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
