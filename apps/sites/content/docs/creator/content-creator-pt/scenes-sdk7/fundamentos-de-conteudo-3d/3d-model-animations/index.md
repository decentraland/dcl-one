> Como animar modelos 3D na sua scene

# Animações de Modelos 3D

Modelos 3D em *.glTF* e *.glb* O formato pode incluir tantas animações quanto quiseres. As animações dizem à mesh como se mover, ao especificar uma série de *keyframes* que são distribuídas ao longo do tempo, e depois a mesh faz a transição de uma pose para a outra para simular movimento contínuo.

A maioria das animações de modelos 3D são [*animações esqueléticas*](https://en.wikipedia.org/wiki/Skeletal_animation). Estas animações simplificam a geometria complexa do modelo numa "figura de pauzinho", ligando cada vértice da mesh ao *bone* em *esqueleto*. Os modeladores ajustam o esqueleto para diferentes poses, e a mesh estica-se e dobra-se para acompanhar estes movimentos.

Como alternativa, *animações de vértices* animam um modelo sem necessidade de um esqueleto. Estas animações especificam diretamente a posição de cada vértice no modelo. Decentraland também suporta estas animações.

Veja [Animações](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/animations.md) para detalhes sobre como criar animações para um modelo 3D. Consulta [componentes Shape](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/) para instruções sobre como importar um modelo 3D para uma cena.


> [!NOTE]
> **💡 Dica**: As animações costumam ser melhores para mover algo no lugar, e não para alterar a posição de uma entity. Por exemplo, podes definir uma animação para mover os pés de uma personagem no lugar, mas, para alterar a localização da entity, o melhor é usar o componente Transform. Consulta [Posicionamento de entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/) para mais detalhes.


## Verificar um modelo 3D quanto a animações

Nem todos os *glTF* ficheiros incluem animações. Para ver se há alguma disponível, podes fazer o seguinte:

* Se estiveres a usar [VS Code](https://code.visualstudio.com/)(recomendado), instala a *extensão GLTF Tools* e vê aí o conteúdo de um ficheiro glTF.
* Abra o [Babylon Sandbox](https://sandbox.babylonjs.com/) site e arrasta o ficheiro glTF (e quaisquer *.jpg* ou *.bin* dependências) para o navegador.
* Abra o *.glTF* ficheiro com um editor de texto e desce até encontrares *"animations":*.


> [!NOTE]
> **💡 Dica**: Nas *esqueléticas* animações, o nome de uma animação é muitas vezes composto pelo nome da sua armature, um sublinhado e o nome da animação. Por exemplo `myArmature_animation1`.


## Reprodução automática

Se um modelo 3D incluir animações, o comportamento predefinido é que a primeira delas seja sempre reproduzida em loop.

Para evitar este comportamento, adiciona um `Animator` componente Animator à entity que tem o modelo e, depois, trata da reprodução das animações explicitamente. Se um `Animator` componente Animator estiver presente na entity, todas as animações passam por predefinição para um estado `playing: false` e precisam de ser reproduzidas manualmente.


> [!NOTE]
> **💡 Dica**: No [Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), podes adicionar um **Animator** componente visualmente. Consulta [Add Components](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components). Também podes controlar animações sem código através de **Actions**, veja [Tornar qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/).


## Gerir animações explicitamente

Um `Animator` O componente Animator é usado para aceder a todas as animações da entity e pode ser usado para indicar explicitamente à entity que reproduza ou pare uma animação. O `Animator` componente inclui um array de `states`, esta lista deve incluir um objeto para cada uma das animações que o modelo 3D pode realizar. Um único `Animator` pode incluir tantos states quantos forem necessários.

```ts
// Criar entity
const shark = engine.addEntity()

// Adiciona um modelo 3D a ele
GltfContainer.create(shark, {
	src: 'models/shark.glb',
})

Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
		},
	],
})
```

Cada `state` O objeto mantém o registo de se uma animação está atualmente em reprodução.


> [!WARNING]
> **📔 Nota**: A `Animator` component deve ser importado via
> 
> > `import { Animator } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Obter uma animação

Obter um clip do `Animator` pelo nome usando a `.Animator.getClip()` função. Esta função devolve uma versão mutável do objeto do estado da animação.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
```

`Animator.getClip` requer os seguintes parâmetros:

* `entity`: A entity do `Animator` componente que queres consultar.
* `clipName`: String para o nome do clip que queres obter.

`Animator.getClip` obtém uma versão mutável do estado da animação, para que possas modificar livremente os valores devolvidos por esta função.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')
swimAnim.loop = false
```


> [!WARNING]
> **📔 Nota**: Se tentares usar `Animator.getClip()` para obter um clip que não esteja listado no `Animator` componente, é lançado um erro. Usa `Animator.getClipOrNull()` se preferires obter uma `null` resposta nesse caso, em vez de um erro.


## Reproduzir uma animação

O `.playing` campo num estado de animação determina se a animação está atualmente a ser reproduzida. Nota que várias animações podem estar a ser reproduzidas num único modelo 3D ao mesmo tempo.

Use o seletor `Animator.playSingleAnimation()` função num `AnimationState` objeto.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim')
```

Se a entity estiver a reproduzir quaisquer outras animações, `Animator.playSingleAnimation` interrompe-as.

`Animator.playSingleAnimation` requer os seguintes parâmetros:

* `entity`: A entity do `Animator` componente que queres afetar.
* `clipName`: String para o nome do clip que queres reproduzir.
* `resetCursor`: *(opcional)* Se *true*, reproduz a animação desde o início, mesmo que a animação tenha estado em pausa anteriormente. Se *false*, vai continuar a reproduzir a animação a partir do ponto em que foi pausada. Predefinição: *true*.

```ts
Animator.playSingleAnimation(sharkEntity, 'swim', false)
```

A tabela seguinte resume como `Animator.playSingleAnimation()` se comporta, usando diferentes valores para a `resetCursor` propriedade:

|                              | `resetCursor` = *false*             | `resetCursor` = *true* (predefinição) |
| ---------------------------- | ----------------------------------- | ------------------------------------- |
| **Atualmente em reprodução** | Não tem efeito.                     | Reproduz desde o início.              |
| **Em pausa**                 | Retoma do último frame reproduzido. | Reproduz desde o início.              |
| **Concluída (sem loop)**     | Reproduz desde o início.            | Reproduz desde o início.              |

## Animações em loop

Por predefinição, as animações são reproduzidas em loop, repetindo a animação para sempre.

Altera esta definição ao definir a `loop` propriedade no `state` objeto.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			loop: false,
		},
	],
})
```

Se `loop` está definido como *false*, a animação é reproduzida apenas uma vez e depois pára, ficando na pose do último frame.

## Parar uma animação

Para parar todas as animações que uma entity está a reproduzir, usa `Animator.stopAllAnimations()`.

```ts
Animator.stopAllAnimations(shark)
```

`Animator.stopAllAnimations` requer os seguintes parâmetros:

* `entity`: A entity do `Animator` componente que queres afetar.
* `resetCursor`: *(opcional)* Se *true*, regressa à pose do primeiro frame da animação. Se *false*, fica em pausa na sua pose atual. Predefinição: *true*.


> [!WARNING]
> **📔 Nota**: Ao reproduzir uma animação com `Animator.playSingleAnimation`, esta função trata de parar todas as outras animações nos bastidores. Nesse caso, não precisas de parar explicitamente as outras animações.


Quando uma animação termina de reproduzir uma animação sem loop, por predefinição o modelo 3D permanece na última pose que tinha. O `shouldReset` propriedade controla o que acontece quando uma animação parada é reproduzida novamente: se *true*, a animação é reposta ao seu estado inicial (o primeiro frame, ou o último frame se for reproduzida com uma `speed`) sempre que muda de parada para reprodução. Se *false* (o padrão), retoma do ponto em que estava.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'bite',
			playing: true,
			shouldReset: true,
			loop: true,
		},
	],
})
```

Também podes usar `Animator.stopAllAnimations()` a qualquer momento para definir explicitamente a pose de volta para o primeiro frame da animação.


> [!WARNING]
> **📔 Nota**: Repor a pose é uma mudança abrupta. Se quiseres que o modelo faça uma transição suave para outra pose, reproduz a outra animação e faz o blend entre as duas ao deslocar gradualmente os seus `weight` propriedades. Consulta [Peso da animação](#animation-weight).


## Detetar quando uma animação termina

Quando uma animação sem loop termina de ser reproduzida, o engine define a `playing` propriedade de volta para *false*. O código da tua scene pode ler este valor para saber quando a animação terminou, por exemplo para encadear outra animação logo a seguir.

```ts
let wasPlaying = false

engine.addSystem(() => {
	const animator = Animator.get(shark)
	const biteState = animator.states.find((state) => state.clip === 'bite')
	const isPlaying = biteState?.playing ?? false

	if (wasPlaying && !isPlaying) {
		console.log('bite animation finished')
		// encadear a animação seguinte
		Animator.playSingleAnimation(shark, 'swim')
	}

	wasPlaying = isPlaying
})
```


> [!WARNING]
> **📔 Nota**: Ao verificar o estado da animação em cada frame, lê-o sempre através de `Animator.get()` (só leitura). Não uses `Animator.getClip()` ou `Animator.getMutable()` para a verificação: estas devolvem uma versão mutável do componente, o que o marca como alterado em cada frame e causa trabalho desnecessário de sincronização.
> 
> O `playing` A propriedade só é alterada pelo engine quando a animação termina por si própria. As animações em loop reproduzem-se até serem paradas, por isso nunca alteram a propriedade por si próprias, e as animações com `speed` speed definido para 0 nunca terminam.



> [!WARNING]
> **📔 Nota**: Esta funcionalidade só é suportada no cliente Desktop.


## Gerir várias animações

Se um modelo 3D tiver várias animações incluídas nele, um único `Animator` componente pode lidar com todas elas.

```ts
// Criar entity
const shark = engine.addEntity()

// Adiciona um modelo 3D a ele
GltfContainer.create(shark, {
	src: 'models/shark.glb'
})

Animator.create(shark, {
	states:[{
			clip: "swim",
			playing: true,
			loop: true
		}, {
			clip: "bite",
			playing: true,
			loop: true
		}
	]
})
```

No exemplo acima, duas animações são tratadas por objetos separados e, depois, são ambas atribuídas ao mesmo `state` objectos, e depois são ambas atribuídas ao mesmo `Animator` component.

Cada bone numa animação só pode ser afetado por uma animação de cada vez, a menos que estas animações tenham um `weight` que some para um valor de 1 ou menos.

Se uma animação apenas afetar as pernas de uma personagem e outra apenas afetar a cabeça de uma personagem, então podem ser reproduzidas ao mesmo tempo sem qualquer problema. Mas se ambas afetarem as pernas da personagem, então tens de reproduzir apenas uma de cada vez, ou reproduzi-las com pesos mais baixos `weight` valores.

Se, no exemplo acima, a `bite` animação afeta apenas a boca do tubarão, e a `swim` animação afeta apenas os bones da espinha dorsal do tubarão, então ambas podem ser reproduzidas ao mesmo tempo.


> [!WARNING]
> **📔 Nota**: `Animator.playSingleAnimation()` interrompe todas as outras animações que a entity está atualmente a reproduzir. Para reproduzir várias animações ao mesmo tempo, modifica a `playing` propriedade nos states da animação manualmente.


## Velocidade da animação

Altera a velocidade a que uma animação é reproduzida ao alterar a `speed` propriedade. O valor da speed é 1 por predefinição.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			speed: 2,
		},
	],
})
```

Define a speed abaixo de 1 para a reproduzir mais devagar, por exemplo 0.5 para a reproduzir a metade da speed. Define-a acima de 1 para a reproduzir mais depressa, por exemplo 2 para a reproduzir ao dobro da speed.

```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.speed = 0.5
```

## Peso da animação

O `weight` propriedade permite que um único modelo realize várias animações ao mesmo tempo, calculando uma média ponderada de todos os movimentos envolvidos na animação. O valor de `weight` determina quanta importância essa animação terá na média.

Por predefinição, `weight` é igual a *1*. `weight` não pode ser superior a *1*.

```ts
Animator.create(shark, {
	states: [
		{
			clip: 'swim',
			playing: true,
			loop: true,
			weight: 0.2,
		},
	],
})
```

O `weight` os valores de todas as animações ativas numa entity devem somar 1 em todos os momentos. Se somar menos de 1, a média ponderada vai usar a posição predefinida da armature para a parte restante do cálculo.

Por exemplo, no exemplo de código acima, estamos a reproduzir a *swim* animação, que só tem um `weight` de *0.2*.

O `weight` propriedade pode ser usada de formas interessantes, por exemplo a `weight` propriedade de *swim* podia ser definida em proporção à velocidade a que o tubarão nada, para que não precises de criar várias animações para natação rápida e lenta.

Também poderias alterar o `weight` valor gradualmente ao iniciar e parar uma animação, para lhe dar uma transição mais natural e evitar saltos da pose predefinida para a primeira pose na animação.


> [!WARNING]
> **📔 Nota**: O valor adicionado `weight` de todas as animações que atuam sobre um bone de um modelo 3D não pode ser superior a 1. Se mais do que uma animação estiver a afetar os mesmos bones ao mesmo tempo, é preciso que o peso delas esteja definido para valores que, somados, deem menos de 1.


```ts
const swimAnim = Animator.getClip(sharkEntity, 'swim')

swimAnim.weight = 0.5
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/3d-model-animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
