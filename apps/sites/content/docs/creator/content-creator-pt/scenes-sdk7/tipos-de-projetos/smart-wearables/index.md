> Crie wearables com capacidades interativas

# Smart Wearables

Os Smart Wearables são um tipo de global scene. Como [experiências portáteis](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/portable-experiences/), são jogabilidade que os jogadores levam consigo à medida que se movem pelo metaverso. Por exemplo, enquanto executa uma global scene, um jogador poderia apanhar uma bola de neve do chão em Genesis Plaza, afastar-se para outra scene e lançar a bola de neve a outro jogador que também esteja a jogar o mesmo jogo.

Os Smart Wearables são experiências portáteis que são ativadas quando o jogador veste uma determinada peça de roupa. Os Smart Wearables podem conceder aos jogadores novas habilidades, como um jetpack que lhes permite voar, ou acrescentar uma nova camada de conteúdo por cima do resto do mundo, como colocar aleatoriamente moedas para serem recolhidas por toda a Genesis City.


> [!WARNING]
> **📔 Nota**: Smart Wearables só podem ser criados usando SDK 7.
> 
> O **Creator Hub** não suporta atualmente a criação de projetos de Smart Wearables.


## Introdução

### Usando a CLI

1. Abra uma linha de comandos numa nova pasta e execute

`npx @dcl/sdk-commands init --project smart-wearable`

Este comando cria os ficheiros básicos e a estrutura para um novo smart wearable.

## Os ficheiros no template

A pasta de um projeto Smart Wearable totalmente novo é muito semelhante à de uma [scene do Decentraland](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-files/), mas irá notar as seguintes diferenças:

* `wearable.json` inclui todos os metadados da experiência portátil
* Há um modelo 3D provisório (glasses.glb) e uma miniatura (glasses.png) para um par de óculos escuros. Deve substituí-los pelo wearable real que está a criar
* `scene.json` inclui `"isPortableExperience": true`. Os Parcels não são relevantes para o wearable publicado, que funciona em qualquer lugar, mas pode defini-los para testes locais: a preview só é executada nos Parcels listados em `scene.json`

## Sobre wearable.json

O `wearable.json` ficheiro predefinido tem o seguinte aspeto:

```json
{
	"data": {
		"replaces": [],
		"hides": [],
		"tags": ["special", "new", "eyebrows"],
		"representations": [
			{
				"bodyShapes": [
					"urn:decentraland:off-chain:base-avatars:BaseMale",
					"urn:decentraland:off-chain:base-avatars:BaseFemale"
				],
				"mainFile": "glasses.glb",
				"contents": ["glasses.glb"],
				"overrideHides": [],
				"overrideReplaces": []
			}
		],
		"category": "eyewear"
	},
	"name": "Smart Wearable Example",
	"description": "Coloque os óculos para ver um novo mundo",
	"rarity": "mythic"
}
```

Estes são os campos mais comuns em `wearable.json`:

* `id`: ID único do smart wearable.
* `nome` (obrigatório): O nome do wearable que os utilizadores verão no Marketplace
* `descrição`: A descrição do wearable que os utilizadores verão no Marketplace. Certifique-se de indicar o que o smart wearable pode fazer, uma vez que os utilizadores do Marketplace não terão forma de pré-visualizar a sua funcionalidade antes de comprar. Pode ter no máximo **64 caracteres** de comprimento. Pode escrever uma descrição mais longa no editor de itens do Builder depois de carregar.
* `raridade`: O nível de raridade da oferta do token. Os valores possíveis são:
  * unique (1 cópia)
  * mythic (10 cópias)
  * exotic (50 cópias)
  * legendary (100 cópias)
  * epic (1000 cópias)
  * rare (5000 cópias)
  * uncommon (10.000 cópias)
  * common (100.000 cópias)


> [!WARNING]
> **📔 Nota**: Se bifurcou o projeto a partir de um existente e ele inclui um `id`, certifique-se de que o valor é único antes de publicar o seu wearable. Use [uuidgenerator.net](https://www.uuidgenerator.net/) para gerar um novo UUID aleatório


* `data` (obrigatório): Inclui o seguinte. `substitui`, `hides`, `tags`, `representations` e `category` devem estar todos presentes. `substitui`, `hides` e `tags` podem ser listas vazias `[]`, mas `representations` precisa de pelo menos uma entrada. Também pode ajustar estas definições a partir da UI do Builder depois de carregar o smart wearable.
  * `substitui`: Lista de categorias de outros wearables que devem ser removidos ao equipar este wearable, além da predefinição desta categoria. Ex.: Ao vestir uma capa de upper\_body, oculte também os feet.
  * `hides`: Lista de categorias de outros wearables que devem ser ocultados (mas não removidos) ao equipar este wearable, além da predefinição desta categoria.
  * `tags`: Tags usadas para tornar o wearable pesquisável no Marketplace.
  * `representations`:
    * `bodyShapes`: A lista de representações de avatar que podem usar este wearable. Todos os smart wearables devem estar disponíveis para ambas as bodyShapes, masculina e feminina.
    * `mainFile`: O ficheiro principal com o modelo 3D do wearable.
    * `contents`: A lista completa de ficheiros usados para renderizar o modelo 3D do wearable. Por exemplo, o modelo 3D pode incluir texturas como ficheiros separados.
    * `overrideHides`: Quaisquer exceções ao comportamento padrão de *ocultar* desta categoria de wearable.
    * `overrideReplaces`: Quaisquer exceções ao comportamento padrão de *substituir* desta categoria de wearable.
  * `category`: Que categoria de wearable usar. Os valores possíveis são:
    * 'eyebrows'
    * 'eyes'
    * 'facial\_hair'
    * 'hair'
    * 'mouth'
    * 'upper\_body'
    * 'lower\_body'
    * 'feet'
    * 'earring'
    * 'eyewear'
    * 'hat'
    * 'helmet'
    * 'mask'
    * 'tiara'
    * 'top\_head'
    * 'skin'
    * 'hands\_wear'


> [!WARNING]
> **📔 Nota**: Além dos campos acima, o Builder também aceita um campo opcional de nível superior `mapeamento` e campos opcionais `removesDefaultHiding` e `blockVrmExport` dentro de `data`. Os smart wearables normalmente não precisam deles. O Builder rejeita um `wearable.json` que tenha qualquer outro campo de nível superior, com o erro *"the wearable config file is invalid"*. Versões anteriores desta página listavam `menuBarIcon`, `model`, `bodyShape` e `collectionAddress`: remova-os se o seu projeto ainda os tiver. O mesmo erro aparece se `descrição` tiver mais de 64 caracteres.


## A miniatura

Deve incluir uma imagem chamada `thumbnail.png` ao nível raiz na sua pasta. Esta imagem será mostrada tanto na backpack como no Marketplace, para representar o seu wearable em 2D. O tamanho de imagem recomendado é 256x256.

Escolha uma imagem que defina as expectativas do jogador e represente adequadamente a sua criação.

## A preview

Executar uma preview de um smart wearable é como executar a de uma scene: clique **Play** no Creator Hub, ou execute `npm run start` na linha de comandos. O Creator Hub não consegue criar um projeto de smart wearable, mas pode abrir e pré-visualizar um.

A preview executa o código do seu wearable como uma scene regular nos Parcels listados em `scene.json`, e não como uma experiência portátil. Isto significa que alguns comportamentos diferem do wearable publicado:

* O código que só funciona na scene em que o jogador se encontra, como [forças físicas no jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics/), deixa de funcionar quando sai dos Parcels da preview. Depois de publicado, o smart wearable continua a funcionar onde quer que o jogador vá.
* O modelo 3D do wearable pode não aparecer no seu avatar.

Para experimentar o wearable equipado e a funcionar em qualquer lugar, carregue-o no Builder e use **Ver no Decentraland** (ver [Publicar](#publish)). Não precisa de fazer mint nem publicar a collection para fazer isto.

Para testar como o smart wearable se comporta no contexto de uma scene, também pode executar uma preview do seu wearable ao mesmo tempo que executa uma preview de uma ou várias scenes, utilizando um [Workspace](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/workspaces/). Por exemplo, pode executar o seu smart wearable juntamente com a scene de [Genesis Plaza](https://github.com/decentraland-scenes/Genesis-Plaza) para testar como se comporta numa scene movimentada, enquanto num elevador, etc.

## Dicas

* Ao posicionar uma Entity, note que as posições são globais, relativas às coordenadas 0,0 de Genesis Plaza.
* Para reagir a jogadores próximos:
  * Veja [Buscar todos os jogadores](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#fetch-all-players) para saber como obter dados de outros jogadores nas redondezas.
  * Tenha em conta que o carregamento do smart wearable, das scenes em redor e de outros jogadores pode ocorrer em ordens diferentes consoante a situação. Se o jogador entrar no Decentraland com o smart wearable já vestido, é provável que a scene global do seu wearable seja carregada antes das outras pessoas. Por outro lado, se o jogador carregar primeiro numa scene e depois vestir o wearable, é provável que os outros jogadores já tenham sido carregados quando a scene do wearable começar a ser executada.
  * Os outros jogadores só são carregados depois de o jogador estar ligado a uma ilha dentro do seu realm. Busque os dados do realm e verifique o campo ‘room’. Se o campo ‘room’ for null, o jogador ainda não está ligado a uma ilha e os outros jogadores ainda não serão carregados. Pode verificar periodicamente isto a cada 1 segundo até o campo ‘room’ estar presente, e só então inicializar a sua lógica. Isto apenas lhe permite ver outros jogadores: para partilhar dados entre utilizadores que usam wearables, consulte [Limitações](#limitations).
* Para interagir com scenes em redor:
  * Não pode enviar diretamente quaisquer instruções para scenes próximas ou outras experiências portáteis, o `messageBus` está atualmente isolado para cada experiência portátil/scene.
  * Pode usar um servidor intermédio para enviar informação entre a experiência portátil e uma scene.
  * Se fizer um raycast, pode detetar impactos contra os colliders de Entities das scenes em redor. Isto pode dizer-lhe a localização exata do impacto, a direção da normal e até o nome da Entity e o nome do mesh do modelo 3D. Isto só funciona ao atingir Entities em scenes escritas com SDK7.
* Terminar uma experiência portátil: Execute o `kill()` método para terminar a si própria uma experiência portátil.
* Empurrar o jogador: [Physics](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics/) funções como `Physics.applyImpulseToPlayer()` funcionam em qualquer lugar para onde o utilizador vá, uma vez que a scene de um smart wearable conta sempre como a scene em que o jogador se encontra. Não é necessária qualquer permissão. As scenes podem desativar smart wearables para os seus visitantes, consulte [Feature Toggles](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#feature-toggles).
* Tocar um som que só o utilizador ouve: anexe uma Entity ao jogador com [AvatarAttach](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#attach-an-entity-to-an-avatar) e atribua-lhe um `AudioSource`. Os outros jogadores não o ouvem.
* Carregar ficheiros antes de serem necessários: use o componente [AssetLoad](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/pre-load-resources/) para obter sons, modelos ou ficheiros de emote quando o wearable inicia, para que o primeiro uso não fique à espera de um descarregamento.

## Publicar

Para publicar o seu smart wearable:

1. Certifique-se de que a informação em `wearable.json` está correta. Se usou outro projeto como ponto de partida, certifique-se de que o `id` é um identificador único, não usado por outros wearables.
2. Execute `npm run pack` na pasta do seu projeto. Isto gera um `smart-wearable.zip` na pasta do seu projeto.


> [!WARNING]
> **📔 Nota**: O conteúdo descomprimido de `smart-wearable.zip` deve somar menos de 3MB, ou o Builder rejeita o ficheiro por ser demasiado grande. `npm run pack` imprime uma mensagem de tamanho quando o projeto ultrapassa 2MB, mas ainda assim cria o zip. Verifique o tamanho dos ficheiros no zip, não o tamanho do zip em si. Consulte [Limitações](#limitations).


3. Abra o Builder, abra o separador Collections, clique em + para carregar um novo wearable.
4. Arraste o seu `smart-wearable.zip` comprimido para o Builder e verifique se toda a informação está correta.

> Nota: Se o seu wearable tiver um modelo 3D diferente para cada bodyShape, liste uma representação por bodyShape:
>
> 1. No seu projeto, crie uma nova pasta para cada representação(`male` e `female`), e coloque o modelo 3D de cada representação na pasta correspondente.
> 2. Atualize o seu `wearable.json` ficheiro para incluir as novas representações.
> 3. ```lang-json
>    "representations": [{
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
>      "mainFile": "male/glasses.glb",
>      "contents": ["male/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    },
>    {
>      "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
>      "mainFile": "female/glasses.glb",
>      "contents": ["female/glasses.glb"],
>      "overrideHides": [],
>      "overrideReplaces": []
>    }],
>    ```
> 4. Execute `npm run pack` para gerar um novo ficheiro smart-wearable.zip.
> 5. Arraste o novo ficheiro smart-wearable.zip para o Builder.

5. Abra o editor de wearables e certifique-se de que as categorias “hide” e “remove” estão corretamente definidas para desativar outras categorias de wearables quando este wearable estiver equipado.
6. Crie uma nova coleção com este e talvez com outros wearables.
7. Na coleção, clique **Ver no Decentraland**. Isto abre o Decentraland na rede de testes Sepolia, onde pode equipar os wearables da sua coleção a partir da backpack e ver como se comportam num cenário mais real, por exemplo a correr em volta de Genesis Plaza. Consulte [Testar no Decentraland](/docs/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-smart-wearables/#testing-in-decentraland).
8. Nesta fase, o seu wearable está pronto para ser publicado.

## Permissões

Os smart wearables são executados com permissões restritas. Para usar qualquer uma das seguintes funcionalidades, declare a permissão correspondente na `requiredPermissions` lista do seu `scene.json`, ou a chamada falha em runtime:

* `USE_FETCH`: pedidos HTTP com `fetch` ou `, e outras APIs do runtime contam para o mesmo limite. Chamadas em excesso não são`
* `USE_WEBSOCKET`: ligações WebSocket
* `USE_WEB3_API`: transações de Wallet ou assinatura de mensagens
* `OPEN_EXTERNAL_LINK`: solicitar ao jogador que abra uma ligação externa

```json
"requiredPermissions": [
    "USE_FETCH"
  ],
```

Quando um smart wearable declara qualquer uma destas permissões, os jogadores veem um pedido de autorização quando o equipam, e o wearable não é executado se recusarem. Declare apenas o que usa. Consulte [Permissões Necessárias](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#required-permissions) para detalhes.

## Limitações

> IMPORTANTE: O smart wearable inteiro tem de caber dentro de 3MB. Isto inclui o modelo 3D, a miniatura, o código, as libraries, ficheiros de som, modelos 3D adicionais, imagens da UI, etc. Este limite aplica-se à pasta descomprimida. O builder não lhe permitirá carregar wearables maiores do que isto. Para verificar o tamanho, descomprima o `smart-wearable.zip` ficheiro gerado e verifique o tamanho da pasta, ou execute `unzip -l smart-wearable.zip` e leia o total.

Para manter ficheiros fora do zip, liste-os no `.dclignore` ficheiro do projeto, da mesma forma que faria para uma scene.


> [!WARNING]
> **📔 Nota**: Se editar o seu projeto de smart wearable no Creator Hub, pode guardar uma captura de ecrã em tamanho real da vista 3D como `assets/Images/autogenerated-thumbnail.png` e apontar `scene.json` para ele. Essa imagem pode ter vários MB por si só e fazer com que o wearable ultrapasse o limite. Os smart wearables não a utilizam, por isso adicione `assets/Images/autogenerated-thumbnail.png` como `.dclignore`.


Os smart wearables só executam a experiência portátil para o jogador que está a usar o wearable. Os outros jogadores não veem os efeitos. Por exemplo, se a experiência portátil renderizar um animal de estimação que segue o jogador, os outros jogadores à volta não verão esse animal. No entanto, os outros jogadores verão os avatares executar animações que fazem parte da scene do wearable, mesmo [animações personalizadas de avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#custom-animations) carregadas como parte dos ficheiros do smart wearable.

Os smart wearables não têm multiplayer incorporado. Cada jogador que usa o wearable executa a sua própria cópia separada do código, mesmo quando dois utilizadores estão lado a lado. O `MessageBus`, sincronizar Entities com `syncEntity()` e o [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) não funciona num smart wearable. A única forma de partilhar dados entre utilizadores, como uma pontuação partilhada, é um servidor seu com o qual o wearable comunica com `fetch` ou um WebSocket, consulte [Ligações de rede](/docs/creator/content-creator-pt/scenes-sdk7/rede/network-connections/).

Uma animação personalizada pode incluir props, e os props podem incluir efeitos de partículas feitos de bones animados (ver [Props and Sounds](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/props-and-sounds/) e [Particles in Emotes](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/particles-in-emotes/)). Os outros jogadores também veem isto. O som é diferente: animações personalizadas reproduzidas com `triggerSceneEmote()` não reproduzem áudio, mesmo que o ficheiro de emote viesse com um. Para reproduzir um som, use um `AudioSource`, que só o utilizador ouve.

Os smart wearables só funcionam quando os jogadores os têm vestidos. Por esse motivo, os jogadores só podem ter um número limitado de smart wearables ativados, e dependendo da parte do corpo que ocupam, alguns serão incompatíveis com outros. Por exemplo, não pode usar dois hats ao mesmo tempo, nem um helmet e cabelo ao mesmo tempo.

Se um wearable estiver “hidden” mas não “removed” pelos outros wearables que estão a ser usados, então o smart wearable pode continuar ligado, mesmo que o wearable correspondente não esteja visível.

## Exemplos

[Exemplo de smart wearable](https://github.com/decentraland/smart-wearable-sample))


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
