> Como obter permissões de World do Decentraland ou LAND para publicar sua scene

# Opções de Publicação

Para convidar outras pessoas a visitar uma cena que você construiu, você precisará publicá-la em Decentraland. Para isso, você precisará ter permissões de deploy em um Decentraland World ou possuir parcelas de Decentraland LAND.

Este documento oferece uma visão geral das diferentes maneiras de obter permissões. Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para instruções sobre como publicar depois de obtê-las.

### Decentraland Worlds

Decentraland Worlds são seu próprio espaço 3D pessoal no ecossistema Decentraland, separado do mapa público de parcelas de LAND de Genesis City.

Um World pode ser mantido privado ou compartilhado com qualquer pessoa apenas com um link. Capaz de hospedar até 100 usuários simultâneos, você pode usar seu World para:

* Hospedar eventos
* Construir novas experiências
* Compartilhar um portfólio de cenas
* Testar cenas antes de implantá-las no LAND de Genesis City

### Como obter um World

Há duas maneiras de obter um Decentraland World:

* **Obtenha um Decentraland NAME (recomendado).**\
  Você pode obter seu próprio Decentraland World ao obter um [Decentraland NAME](https://builder.decentraland.org/names), que são NFTs de NAME dentro do ecossistema DCL. Um NAME custa 100 MANA e imediatamente lhe dá um World com esse nome. Você pode [comprar um novo NAME](https://decentraland.org/shop/items?category=names) ou comprar um existente de um proprietário anterior no [Marketplace](https://decentraland.org/marketplace/names).
* **Compre um domínio ENS.**\
  Você também pode obter um World comprando um [domínio ENS](https://ens.domains/), um nome descentralizado que pode ser usado tanto no Web2 quanto no Web3. Os Worlds concedidos por domínios ENS têm um tamanho máximo fixo de arquivo de cena de 36 MB por World, independentemente de suas outras participações em Decentraland.

Depois de ter um NAME ou domínio ENS, você pode publicar cenas nesse World usando o fluxo padrão de publicação.\
Você também pode permitir que outras pessoas publiquem no seu Decentraland World. Consulte a [Publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) documentação de Gerenciamento de Worlds para saber mais sobre Multi Scene Worlds e outros tópicos como Worlds privados. [documentação de Gerenciamento de Worlds](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#managing-worlds) para saber mais sobre Multi Scene Worlds e outros tópicos como Worlds privados.

### Capacidade de armazenamento do World

Dentro de um World, você pode usar quantas parcelas quiser. Para os detentores de Decentraland NAME, o limite de tamanho da sua cena em MegaBytes depende de quantos NAMES, MANA e parcelas de LAND você possui. Veja [limites de tamanho dos Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits) para informações detalhadas sobre como a capacidade de armazenamento é calculada.

Você pode verificar seu orçamento de armazenamento usado e restante na **seção Gerenciar** do Creator Hub, ou na **aba Worlds** do [Builder](https://decentraland.org/builder/worlds).

### Saiba mais sobre Worlds

* [Worlds como tipo de projeto](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#publish-to-worlds) - Saiba mais sobre limites de tamanho e como Worlds diferem de cenas de LAND
* [Publicação em Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/#publishing-to-worlds) - Instruções passo a passo de publicação
* [Configuração do World](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#world-configuration) - Configure skybox, definições de comunicação e mais
* [Gerenciamento de Worlds](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/manage-scenes/#managing-worlds) - Visualize o espaço de armazenamento e gerencie implantações
* [Perguntas frequentes sobre Worlds](https://github.com/decentraland/docs/tree/main/creator/scene-editor/faq/README.md) - Perguntas comuns sobre Worlds

### Publicação em Decentraland LAND

Genesis City é o espaço compartilhado de Decentraland: o grande mapa público composto por parcelas de LAND. Cada parcela de LAND é **16 metros × 16 metros**. Uma cena pode ser construída em uma ou mais parcelas.

Se você preferir publicar em LAND, há três opções:

* **Alugar LAND**: Adquira o direito de implantar e manter seu conteúdo em determinadas parcelas por um período fixo de tempo. Veja [Aluguéis](/docs/player/marketplace/rentals/).
* **Comprar LAND**: Compre parcelas de LAND e publique seu conteúdo pelo tempo que desejar. Veja [Marketplace](/docs/player/marketplace/marketplace/).
* **Obter permissões de um proprietário**: Vários proprietários de terrenos e distritos gerenciam grandes extensões de terra e estão dispostos a hospedar gratuitamente conteúdo de outras pessoas, se ele se encaixar em seu tema e no tipo de ambiente que desejam criar. Conecte-se com eles no [servidor Discord de Decentraland](https://dcl.gg/discord).

### Como as permissões de LAND funcionam

Os tokens de LAND podem gerenciar permissões que permitem que os seguintes tipos diferentes de funções implantem em uma determinada parcela ou estate.

* **Proprietário de LAND**: A pessoa que possui o token em sua carteira Ethereum. Esse usuário pode sempre implantar, atribuir outras funções ou transferir o token (a menos que o LAND esteja atualmente alugado por outra pessoa).
* **Operador de LAND**: O operador só pode implantar, enquanto estiver com essa função. O endereço do operador é referenciado pelo token, sem precisar possuir nada. O proprietário pode atribuir e revogar permissões de operador para qualquer número de endereços. Veja [Conceder permissões](/docs/player/marketplace/land-manager/#give-permissions).

Ao alugar uma LAND, o inquilino que paga pelo aluguel pode escolher um único endereço que terá uma **Operador de LAND** função durante a vigência do aluguel (esse pode ser o próprio endereço dele ou o de outra pessoa). Enquanto o aluguel estiver ativo, nenhum dos detentores originais das funções poderá interferir no conteúdo dessa LAND ou transferir o token.

### Como publicar sua cena

Quando você tiver obtido seu World ou permissões de LAND, siga o guia padrão de [Publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para implantar sua cena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
