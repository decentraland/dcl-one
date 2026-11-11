> Pré-carregar recursos permite que os assets sejam baixados no início da scene para que estejam prontos quando o jogador interagir com eles pela primeira vez.

# Pré-carregar Recursos

Em alguns casos, um asset é adicionado à scene, mas não é usado imediatamente. Por exemplo, um arquivo de som pode só ser reproduzido quando o jogador pressiona um botão. Nesse cenário, na primeira vez que o jogador pressiona o botão, o áudio pode tocar alguns segundos atrasado, porque o arquivo só é baixado quando necessário.

Para evitar esse problema, use o `AssetLoad` componente para garantir que esses assets sejam baixados e estejam prontos para uso antes de serem necessários.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

Os assets listados no `AssetLoad` componente são baixados e adicionados à memória, garantindo que estejam instantaneamente disponíveis quando a scene precisar carregá-los.

Algumas considerações importantes:

* Você pode colocar o `AssetLoad` componente em qualquer entity (não apenas no RootEntity) e pode usá-lo em quantas entities forem necessárias. Isso pode ser útil para lidar com estados de carregamento separados para diferentes níveis ou regiões da sua scene.
* **Mantenha cada `AssetLoad` entity com cerca de 40 paths.** Divida uma lista mais longa entre várias entities. Veja [Dividir listas longas entre entities](#split-long-lists-across-entities).
* O `AssetLoad` componente é usado para adicionar assets à memória, não para removê-los. Remover um asset da lista em `AssetLoad.create` não liberará memória.
* Se um asset é usado imediatamente quando a scene carrega (por exemplo, um modelo GLB colocado na scene ou um som de fundo que toca continuamente), não há necessidade de usar o `AssetLoad` componente, já que ele já está sendo baixado.
* Tenha cuidado ao adicionar assets a `AssetLoad.create`e faça pré-carregamento apenas dos assets que não são necessários na inicialização da scene para evitar custos desnecessários de desempenho.
* Você só pode pré-carregar assets que foram enviados como parte dos arquivos da scene. Esse recurso não funcionará para pré-carregar imagens de uma URL externa

## Reagindo ao estado de carregamento

Pré-carregar assets é mais útil se a sua scene puder reagir a quando cada asset terminar de ser baixado. Por exemplo, talvez você queira manter uma tela de carregamento até que todos os assets estejam prontos, mostrar o progresso ao jogador ou lidar de forma elegante com um asset que falhou ao ser baixado.

Para acompanhar isso, use o `assetLoadLoadingStateSystem`. Chame o método `registerAssetLoadLoadingStateEntity` , passando a entity que contém o `AssetLoad` componente e uma função de callback. O callback é executado toda vez que o estado de carregamento de um dos assets dessa entity muda.

```ts
import {
  AssetLoad,
  LoadingState,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"

const preloader = engine.addEntity()
AssetLoad.create(preloader, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    console.log(
      `Asset ${assetLoadState.asset} is now: ${assetLoadState.currentState}`
    )
  }
)
```

O callback recebe um objeto com as seguintes propriedades:

* `asset`: O path do asset cujo estado mudou. Isso corresponde a uma das strings que você listou no `AssetLoad` componente.
* `currentState`: Um valor do `LoadingState` enum, que descreve o novo estado desse asset.

O `LoadingState` enum pode conter os seguintes valores:

* `LoadingState.LOADING`: O asset está sendo baixado no momento.
* `LoadingState.FINISHED`: O asset terminou de ser baixado com sucesso e está pronto para uso.
* `LoadingState.FINISHED_WITH_ERROR`: O asset foi encontrado, mas ocorreu um erro ao baixá-lo.
* `LoadingState.NOT_FOUND`: Nenhum asset foi encontrado no path fornecido.
* `LoadingState.UNKNOWN`: O estado do asset não é conhecido.

O exemplo a seguir usa o estado de carregamento para mudar a cor de uma entity dependendo de como o pré-carregamento está progredindo:

```ts
import {
  AssetLoad,
  LoadingState,
  Material,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"
import { Color4 } from "@dcl/sdk/math"

function getLoadingColor(state: LoadingState): Color4 {
  switch (state) {
    case LoadingState.FINISHED:
      return Color4.Green()
    case LoadingState.LOADING:
      return Color4.Yellow()
    case LoadingState.FINISHED_WITH_ERROR:
    case LoadingState.NOT_FOUND:
      return Color4.Red()
    default:
      return Color4.Gray()
  }
}

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    Material.setPbrMaterial(myEntity, {
      albedoColor: getLoadingColor(assetLoadState.currentState),
    })
  }
)
```


> [!NOTE]
> **💡 Dica**: Você pode adicionar mais assets a um `AssetLoad` componente existente após criá-lo, por exemplo em resposta a uma ação do jogador. Use `AssetLoad.getOrCreateMutable()` para buscar o componente e adicionar novos paths ao seu array de `assets` . O callback registrado também será acionado para esses assets recém-adicionados à medida que forem sendo baixados.


### Dividir listas longas entre entities

Dê a cada `AssetLoad` entity **no máximo 40 paths**. Depois de cerca de 50 assets em uma entity, o callback para de ser acionado e os assets restantes nunca são reportados, sem erro.

A razão é um limite rígido de como os estados de carregamento são armazenados. O client adiciona uma entrada por mudança de estado por asset, normalmente `LOADING` e depois `FINISHED`, em um conjunto que comporta 100 entradas por entity. Quando esse conjunto fica cheio, ele permanece em 100, e o SDK só chama o seu callback quando o número de entradas muda, então mais nada passa.

Para pré-carregar mais de 40 assets, divida a lista em lotes e dê a cada lote a sua própria entity e o seu próprio callback:

```ts
import { AssetLoad, assetLoadLoadingStateSystem, engine } from '@dcl/sdk/ecs'

const allAssets = [
	'assets/models/tree.glb',
	'assets/models/rock.glb',
	// ...uma longa lista
]

// Uma entity AssetLoad por lote de 40 paths
for (let i = 0; i < allAssets.length; i += 40) {
	const batchEntity = engine.addEntity()
	AssetLoad.create(batchEntity, { assets: allAssets.slice(i, i + 40) })

	assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(batchEntity, (assetLoadState) => {
		console.log(assetLoadState.asset, assetLoadState.currentState)
	})
}
```


> [!NOTE]
> **💡 Dica**: Se a sua scene esperar por cada asset antes de iniciar, coloque um timeout nessa espera e registre os paths dos quais você nunca recebeu resposta. Uma lista nomeada é muito mais fácil de depurar do que uma tela de carregamento que nunca desaparece.



> [!NOTE]
> **💡 Dica**: Para um exemplo funcional deste componente, veja a [`88,-12-asset-load`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) scene de teste, que pré-carrega um mp3, uma texture, um video e um glb por meio de um único `AssetLoad` componente e informa o estado de cada asset por meio de `assetLoadLoadingStateSystem` — incluindo um path propositalmente ausente que resulta em `NOT_FOUND`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/otimizacao/pre-load-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
