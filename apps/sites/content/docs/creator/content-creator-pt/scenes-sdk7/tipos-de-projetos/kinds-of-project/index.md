> Os tipos de projetos disponíveis que você pode criar no Decentraland.

# Visão geral

Existem diferentes tipos de conteúdo que você pode criar para Decentraland.

![](/docs/_remote/0a4a8ae5cf4dbf5eb00b75db.webp)

## Cenas

As cenas de Decentraland podem ser hospedadas em Parcels em Genesis City ou em WORLDs.

### Publicar no LAND

As cenas publicadas em Parcels podem ser encontradas em coordenadas específicas dentro do mundo aberto de Decentraland.

Essas cenas estão vinculadas a tokens LAND ou Estate. Cada parcel ocupa 16x16 metros. Várias parcels adjacentes podem ser usadas por uma única cena; elas podem ser organizadas em qualquer formato, desde que as bordas se toquem.

Os tokens LAND podem ser comprados no [Marketplace](https://decentraland.org/marketplace/lands). Há um estoque limitado deles, cobrindo o mapa de Genesis City.

As cenas publicadas no LAND são mais fáceis de descobrir, pois os jogadores podem encontrá-las ao visitar conteúdo próximo ou explorar.

As cenas publicadas no LAND podem usar até 15 MB de espaço por cada parcel na cena. Quanto mais parcels, mais espaço disponível. Isso serve para evitar sobrecarregar a CPU do jogador, já que os jogadores podem estar vivenciando muitas cenas próximas ao mesmo tempo. Veja [limitações de tamanho](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/).

### Publicar em WORLDs

As cenas publicadas em um World devem ser acessadas por meio de um link.

Essas cenas estão vinculadas a tokens NAME. Os tokens NAME podem ser comprados no [Marketplace](https://decentraland.org/marketplace/names/claim). Você pode reivindicar qualquer nome que quiser, desde que ainda não tenha sido reivindicado. Como alternativa, você pode usar um [domínio ENS](https://ens.domains) para criar um World.

As cenas publicadas em um World têm uma limitação de parcel de 300x300. A mesma [limitações de tamanho](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) por parcel se aplicam como em cenas publicadas em parcels LAND, mas você pode adicionar mais parcels à sua cena sem nenhum custo.

Se for um Multi-Scene WORLD, diferentes Collaborators (endereços) podem ter direitos de publicação para parcels específicos do WORLD, definidos pelo World Owner (o endereço que reivindicou o World NAME). Além disso, o acesso a um World pode ser **Público** ou **Privado**. Mais informações sobre [Gerenciar Worlds](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/#managing-worlds)

Um World pode ser **Público** ou **Privado**

#### Limites de tamanho

O tamanho máximo de arquivo que você pode enviar para o seu World depende de você estar usando um Decentraland NAME ou um domínio ENS.

**Worlds de Decentraland NAMEs**

Os detentores de Decentraland NAME desfrutam de capacidade de armazenamento dinâmica dentro do Foundation Worlds Content Server, que depende dos ativos em sua wallet. As regras a seguir regem essa alocação:

* Cada Decentraland NAME que você possui concede 100 MB de capacidade de armazenamento (assim como um World).
* Cada parcel de Decentraland LAND que você possui concede 100 MB adicionais de capacidade de armazenamento.
* A cada 2.000 MANA mantidos em sua wallet, são concedidos 100 MB adicionais de capacidade de armazenamento.

O espaço no Foundation Worlds Content Server pode ser usado para hospedar scenes tão grandes quanto os usuários quiserem, utilizando os Decentraland NAMEs que possuem e o espaço combinado concedido por seus ativos Decentraland coletivos. Por exemplo, um usuário com múltiplos Worlds (concedidos por possuir múltiplos NAMES) e uma capacidade de armazenamento combinada de 500 MB pode escolher implantar um World com um arquivo de cena de 200 MB, outro com um arquivo de cena de 200 MB e um terceiro com um arquivo de cena de 100 MB. Alternativamente, poderia optar por implantar um World com um arquivo de cena de 300 MB e outro com um arquivo de cena de 200 MB.

Você pode verificar quanto do seu orçamento de armazenamento está sendo usado e quanto resta em dois lugares:

* A **Gerir** na seção do Creator Hub. Clique **Ver Detalhes** para ver um detalhamento de como suas posses de MANA, LAND e NAME se somam.
* A **Worlds** separador do [Builder](https://decentraland.org/builder/worlds), que também mostra quanto espaço cada um dos seus Worlds está usando.

**Worlds de domínios ENS**

Em contraste, Worlds concedidos por domínios ENS têm um tamanho máximo fixo de arquivo de cena de 36 MB por World, independentemente das outras posses de Decentraland do usuário. Usuários com Worlds de domínios ENS não podem aumentar o limite de tamanho da cena de seu ENS World comprando MANA ou LAND adicionais.

No entanto, Worlds concedidos pela posse de um domínio ENS servem como o primeiro passo perfeito para concretizar a liberdade criativa oferecida por Decentraland.

#### Diferenças em relação às cenas de LAND

Worlds oferecem várias vantagens em relação às cenas de LAND:

* Não é necessário possuir LAND - adicione quantas parcels quiser à sua cena sem custo (até 300x300)
* Acesso por link - os jogadores podem visitar seu World diretamente pela URL
* Mais capacidade de armazenamento - especialmente para detentores de Decentraland NAME
* Controle de acesso - você pode permitir que apenas certos jogadores acessem seu World

Veja [Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para mais informações, ou saiba mais sobre [publicação para Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/#publishing-to-worlds) e [configurar as configurações do World](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#world-configuration).

## Cenas globais

As cenas globais podem transformar o cenário já existente de Decentraland, adicionando camadas de interatividade e jogabilidade. São scenes que não ficam restritas a rodar apenas em certos parcels de LAND ou em certos Worlds. Os jogadores as levam consigo para onde quer que vão.

### Smart Wearables

Os Smart Wearables estão vinculados a tokens Wearable. Eles são vendidos como NFTs e comprados no [Marketplace](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Os Smart Wearables são ativados sempre que o jogador veste o wearable associado. Eles são desativados se o jogador tirar o item, ou também podem desativar manualmente a cena global via a UI. Além disso, a cena em que o jogador está pode suspender quaisquer Smart Wearables que estejam ativos na cena.

* Aprenda tudo sobre [Criando wearables](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/).
* Saiba mais sobre [smart wearables](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
