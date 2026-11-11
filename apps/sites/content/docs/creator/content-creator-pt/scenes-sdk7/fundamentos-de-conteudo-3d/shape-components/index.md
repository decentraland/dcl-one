> Saiba mais sobre os diferentes components que dão às entities sua forma 3D e colisão.

# Components de Forma

Cenas tridimensionais em Decentraland são baseadas no [Entity-Component](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) modelo, em que tudo em uma scene é um *entity*, e cada entity pode incluir *components* que moldam suas características e funcionalidade.

A forma renderizada de uma entity é determinada pelo component que ela usa.

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

## Use o Scene Editor no Creator Hub

A maneira mais fácil de dar uma shape a uma entity é usar o Scene Editor. Você pode adicionar um **Mesh Renderer** component para fornecer uma forma primitive, ou um **GLTF** component para referenciar um modelo 3D de um arquivo. Veja [Adicionar Components](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components).

## Formas primitivas

Várias formas básicas, frequentemente chamadas de *primitives*, podem ser adicionadas a uma entity ao dar a ela um `MeshRenderer` componente.

As seguintes formas estão disponíveis. Várias formas incluem campos adicionais opcionais, específicos para essa forma.

* **box**:

  Use `MeshRenderer.setBox()`, passando a entity. Passe `uvs` como um campo adicional opcional, para mapear o alinhamento da texture. Veja [materials](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/) para mais detalhes.
* **plane**:

  Use `MeshRenderer.setPlane()`, passando a entity. Passe `uvs` como um campo adicional opcional, para mapear o alinhamento da texture. Veja [materials](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/) para mais detalhes.
* **sphere**:

  Use `MeshRenderer.setSphere()`, passando a entity.
* **cylinder**:

  Use `MeshRenderer.setCylinder()`, passando a entity. Passe `radiusTop` e `radiusBottom` como campos adicionais opcionais, para modificar o cylinder.

  DICA: Defina qualquer um dos `radiusTop` ou `radiusBottom` como 0 para fazer um cone.

O exemplo a seguir cria um cube:

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)
```

O exemplo a seguir cria um cylinder com um `radiusTop` de 0, o que produz um cone:

```ts
const myCone = engine.addEntity()

Transform.create(myCone, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setCylinder(myCone, 1, 0)
```

As formas primitivas não incluem materials. Para dar cor ou texture a elas, você deve atribuir um [material component](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/) à mesma entity.

Para tornar uma primitive clicável, ou para impedir que os players passem por ela, você deve dar à entity um *collider* por meio de um [MeshCollider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/) componente.

Para mudar a shape de uma entity que já tenha um `MeshRenderer` component, execute `MeshRenderer.setBox()` ou qualquer uma das outras helper functions, e ela substituirá a shape original. Não há necessidade de remover o original `MeshRenderer` nem de usar a sintaxe avançada.

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)

// substituir a shape
MeshRenderer.setSphere(myCube)
```


> [!WARNING]
> **📔 Nota**: O `MeshRenderer` component deve ser importado via
> 
> > `import { MeshRenderer } from "@dcl/sdk/ecs"`
> 
> Veja [Importações](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## 3D models

Para formas mais complexas, você pode criar um modelo 3D em uma ferramenta externa como Blender e depois importá-lo em *.glTF* ou *.glb* (binary *.glTF*). [glTF](https://www.khronos.org/gltf) (GL Transmission Format) é um projeto aberto da Khronos que fornece um formato comum e extensível para assets 3D, eficiente e altamente interoperável com tecnologias web modernas.

Para adicionar um modelo externo em uma scene, adicione um `GltfContainer` component a uma entity e defina seu `src` para o path do arquivo glTF que contém o modelo.

```ts
const houseEntity = engine.addEntity()

GltfContainer.create(houseEntity, {
	src: 'models/House.gltf',
})
```

A `src` campo é obrigatório; você deve atribuir um valor ao construir o component. No exemplo acima, o modelo está localizado em uma `models` folder na raiz da pasta do projeto da scene.


> [!NOTE]
> **💡 Dica**: Recomendamos manter seus models separados na `assets/scene/models` folder dentro da sua scene.


modelos glTF podem incluir suas próprias textures, materials, colliders e animations embutidos. Veja [3D models](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) para mais informações sobre isso. Para sobrescrever os materials de um modelo, use o [GltfNodeModifiers](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) componente. Veja [Modificar materiais glTF](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) para mais detalhes.

Para impedir que os players atravessem um modelo 3D, ou para tornar um modelo clicável, você deve ter um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/), que pode estar embutido no modelo ou ser fornecido via um `MeshCollider` componente.

Lembre-se de que todos os models, seus shaders e suas textures devem estar dentro dos parâmetros do [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/).


> [!WARNING]
> **📔 Nota**: O `GltfContainer` component deve ser importado via
> 
> `import { GltfContainer } from "@dcl/sdk/ecs"`
> 
> Veja [Importações](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


### Pré-carregando um modelo 3D

Em alguns casos, um modelo 3D personalizado é adicionado à scene, mas não é usado imediatamente. Por exemplo, um modelo personalizado de cadeira pode só ser carregado quando o player interage com outro objeto. Nesse cenário, a primeira interação pode levar algum tempo até concluir o download e o carregamento do modelo.

Para evitar isso, use o `AssetLoad.create` método para garantir que o asset seja baixado antes de ser necessário.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

Para mais informações, consulte a [documentação de Pre Load Resources](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/pre-load-resources/) documentação.

### Bibliotecas gratuitas de modelos 3D

Em vez de criar seus próprios modelos 3D, você também pode baixá-los de várias bibliotecas gratuitas ou pagas.

Para começar, abaixo está uma lista de bibliotecas que têm conteúdo gratuito ou relativamente barato:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [Assets from the Builder](https://github.com/decentraland/builder-assets/tree/master/assets)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/) (modelos 3D feitos principalmente para impressão 3D, mas adaptáveis a Virtual Worlds)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://www.cgtrader.com/)


> [!WARNING]
> **📔 Nota**: Preste atenção às restrições de licença que o conteúdo que você baixar possui.


Observe que, em vários desses sites, você pode escolher em que formato baixar o modelo. Sempre escolha *.glTF* esse formato, se disponível. Se não estiver disponível, você deve convertê-los para *glTF* antes de poder usá-los em uma scene. Para isso, recomendamos importá-los para Blender e exportá-los como *.glTF* a partir daí.

### Otimize modelos 3D

Para garantir que os modelos 3D em sua scene carreguem mais rápido e ocupem menos memória, siga estas práticas recomendadas:

* Salve seus models no *.glb* formato, que é uma versão mais leve de *.gltf*.
* Se você tiver vários models que compartilham as mesmas textures, exporte seus models com textures em um arquivo separado. Dessa forma, vários models podem referenciar um único texture file que só precisa ser carregado uma vez.
* Se sua scene tiver entities que aparecem e desaparecem, pode ser uma boa ideia agrupar essas entities e mantê-las subterrâneas, ou em uma escala de 0. Isso ajudará que apareçam mais rápido; a contrapartida é que ocuparão memória quando não estiverem em uso.

## Esticando uma forma

Formas primitivas e modelos 3D têm dimensões padrão que você pode alterar mudando a escala na Transform da entity `Transform` componente.

```ts
const primitiveEntity = engine.addEntity()

MeshRenderer.setBox(primitiveEntity)

Transform.create(primitiveEntity, {
	position: { x: 8, y: 1, z: 8 },
	scale: { x: 4, y: 0.5, z: 4 },
})
```

## Tornar invisível

Você pode tornar uma entity invisível dando a uma entity um `VisibilityComponent`, com sua `visible` propriedade definida como *false*.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(myEntity)

VisibilityComponent.create(myEntity, { visible: false })
```

A `VisibilityComponent` funciona da mesma forma para entities com formas primitivas e com `GLTFContainer` components.

Se uma entity estiver invisível, seu collider pode bloquear o caminho de um player e/ou impedir clicar em entities que estejam atrás dela, dependendo das collision layers atribuídas ao collider.

### Propagar visibilidade

Você pode usar o `propagateToChildren` campo no `VisibilityComponent` para aplicar uma configuração a cada child na árvore de children da entity. Se `propagateToChildren` estiver definido como *true*, essas configurações afetam todos os children em todos os níveis abaixo. Isso pode poupar muito trabalho tedioso de marcar cada child entity como invisível ou visível também.

```ts
// entity pai (explicitamente invisível)
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(parentEntity)
VisibilityComponent.create(parentEntity, { visible: false, propagateToChildren: true })

// child entity (implicitamente invisível por causa do parent)
const child = engine.addEntity()
Transform.create(child, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity
})
MeshRenderer.setBox(child)
```


> [!WARNING]
> **📔 Nota**: Se uma entity tiver seu próprio `VisibilityComponent`, isso substitui qualquer configuração dos parents.
> 
> Se uma entity não tiver seu próprio `VisibilityComponent`, então sua visibilidade é determinada pelo parent mais próximo com um `VisibilityComponent` e `propagateToChildren` definido como *true*.


## Estado de loading

Se um modelo 3D for razoavelmente grande, pode levar um tempo perceptível para ser renderizado; esse tempo pode variar dependendo do hardware do player e de muitos outros fatores. Às vezes, você precisa garantir que um modelo terminou de carregar antes de executar outra ação. Por exemplo, se você quiser teleportar o player para uma plataforma no alto do céu, primeiro precisa garantir que a plataforma esteja totalmente renderizada antes de mover o player para lá; caso contrário, o player pode cair direto através da plataforma.

Para verificar se um modelo 3D terminou de ser renderizado, verifique o `GltfContainerLoadingState` component. Este component é somente leitura e existe em qualquer entity que também tenha um `GltfContainer`componente.

Este component tem uma única propriedade chamada `currentState`, contendo um valor do `LoadingState` enum.

O exemplo a seguir usa um system para verificar periodicamente o loading state de um modelo 3D de uma entity. Se o estado for `LoadingState.FINISHED`, talvez você queira executar uma lógica personalizada ali e encerrar a execução do system.

```ts
export function main() {
	const meshEntity = engine.addEntity()
	GltfContainer.create(meshEntity, { src: 'models/Monster.glb' })
	engine.addSystem((deltaTime) => {
		const loadingState = GltfContainerLoadingState.getOrNull(meshEntity)
		if (!loadingState) return
		switch (loadingState.currentState) {
			case LoadingState.LOADING:
				console.log('mesh está carregando')
				break
			case LoadingState.FINISHED:
				console.log('mesh está finalizado')
				// Executar lógica personalizada
				break
			case LoadingState.FINISHED_WITH_ERROR:
				console.log('mesh terminou, mas pode ter problemas')
				break
			case LoadingState.UNKNOWN:
				console.log('mesh está em um estado desconhecido')
				break
		}
	})
}
```

## Sintaxe avançada

A sintaxe completa para criar um `MeshRenderer` component, sem nenhum helper para simplificá-lo, é assim:

```ts
MeshRenderer.create(myBox, {
	mesh: {
		$case: 'box',
		box: { uvs: [] },
	},
})

MeshRenderer.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: { uvs: [] },
	},
})

MeshRenderer.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshRenderer.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

É assim que o protocolo base interpreta os componentes MeshRenderer. As helper functions abstraem isso e expõem uma sintaxe mais amigável, mas nos bastidores elas produzem essa sintaxe.

A `$case` campo permite que você especifique um dos tipos permitidos. Cada tipo suporta um conjunto diferente de parâmetros. No exemplo acima, o `box` type suporta um `uvs` .

Os valores suportados para `$case` são os seguintes:

* `box`
* `plane`
* `sphere`
* `cylinder`

Dependendo do valor de `$case`, é válido definir o objeto para a forma correspondente, passando quaisquer propriedades relevantes.

Para adicionar um `MeshRenderer` component a uma entity que possivelmente já tenha uma instância desse component, use `MeshRenderer.createOrReplace()`. As helper functions como `MeshRenderer.setBox()` tratam de sobrescrever instâncias existentes do component, mas executar `MeshRenderer.create()` em uma entity que já tenha este component retorna um erro.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
