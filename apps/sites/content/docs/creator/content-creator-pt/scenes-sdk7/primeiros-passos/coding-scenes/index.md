> Este conjunto vai ajudar você a entender como as coisas funcionam no client e no SDK do Decentraland.

# Essenciais de codificação

## As ferramentas de desenvolvimento

Em um nível muito alto, a Decentraland **Kit de Desenvolvimento de Software** (SDK) permite fazer o seguinte:

* Gerar um *projeto* contendo uma scene da Decentraland, incluindo todos os assets necessários para renderizar e executar seu conteúdo.
* Construir, testar e pré-visualizar o conteúdo da sua scene localmente no navegador web - completamente offline, e sem precisar fazer nenhuma transação de Ethereum ou possuir LAND.
* Escrever código TypeScript usando a API da Decentraland para adicionar comportamento interativo e dinâmico à scene.
* Fazer upload do conteúdo da sua scene para o content server.
* Vincular seus tokens LAND à URL do conteúdo que você fez upload.

Nosso SDK inclui o seguinte:

* **O Creator Hub**: Um aplicativo independente que, entre outras coisas, permite criar scenes com uma interface fácil de arrastar e soltar. Você pode executar previews, depurar, editar código e publicar. [Leia mais](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/)
* **O Decentraland ECS**: Um pacote TypeScript contendo o framework de métodos auxiliares que permite criar experiências interativas. Use-o para criar e manipular objetos na scene e também para facilitar transações dentro do mundo entre players ou outros aplicativos. ( [referência mais recente do ECS](https://github.com/decentraland/ecs-reference/blob/master/docs-latest/decentraland-ecs.md))
* **Exemplos de cenas**: Inspire-se e siga as melhores práticas de código dos [exemplos de scene](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/).

Outras ferramentas legadas:

* **O Web Editor**: Uma ferramenta baseada na web para criar scenes simples e publicá-las.

## Requisitos

Para desenvolver uma scene localmente, você não precisa possuir tokens LAND. Desenvolver e testar uma scene pode ser feito completamente offline, sem a necessidade de fazer deploy de uma scene na rede Ethereum (o sistema que a Decentraland usa para estabelecer a propriedade de LAND, de um Decentraland Name) ou no content server.

Você deve ter:

* **O Creator Hub**: Um aplicativo independente que, entre outras coisas, permite criar scenes com uma interface fácil de arrastar e soltar. Você pode executar previews, depurar, editar código e publicar. [Leia mais](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/).

Se você planeja editar o código da scene, também precisará instalar um dos seguintes:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> **Visual Studio Code**: Baixe-o [aqui](https://code.visualstudio.com/). Ele ajuda você a escrever código muito mais rápido e com menos erros. Um editor de código-fonte marca erros de sintaxe, faz autocompletar enquanto você escreve e até mostra sugestões inteligentes que dependem do contexto em que você está. Você também pode clicar em um objeto no código para ver a definição completa da sua class e quais atributos ela suporta.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> **Cursor AI**: Baixe-o [aqui](https://www.cursor.com/). Um editor de código poderoso que é integrado com IA. Ele permite escolher diferentes modelos de IA para ajudar você a escrever código, todos gratuitos. O assistente de IA não apenas faz autocompletar enquanto você escreve; você também pode pedir para ele refatorar uma grande base de código, escrever documentação e muito mais.


> [!NOTE]
> **💡 Dica**: Você pode usar assistentes de IA como Cursor, OpenDCL ou Claude Code para construir scenes inteiras a partir de descrições em linguagem natural — sem experiência em TypeScript necessária. Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para começar.


## Idiomas e sintaxes suportados

A Decentraland utiliza [TypeScript (.ts)](https://www.typescriptlang.org/docs/handbook/jsx.html) como a linguagem padrão para escrever scenes.

TypeScript é um superconjunto de JavaScript, então se você está familiarizado com JavaScript, verá que é quase a mesma coisa, mas TypeScript inclui declarações de tipo. Graças às declarações de tipo, é possível ter recursos como autocompletar e melhores dicas de depuração; isso acelera o desenvolvimento e permite a criação de uma base de código mais sólida. Esses recursos são componentes-chave de uma boa experiência de desenvolvimento.

Quando uma scene é construída, o código TypeScript que você escreveu é compilado em JavaScript minificado, para deixá-lo mais leve. O código-fonte original em TypeScript nunca é enviado para os servidores, apenas a versão compilada em JavaScript.

### Outras linguagens

Você pode usar outra ferramenta ou linguagem em vez de TypeScript e compilá-la para JavaScript, desde que seus scripts compilados estejam contidos em um único arquivo JavaScript que corresponda ao path definido no `main` field da sua scene `scene.json` file (por padrão *bin/index.js*). Todas as declarações de tipo fornecidas são feitas em TypeScript, e outras linguagens e transpilers não têm suporte oficial.

## Cenas

O conteúdo que você faz deploy para o seu LAND é chamado de um **cena**. Uma scene é um programa interativo que renderiza conteúdo 3D; isso pode ser um jogo, uma experiência interativa, uma galeria de arte, o que você quiser!

Scenes são feitas deploy em LAND virtual na Decentraland. LAND é um asset escasso e não fungível mantido em um smart contract de Ethereum. Faça deploy para um único **parcel**, um lote de LAND de 16 metros por 16 metros, ou para múltiplos parcels adjacentes.

Quando os players visitam a Decentraland, eles fazem download e renderizam o conteúdo de cada scene à medida que caminham pelo mapa. Eles descarregam as scenes à medida que se afastam delas.

Você também pode executar uma scene localmente na sua máquina executando uma preview pela CLI.

## Entidades e Componentes

Scenes tridimensionais na Decentraland são baseadas em uma [Entity-Component-System](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) arquitetura, em que tudo em uma scene é uma *entity*Entity *components*. Entities têm

![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

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

cada component dá à entity a que pertence propriedades específicas. Uma entity de porta provavelmente terá pelo menos um component Transform (que define posição, rotação e escala) e outro para lhe fornecer uma forma. Components são apenas um lugar para armazenar dados; eles não executam nenhuma ação por conta própria.

![](/docs/_remote/28a71287061591ece4e52409.webp)

Entities podem estar aninhadas dentro de outras entities para formar uma estrutura em árvore. Se você está familiarizado com desenvolvimento web, talvez ache útil pensar em entities como elementos em uma árvore DOM e em components como os atributos de cada um desses elementos.

Veja [Entities são um conceito abstrato. Uma entity é apenas um id, usado como referência para agrupar diferentes components.](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/) Entities and components

### Components personalizados

para uma visão aprofundada desses dois conceitos e de como eles são usados pelas scenes da Decentraland. `Transform`, `GltfContainer`, `Material`O conjunto padrão de components (como

Você também pode definir *components customizados* components personalizados para armazenar dados que possam ser úteis para a mecânica da sua scene. O engine não saberá como interpretar o que os valores desses components significam, e eles não terão consequências diretas em como a scene é renderizada. No entanto, você pode escrever lógica no código da sua scene para monitorar esses valores e responder a eles. Por exemplo, você pode definir um component personalizado "doorState" para acompanhar o estado aberto/fechado da porta. Nesse caso, o component é apenas um lugar para armazenar um valor que acompanha esse estado. Para ver a porta abrir e fechar na sua scene, você precisa então implementar separadamente a lógica que usa esses valores para afetar a rotação da porta, um valor do `Transform` component que o engine sabe interpretar.

Veja [Componentes Personalizados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) para mais informações.

### Buscar entities pelo nome

As entities adicionadas ao arrastar e soltar no Scene Editor no Creator Hub também podem ser acessadas via código para editá-las ainda mais e adicionar comportamento.

Usa `engine.getEntityOrNullByName()` para buscar uma entity, passando o nome atribuído à entity na UI do Scene Editor. Cada uma deve ter um nome único.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```

Então você pode fazer qualquer coisa com essa entity, como adicionar novos components, modificar seus components existentes, duplicá-la ou excluí-la.

Veja [Obter entity pelo nome](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#get-an-entity-by-name) para mais informações.

Se a entity for um [Smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/), você também pode chamar seu **Actions** ou assinar seus **Triggers** via código. Veja [Itens de referência](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/reference-items/).

## Systems

Entities and components são lugares para armazenar informações sobre os objects em uma scene. *Systems* contêm functions que alteram as informações armazenadas em components ao longo do tempo.

Systems são onde implementamos a lógica do jogo; elas executam as actions que precisam ser atualizadas ou verificadas periodicamente a cada tick do game loop.

Uma system é uma function pura e simples que é chamada uma vez a cada tick (até 30 vezes por segundo), seguindo o [*padrão de atualização*](http://gameprogrammingpatterns.com/update-method.html).

```ts
// System básico
function mySystem() {
	console.log('meu system está em execução')
}

engine.addSystem(mySystem)

// System com dt
function mySystemDT(dt: number) {
	console.log('tempo desde o último frame:  ', dt)
}

engine.addSystem(mySystemDT)
```

Uma única scene pode ter 0 ou muitos systems em execução ao mesmo tempo. Systems podem ser ativados ou desativados em diferentes momentos durante a duração da scene. Geralmente, é uma boa prática manter comportamentos independentes em systems separados.

Veja [Systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) para mais detalhes sobre como systems são usados em uma scene.

### O game loop

A [game loop](http://gameprogrammingpatterns.com/game-loop.html) é a base do código de uma scene da Decentraland. Ele percorre parte do código em intervalos regulares e faz o seguinte:

* Escutar o input do player
* Atualizar a scene
* Renderizar novamente a scene

Na maioria dos programas de software tradicionais, todos os eventos são acionados diretamente pelas ações do player. Nada no estado do programa mudará até que o player clique em um botão, abra um menu, etc.

Mas ambientes interativos e jogos são diferentes disso. Nem todas as mudanças na scene são necessariamente causadas pelas ações de um player. A sua scene pode ter objetos animados que se movem por conta própria ou até personagens não jogáveis que têm sua própria IA. Algumas ações do player também podem levar vários ticks para serem concluídas; por exemplo, se a abertura de uma porta precisar levar um segundo inteiro, a rotação da porta deverá ser atualizada incrementalmente cerca de 30 vezes enquanto ela se move.

Chamamos cada iteração do loop de *tick*. Scenes da Decentraland são renderizadas a 30 ticks por segundo, sempre que possível. Se a máquina estiver com dificuldade para renderizar cada tick, isso pode resultar em atualizações menos frequentes.

Em cada tick, a scene é atualizada; em seguida, a scene é renderizada novamente, com base nos valores atualizados.

Nas scenes da Decentraland, não há um game loop explicitamente declarado; em vez disso, o [Systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) da scene compõem o game loop.

A compilação e o rendering da scene são feitos no backend; você não precisa lidar com isso ao desenvolver sua scene.

## Consultando components

Você pode [consultar componentes](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/) com o método `engine.getEntitiesWith(...components)` para acompanhar todas as entities na cena que têm certos componentes.

Muitas vezes faz sentido consultar components dentro de um [system](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/), para então iterar sobre cada uma das entities retornadas e executar o mesmo conjunto de actions em cada uma.

Se você tentar iterar sobre todas as entities na scene em cada tick do game loop, isso pode ter um custo significativo de performance. Ao se referir apenas às entities retornadas por uma consulta, você garante que está lidando apenas com aquelas que são relevantes.

```ts
// Defina um System
function boxHeightSystem(dt: number) {
	// consultar entities que incluem tanto components MeshRenderer quanto Transform
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.get(entity)
		console.log('uma box está na altura:  ', transform.position.y)
	}
}

// Adicione o system ao engine
engine.addSystem(boxHeightSystem)
```

## Ciclo de vida da scene

Se você começar a escrever linhas soltas de código diretamente em `index.ts`, seu código pode estar sem algum contexto importante. Por exemplo, você ficará sem informações sobre o player entity, ou sobre entities que foram adicionadas via arrastar e soltar no Creator Hub. No momento em que suas linhas de código forem lidas, essas coisas ainda não estarão carregadas.

Para evitar esse cenário, é sempre recomendado escrever o código de carregamento inicial da sua scene usando a `main()` function (no `index.ts` file) como ponto de entrada. Essa function só é executada depois que todo o contexto inicial da scene já estiver carregado; isso inclui qualquer coisa adicionada via a UI do Scene Editor.

Você pode escrever seu código fora da `main()` function quando:

* O código é chamado indiretamente por `main()`
* O código define uma system ou adiciona uma system ao engine
* O código está dentro de uma [async function](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/)


> [!WARNING]
> **📔 Nota**: Quando o código dentro de uma async function ou de uma system é executado pela primeira vez, tudo na scene já está devidamente inicializado.
> 
> [Component personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) definições são uma exceção; elas devem sempre ser escritas fora da `main()` function, em um arquivo separado. Elas precisam ser interpretadas antes de `main()` ser executado.


## Mutabilidade

Você pode escolher lidar com versões mutáveis ou imutáveis (somente leitura) de um component. A `.get()` function em um component retorna uma versão imutável do component. Você só pode ler seus valores, mas não pode alterar nenhuma de suas properties.

A `.getMutable()` a function retorna uma representação do component que permite alterar seus valores. Use versões mutáveis apenas quando você planeja fazer alterações em um component. Lidar com versões imutáveis de components resulta em um enorme ganho de performance.

```ts
// buscar uma versão imutável (somente leitura)
const immutableTransform = Transform.get(myEntity)

// o seguinte NÃO funciona:
// 	immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// o seguinte REALMENTE altera a posição da entity
mutableTransform.position.y = 2
```

Veja [dados mutáveis](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/mutable-data/) para mais detalhes.

## Juntando tudo

A *engine* é o que fica entre *entities*, e *components* de um lado e *systems* de outro.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

Todos os valores armazenados nos components da scene representam o estado da scene naquele ponto no tempo. A cada tick do game loop, o engine executa as functions de cada uma das systems para atualizar os valores armazenados nos components.

Depois que todas as systems executam, os components de cada entity terão novos valores. Quando o engine renderiza a scene, ele usará esses novos valores atualizados e os players verão as entities mudando para corresponder aos seus novos estados.

```ts
export function main() {
	// Criar uma Entity
	const cube = engine.addEntity()

	// Dar à Entity uma posição por meio de um component Transform
	Transform.create(cube, {
		position: Vector3.create(5, 1, 5),
	})

	// Dê à entity uma forma visível por meio de um component MeshRenderer
	MeshRenderer.setBox(cube)
}

// Defina um System
function rotationSystem(dt: number) {
	// consultar entities que incluem tanto components MeshRenderer quanto Transform
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.getMutable(entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(dt * 10, Vector3.Up())
		)
	}
}

// Adicione o system ao engine
engine.addSystem(rotationSystem)
```

No exemplo acima, uma `cubo` entity e uma `rotationSystem` system são adicionados ao engine. A `cubo` entity tem um `Transform`, e um `MeshRenderer` component. Em cada tick do game loop, o `rotationSystem` system é chamada, e ela altera os valores de rotação no `Transform` component da `cubo` entity.

Observe que a maior parte do código acima é executada apenas uma vez, ao carregar a scene. A exceção é o `rotationSystem` system, que é chamado em cada tick do game loop.

## Desacoplamento da scene

Suas scenes não são executadas no mesmo contexto que o engine (também conhecido como a main thread). Criamos o SDK de uma forma que é totalmente desacoplada do rendering engine. Projetamos isso assim por razões de segurança e performance.

Devido a esse desacoplamento, o código da sua scene não tem acesso ao DOM nem ao `window` object, então você não pode acessar dados como o navegador do player ou sua localização geográfica.

O desacoplamento funciona usando o protocolo RPC; esse protocolo atribui uma pequena parte do client apenas para renderizar a scene e controlar eventos.

Também abstraímos o protocolo de comunicação. Isso nos permite executar as scenes localmente em um WebWorker.

Não queremos que developers interfiram com os detalhes internos do engine nem sequer precisem saber o que há dentro do engine. Precisamos garantir uma experiência consistente para os players em todo o mapa da Decentraland, e erros são mais prováveis de acontecer nesse nível "baixo".

Esse desacoplamento também é importante para evitar que scenes vizinhas interfiram na experiência dos players enquanto eles estiverem na scene de outra pessoa. Um player pode ter várias scenes próximas carregadas ao mesmo tempo, cada uma executando seu próprio código. Algumas actions (como abrir links externos ou mover o player) só são permitidas quando o player está em pé naquela scene específica, e não se a scene estiver carregada, mas o player estiver fora.

## Tree Shaking

Ao converter o código-fonte em TypeScript para o código compilado em JavaScript minificado, o processo realiza [tree shaking](https://en.wikipedia.org/wiki/Tree_shaking) para garantir que apenas as partes do código que estão realmente sendo usadas sejam convertidas. Isso ajuda a manter o código final da scene o mais leve possível. Isso é especialmente útil ao usar libraries externas, já que muitas vezes essas libraries incluem muita functionality que não é usada e que, de outra forma, aumentaria o tamanho da scene.

Como consequência do tree shaking, qualquer código que você queira que sua scene execute precisa ser referenciado de uma forma ou de outra pelos entry points do seu código: a `main()` function em `index.ts`. Systems também podem ser adicionados ao engine alternativamente no `index.ts` file, sem referenciar `main()`. Qualquer código que não seja explicitamente ou indiretamente referenciado por esses arquivos não entrará na scene.

Por exemplo, suponha que você tenha um arquivo chamado `extraContent.ts` com o conteúdo a seguir, a entity não será renderizada e o system não começará a executar:

```ts
// extraContent.ts

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: { x: 8, y: 0, z: 8 },
})
MeshRenderer.setBox(myEntity)

function mySystem(dt: number) {
	console.log('system em execução')
}

engine.addSystem(mySystem)
```

Para fazê-lo executar como parte da sua scene, você pode referenciá-lo a partir de `index.ts` da seguinte maneira:

```ts
// em extraContent.ts

export function addEntities() {
	const myEntity = engine.addEntity()
	Transform.create(myEntity, {
		position: { x: 8, y: 0, z: 8 },
	})
	MeshRenderer.setBox(myEntity)
}

export function mySystem(dt: number) {
	console.log('system em execução')
}

/////////////////////////////

// em index.ts

import { addEntities, mySystem } from './extraContent'

export function main() {
	addEntities()
}

engine.addSystem(mySystem)
```

A exceção a essa regra são as definições de custom components. Elas não devem ser acessadas pelo `main()` ponto de entrada da function, pois precisam ser interpretadas antes de tudo.

## Imports

Todas as functions, objects, components e outros elementos usados pela scene devem ser importados para cada arquivo para serem usados. Isso é uma consequência de [tree-shaking](#tree-shaking), pois evita empacotar o SDK inteiro e, em vez disso, inclui apenas as partes que a scene usa.

Os snippets ao longo da documentação omitem as linhas de import no início de cada arquivo para mantê-los limpos, mas, para funcionarem, você precisa adicioná-las à scene.

Ao usar o VS Studio Code para escrever suas scenes, as opções inteligentes de autocompletar devem cuidar do tratamento dos imports para você enquanto você escreve, sem que precise estar ciente disso.

Quando você colar um snippet na sua scene, no entanto, provavelmente verá alguns elementos marcados em vermelho, que não foram importedidos para aquele arquivo. Para corrigir isso:

* Clique em cada palavra sublinhada
* Clique no ícone de lâmpada à esquerda da linha
* Selecione **Add Import From**
* Uma linha de import aparece no início do arquivo.

![](/docs/_remote/d183f4383d8bff7a66afa15b.webp)

Se houver muitas coisas diferentes para importar, você também pode selecionar **Add all missing imports** no mesmo dropdown.

Observe que os imports devem ser feitos em todo arquivo onde um elemento é usado.

O VS Studio Code deve conseguir resolver os paths corretos dos seus imports por conta própria. Se, por qualquer motivo, ele estiver tendo dificuldade nisso, um truque é colar as seguintes declarações de import vazias no início do seu arquivo. O VS Studio deve conseguir seguir a partir daí.

```ts
import {} from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'
```

## Versões do SDK

Ao desenvolver uma nova scene, você usa a `@latest` versão estável do SDK por padrão.

Você pode instalar a `@next` versão do SDK se quiser aproveitar ou pré-visualizar recursos futuros que ainda não chegaram à versão estável mais recente.

Para isso, abra o `package.json` arquivo da sua scene e altere as seguintes linhas:

```json
  "devDependencies": {
    "@dcl/js-runtime": "next",
    "@dcl/sdk": "next"
  },
```

Depois, execute o seguinte comando na pasta do projeto da sua scene:

```
npm i
```

Veja [gerenciar dependências](/docs/creator/content-creator-pt/scenes-sdk7/bibliotecas/manage-dependencies/) para mais detalhes.


> [!WARNING]
> **📔 Nota**: Lembre-se de que a versão @next pode apresentar problemas de vez em quando. A sintaxe e o nome de novos recursos podem mudar antes de serem lançados em uma versão estável.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
