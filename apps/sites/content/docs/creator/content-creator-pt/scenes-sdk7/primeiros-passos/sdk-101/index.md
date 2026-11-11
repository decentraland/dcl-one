> Crie sua primeira scene do Decentraland com o Creator Hub, o SDK e vibe coding assistido por IA usando o SDK Skills oficial.

# Início Rápido do SDK

Este tutorial guia você na criação da sua primeira scene, combinando as diferentes ferramentas disponíveis: o Scene Editor visual do Creator Hub, código escrito manualmente usando o Decentraland SDK e vibe coding assistido por IA **vibe coding**. Você usará as três juntas e aprenderá quando cada uma se destaca.

## Como crio uma scene no Decentraland?

1. **Instale o** [**Creator Hub**](https://decentraland.org/download/creator-hub), o aplicativo oficial para desktop para criar, pré-visualizar e publicar scenes. É a ferramenta recomendada para creators de todos os níveis de habilidade.
2. **Se você usar um assistente de codificação com IA** (como Claude Code, Cursor ou Copilot), instale o oficial [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para que ele conheça padrões verificados do SDK7: `npx skills add decentraland/sdk-skills`
3. **A** [**CLI**](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/) é uma alternativa para usuários avançados e fluxos de trabalho automatizados.

O restante deste tutorial percorre esse caminho recomendado passo a passo.

## Instale o Creator Hub

O Creator Hub permite que você crie, visualize e publique scenes do Decentraland. Baixe o Creator Hub [aqui](https://decentraland.org/download/creator-hub).

Para editar o código da sua scene, você também precisa de um editor de código. [Visual Studio Code](https://code.visualstudio.com/) e [Cursor](https://www.cursor.com/) são ambas ótimas opções, mas qualquer editor de código funciona.

Leia o [guia de instalação](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/) para mais detalhes.

## Crie sua primeira scene

1. Abra o Creator Hub.
2. Selecione as **Cenas** aba, e clique em **Nova cena**.

   ![](/docs/_remote/efc0e8d68c33532f7a840f5e.webp)
3. Escolha um template inicial. Para este exercício, escolha o **Empty Scene**.

Esta etapa pode levar alguns minutos. Ela preenche sua pasta com o conjunto padrão de arquivos para uma scene básica. Quando isso terminar, você verá a grade vazia da sua scene.

## Adicione itens dos Asset Packs

Explore os **Asset Packs** na seção inferior do Scene Editor e arraste alguns itens para a sua scene. Qualquer item serve por enquanto.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

Itens já posicionados podem ser clicados e arrastados para reposicioná-los. Veja [Essenciais do Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#position-items) para mais detalhes.


> [!NOTE]
> **💡 Dica**: Cubra toda a scene com um item de Ground. Itens do tipo **Ground** têm um ícone de balde de tinta. Se arrastar um deles para a sua cena, ele cobre todo o ground da sua cena com cópias deste item.
> 
> <img src="/docs/_remote/e120bb99677fe9cb301458fb.webp" alt="Ground" data-size="original">


## Execute uma preview

Clique nos **Play** botão no menu superior para carregar sua scene dentro do Decentraland. Agora você pode explorar a scene como um avatar do Decentraland.

![](/docs/_remote/77e30200d3905dd6bed10959.webp)

Você pode manter a janela de preview aberta enquanto trabalha: ela é atualizada toda vez que você faz uma alteração. Leia mais em [pré-visualizar uma scene](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/).

## Assets 3D personalizados

Baixe este modelo 3D de um abacate em *glb* no formato do seguinte [link](https://github.com/decentraland-scenes/avocado/raw/main/avocado-glb.zip) e descompacte-o.

![](/docs/_remote/54ed4744f3c2c873fc597b60.webp)

Arraste o **avocado.glb** arquivo do seu explorador de arquivos para o painel inferior do Scene Editor (o mesmo painel que contém as **Asset Packs** e **Local Assets** abas) e clique em **Importar**.

![](/docs/_remote/63d04fd8ef301b90c747fa37.webp)

Agora você pode encontrar o **avocado.glb** modelo em **Local Assets** aba, dentro da **Cena** pasta. Arraste o arquivo para a sua scene, como qualquer item dos Asset Packs.

![](/docs/_remote/23bd93a0c282b7207519dc58.webp)

## Edite o código da scene

Clique nos **<> Code** botão no menu superior para abrir o projeto da sua scene no seu editor de código.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)


> [!WARNING]
> **📔 Nota**: Se nada abrir, verifique se você tem um editor de código como [Visual Studio Code](https://code.visualstudio.com/) ou [Cursor](https://www.cursor.com/) instalado.


Na margem esquerda do seu editor de código, você pode navegar pelos arquivos e pela estrutura de pastas do seu projeto. Abra o `index.ts` arquivo dentro da `src` pasta. O conteúdo dele deve ficar assim:

```ts
import {} from '@dcl/sdk/math'
import { engine } from '@dcl/sdk/ecs'

export function main() {}
```

As scenes do Decentraland são escritas em [TypeScript](https://www.typescriptlang.org/), usando o Decentraland SDK: uma biblioteca com tudo o que você precisa para posicionar conteúdo 3D, adicionar interatividade e controlar o que acontece na sua scene.

Este arquivo define uma função chamada `main()`, que é o ponto de entrada da scene: qualquer código que você colocar ali é executado quando a scene carrega pela primeira vez.

Você já arrastou um abacate para a scene visualmente. Agora vamos adicionar um segundo, desta vez escrevendo código. Substitua todo o conteúdo do seu `index.ts` arquivo pelo seguinte:

```ts
import { Vector3 } from '@dcl/sdk/math'
import { engine, Transform, GltfContainer } from '@dcl/sdk/ecs'

export function main() {
	// create a fresh new Entity
	let avocado2 = engine.addEntity()

	// give it a Transform
	Transform.create(avocado2, {
		position: Vector3.create(8, 0, 8),
	})

	// give it a GLTF
	GltfContainer.create(avocado2, {
		src: 'assets/scene/avocado.glb',
	})
}
```

Estas linhas criam uma nova [entity](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/), dão a ela um [shape](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/) com base no modelo 3D que você baixou e [definem sua posição](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/) via **Transform** component.

Como regra geral, o código que você escreve em `index.ts` deve estar todo dentro de `main()` (ou em outras funções que são referenciadas indiretamente por main). Veja [Ciclo de vida da scene](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#scene-lifecycle) para mais detalhes.

Execute a prévia da scene: agora você deve ver dois abacates, o que você adicionou no Scene Editor e o que você adicionou via código.

![](/docs/_remote/9e7996b651530bdb72380bb3.webp)


> [!WARNING]
> **📔 Nota**: O segundo abacate existe **somente no seu código**. Ele aparece quando você executa a scene, mas o canvas do Scene Editor e a árvore de entities não conseguem exibir entities criadas em `index.ts`. Não se assuste se não o vir no editor. Isso é algo importante a lembrar quando você mistura edição visual com código.


Ambos os abacates são construídos com as mesmas partes. Selecione o **primeiro** abacate no Scene Editor: o painel de propriedades mostra um **Transform** e um **GltfContainer** component com os mesmos tipos de campos que você acabou de escrever em código. São duas visualizações da mesma coisa.

![](/docs/_remote/90ea2c66a117f593fd4e77cc.webp)

## Adicione interatividade com um Script

Vamos fazer um abacate responder ao player. Poderíamos fazer isso adicionando código no `index.ts` que já estávamos editando, mas em vez disso vamos usar o **Script component**: uma forma de anexar código diretamente a um item no Scene Editor. Isso mantém o comportamento de cada item autônomo, e você pode até reutilizar o mesmo script em vários items.

1. No Scene Editor, selecione o primeiro abacate (o que você arrastou; lembre-se, o abacate apenas em código não está visível aqui).
2. Clique nos **+** botão no topo do painel de propriedades e selecione **Script** para adicionar um Script component.
3. Clique em **+ Create New Script** e dê o nome de `AvocadoScript`.

![](/docs/_remote/73d08b2cfd32963d68a7790e.webp)

4. Clique nos **<> Code** botão no Script component para abrir o novo arquivo no seu editor de código.

O script é uma class com três partes principais (comentários reduzidos por brevidade):

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string, // DO NOT REMOVE
		public entity: Entity, // DO NOT REMOVE
		// Add your custom inputs below
	) {}

	start() {
		// Called once, when the scene loads
	}

	update(dt: number) {
		// Called on every frame
	}
}
```

* A **constructor** define parâmetros que aparecem como campos editáveis no Script component no Creator Hub. Não remova `src` ou `entity`.
* **`start()`** é executado **uma vez**, quando a scene carrega. Use-o para configuração: criar components, registrar manipuladores de clique etc.
* **`update(dt)`** é executado a cada **frame** do jogo, cerca de 30 vezes por segundo. Use-o para comportamento contínuo, como movimento. `dt` diz quantos segundos se passaram desde o último frame.

Dentro da class, `this.entity` sempre se refere à entity que contém o Script component: neste caso, o seu abacate. É isso que torna os scripts reutilizáveis: anexe o mesmo script a dez items, e cada um age sobre si mesmo.

### Programe sua primeira interação com vibe coding

Você pode escrever o seguinte código manualmente, mas este é um ótimo momento para experimentar **vibe coding**: descrever o que você quer a um assistente de IA e deixá-lo escrever o código. A maioria dos editores de código já vem com um integrado, como o chat do Cursor, o GitHub Copilot no VS Code, ou [Claude Code](https://claude.com/product/claude-code). Você também pode escrever prompts fora do seu editor de código, como com Claude Desktop ou Claude Code na linha de comando.

Antes do seu primeiro prompt, instale os SDK skills do Decentraland, para que a IA conheça os padrões do SDK e cometa muito menos erros:

```bash
npx skills add decentraland/sdk-skills
```

Esses skills são documentos de referência que a IA consulta sempre que não tem certeza de como fazer algo com o SDK, então ela escreve código correto em vez de adivinhar. Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para opções de configuração e dicas de prompting.

Agora tente um prompt como este:

> Em AvocadoScript.ts, faça o abacate registrar uma mensagem no console quando o player clicar nele.

Você deve acabar com algo assim (ou cole você mesmo):

```ts
import { engine, Entity, pointerEventsSystem, InputAction } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER },
			},
			() => {
				console.log('CLICKED AVOCADO')
			}
		)
	}

	update(dt: number) {}
}
```

Ele adicionou algum comportamento de clique dentro da `start()` função. Precisamos apenas definir o comportamento de clique uma vez, e ele reagirá sempre que o player clicar no item. A `pointerEventsSystem.onPointerDown()` declaração define três coisas:

* O que `entity` onde os eventos de clique funcionam: aqui `this.entity`, o abacate que contém o script.
* Uma `opts` objeto: qual button usar e outros argumentos opcionais que não estamos usando agora.
* Uma função que é executada toda vez que a entity é clicada.

Para ver a mensagem registrada, execute a preview e abra o console clicando no ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) ícone no canto superior direito. Você também pode alterná-lo pressionando a **\`** tecla. Toda vez que você clicar no abacate, verá uma nova linha aparecer:

![](/docs/_remote/609636cba47e8f304bc52855.webp)


> [!WARNING]
> **📔 Nota**: Para que uma entity possa ser clicada, ela precisa ter uma geometria de collider. O modelo usado aqui já inclui uma. Veja [Colliders](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/) para soluções alternativas para modelos que não têm.


### Faça o abacate desaparecer

Vamos fazer o clique fazer algo mais empolgante agora! Peça ao seu assistente de IA:

> Quando o abacate for clicado, faça-o desaparecer. Quero que ele desapareça com uma animação bouncy de encolhimento. Também quero que a dica que vejo ao apontar para o abacate diga "Collect".

Você deve acabar com algo assim:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

export class AvocadoScript {
	constructor(
		public src: string,
		public entity: Entity,
	) {}

	start() {
		pointerEventsSystem.onPointerDown(
			{
				entity: this.entity,
				opts: { button: InputAction.IA_POINTER, hoverText: 'Collect' },
			},
			() => {
				this.collect()
			}
		)
	}

	collect() {
		Tween.setScale(
			this.entity,
			Vector3.One(),
			Vector3.Zero(),
			500,
			EasingFunction.EF_EASEINBOUNCE
		)
	}

	update(dt: number) {}
}
```

Vamos decompor o que a IA fez.

* Para criar a animação bouncy, ela usou um **Tween**. Um tween descreve uma transição gradual da posição, rotação ou escala de uma entity ao longo do tempo. Aqui `Tween.setScale()` encolhe o abacate do tamanho total (`Vector3.One()`) até nada (`Vector3.Zero()`) ao longo de 500 milissegundos, usando uma curva de easing bouncy. Saiba mais sobre tweens em [mover entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/).
* Em vez de escrever o tween diretamente dentro da função de clique, ela criou um `collect()` separado. Isso não é obrigatório (se sua IA colocou o tween dentro da função de clique, também funciona), mas é uma boa prática: métodos mantêm o código legível e permitem reutilizar a mesma lógica em lugares diferentes.
* Para mostrar a dica "Collect" no abacate, ela definiu isso como o `hoverText` valor.

Execute a prévia e clique no abacate: ele deve desaparecer com estilo.


> [!WARNING]
> **📔 Nota**: O abacate encolhe até um tamanho 0, mas a entity ainda existe. Idealmente, você deve deletar a entity depois que o tween terminar, para manter sua scene mais leve. Veja [Quando o tween terminar](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/#on-tween-finished).


### Execute código a cada frame

Até agora, todo o nosso código foi executado em `start()`. Vamos usar `update()` para fazer o abacate girar continuamente. Substitua o constructor e `update()` por:

```ts
	constructor(
		public src: string,
		public entity: Entity,
		public speed: number = 45,
	) {}

	update(dt: number) {
		const transform = Transform.getMutable(this.entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(this.speed * dt, Vector3.Up())
		)
	}
```

Você também precisará adicionar `Transform` e `Quaternion` aos imports no topo do arquivo, para que fiquem assim:

```ts
import { engine, Entity, pointerEventsSystem, InputAction, Tween, EasingFunction, Transform } from '@dcl/sdk/ecs'
import { Vector3, Quaternion } from '@dcl/sdk/math'
```


> [!NOTE]
> **💡 Dica**: Em vez de editar imports manualmente, você pode clicar nos erros marcados pelo seu editor de código e deixá-lo adicionar automaticamente.


A cada frame, isso gira o abacate um pouco mais. Multiplicar por `dt` torna o movimento suave e independente da taxa de frames: o abacate gira a 45 graus por segundo, não importa quão rápido a máquina do player funcione.

Como `speed` sendo um parâmetro do constructor, ele também aparece como um campo no Script component no Creator Hub. Clique no ícone de atualização no canto superior direito do Script component para vê-lo e, depois, ajuste o valor sem tocar em nenhum código.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="Botão de atualização" width="360">

Cada entity armazena seu próprio valor para o campo **Speed** , então vários items podem compartilhar o mesmo script, mas se comportar de maneira diferente. Experimente: copie o abacate com Ctrl + C e Ctrl + V, mova a cópia para que os dois não se sobreponham e defina uma speed diferente para cada um. Agora cada abacate gira à sua própria taxa.

Scripts podem fazer muito mais, como expor ações que outros smart items podem disparar. Veja [Script component](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/) para o quadro completo.

## Referencie um item do Scene Editor

O Script component é a forma mais fácil de dar comportamento a um único item, mas há uma alternativa: seu código em `index.ts` pode buscar qualquer item que você adicionou visualmente, pelo nome, usando `engine.getEntityOrNullByName()`. Isso é útil quando uma lógica envolve vários items, ou quando você quer tudo em um só lugar. Use o nome que aparece na [entity tree](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#the-entity-tree).

Neste exemplo, usamos uma entity chamada **Yellow Crate**. Você pode usar qualquer item, basta escrever o nome exatamente como ele aparece na entity tree.


> [!NOTE]
> **💡 Dica**: Você pode renomear entities clicando com o botão direito e selecionando **Rename** na entity tree.


```ts
export function main() {
	// avocado2 code from before
	// (...)

	const crate = engine.getEntityOrNullByName('Yellow Crate')

	if (crate) {
		pointerEventsSystem.onPointerDown(
			{
				entity: crate,
				opts: { button: InputAction.IA_POINTER, hoverText: 'Open' },
			},
			function () {
				console.log('CLICKED CRATE')
			}
		)
	}
}
```

Aqui `engine.getEntityOrNullByName()` busca uma referência à entity chamada *Yellow Crate*. A `if (crate)` a verificação garante que a entity realmente exista na scene; se não houver uma entity com esse nome, `crate` é `null`. Veja [Itens de referência](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/reference-items/) para mais informações.


> [!NOTE]
> **💡 Dica**: Todas as entities adicionadas via Scene Editor já estão carregadas quando `main()` é executado, então é seguro referenciá-las ali ou em funções chamadas indiretamente por `main()`.


## Mais Tutoriais

Leia [Programando scenes](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/) para uma compreensão de alto nível de como as scenes do Decentraland funcionam.

Para exemplos criados com SDK7, confira a [página de exemplos](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/), que contém scenes e repositórios reutilizáveis.

Veja a **guia de desenvolvimento** seção para mais instruções sobre adicionar conteúdo à sua scene.

## Interaja com outros desenvolvedores

Visite o [Discord da Decentraland](https://dcl.gg/discord) e as [Discord da Decentraland DAO](https://discord.gg/bxHtcMxUs4) para participar de uma discussão animada sobre o que é possível e como no **Creators** .

Para depurar quaisquer problemas, verifique as [Resolução de problemas](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/troubleshooting/) e [debug](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview/) seções. Um assistente de IA com os [SDK skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) instalados também é um ótimo companheiro de depuração: cole a mensagem de erro do console, ou descreva o que não está se comportando como esperado, e ele geralmente consegue encontrar o problema no seu código. Se você não encontrar uma solução, pode publicar na [categoria SDK Support](https://forum.decentraland.org/c/support-sdk/11) no Fórum da Decentraland.

## Assets de arte 3D

Uma boa experiência terá uma ótima arte 3D para acompanhá-la. Se você estiver animado para criar esses modelos 3D por conta própria, veja a [seção de Modelagem 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/). Mas se você preferir focar no lado de programação ou game design, não precisa criar seus próprios assets! Veja [Recursos Úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) para bibliotecas de assets e ferramentas de IA que você pode usar para obter modelos 3D para sua scene.

## Publique sua cena

Se você possui um Decentraland NAME, um nome ENS de ETH ou LAND, ou tem permissões concedidas por alguém que tenha, você pode enviar sua scene para a Decentraland. Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/).

## Outras informações úteis

* [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/)
* [Fluxo de trabalho de desenvolvimento](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/dev-workflow/)
* [Editando Scenes](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/)
* [Essenciais do Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/)
* [Restrições de design para games](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games/)
* [modelagem 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/)
* [Limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
