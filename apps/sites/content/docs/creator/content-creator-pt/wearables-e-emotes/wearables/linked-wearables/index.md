> Representações de Wearables de tokens de terceiros

# Wearables Vinculados

## Sobre

De acordo com a [proposta inicial da DAO para Linked Wearables](https://governance.decentraland.org/proposal/?id=14e76cc0-2bc7-11ec-ac84-77607720a240) (anteriormente chamada: Third Party Wearables), a [Proposta preliminar com definições finais](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231) e a [proposta de redesenho de Linked Wearables](https://decentraland.org/governance/proposal/?id=65caf8d1-8601-49a5-ae11-b0b99d7fdd3c), este documento servirá como documentação para abranger todos os detalhes relevantes sobre a funcionalidade Linked Wearables.

Este documento destina-se sobretudo a representantes de comunidades NFT que querem dar aos seus utilizadores a possibilidade de representar os seus NFTs como wearables ao passear por Decentraland.

## O que são Linked Wearables?

Linked Wearables são representações 3D de NFTs provenientes de fora de Decentraland que podem ser usados como wearables no mundo, podem ser equipados no avatar e são encontrados na mochila. Não são [wearables regulares](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/README.md#what-are-wearables). Parecem iguais e seguem as [diretrizes](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/) de wearables regulares,

mas têm um significado completamente diferente. Linked Wearables não existem dentro de coleções tradicionais de wearables (pertencem a um tipo especial de coleção), não têm raridade e não podem ser vendidos em [mercados primário](https://market.decentraland.org/browse?assetType=item&section=wearables) ou [secundário](https://market.decentraland.org/browse?assetType=nft&section=wearables&vendor=decentraland&page=1&sortBy=recently_listed&onlyOnSale=true&viewAsGuest=false&onlySmart=false) . São apenas **representações no mundo ligadas a NFTs externos.**

> Imagine que tem um projeto NFT chamado ‘Cryptojackets’, onde cada NFT é um tipo diferente de casaco 2D e quer que os seus utilizadores tenham uma representação 3D do seu casaco na mochila do Decentraland. Linked Wearables permite-lhe submeter representações 3D dos seus NFTs como wearables em Decentraland. Não há necessidade de cunhar um novo token, e o seu projeto NFT atual terá uma nova funcionalidade pronta a usar para oferecer!

Todos os Linked Wearables são definidos dentro de uma **Linked Wearable Collection**. Veremos como criar uma mais à frente no artigo.

## Como é que os Linked Wearables representam NFTs?

Os wearables estão ligados aos seus NFTs através da criação de uma Linked Wearable Collection no [site do Builder](https://decentraland.org/builder) e da configuração de como os seus NFTs serão representados no momento de criar os wearables.

Suportamos 4 mecanismos para ligar os seus wearables aos NFTs. Todos estes mecanismos usam o token id dos NFTs para os corresponder.

A tabela seguinte mostra os mecanismos mencionados:

| Correspondência   |                                                                                                                                                                                                                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Todos os NFTs     | Um utilizador que possua qualquer um dos NFTs da coleção possuirá o wearable                                                                                                                                                                                 |
| NFT único         | Um utilizador que possua um NFT especificado por TOKEN ID possuirá o wearable. *ex.: 123456. O utilizador possuirá o wearable se possuir o NFT com TOKEN ID: 123456*                                                                                         |
| Vários NFTs       | Um utilizador que possua um de vários NFTs especificados por TOKEN IDs, descritos como separados por vírgula, possuirá o wearable. *ex.: 123456, 123457, 123458. O utilizador possuirá o wearable se possuir o NFT com TOKEN ID: 123456 ou 123457 ou 123458* |
| Intervalo de NFTs | Um utilizador que possua um de vários NFTs especificados no intervalo de TOKEN IDs possuirá o wearable. *ex.: \[1, 1000]. O utilizador possuirá o wearable se possuir o NFT com TOKEN ID 1 ou 2 ou 3 ou 4 ou 5 ... ou 100*                                   |

## Tipos de Linked Wearables Collections

Normalmente, existem dois tipos de coleções NFT:

* **Feitas à mão:** em que cada token asset é feito à medida, ou manualmente, sem qualquer processo de automatização.
* **Programáticas:** em que cada token asset não foi criado individualmente à mão, mas gerado automaticamente com código, muitas vezes a partir de traits que foram previamente concebidos e modelados. Por exemplo: [CryptoPunks](https://opensea.io/collection/cryptopunks) e [BAYC](https://opensea.io/collection/boredapeyachtclub) são exemplos de coleções NFT 2D pfp que foram criadas programaticamente.

Seguimos o mesmo princípio em Decentraland também com as Linked Wearables Collections; pode haver Linked Wearable Collections standard para as coleções feitas à mão e Linked Wearable Collections programáticas para as geradas automaticamente. Ambas as coleções diferem nos custos das taxas de publicação que o wearable tem. Consulte a [secção de Custos](#costs) para mais informações sobre as taxas de publicação.

## Criar Linked Wearables

## Criar uma Linked Wearable Collection

Criar uma Linked Wearable Collection é o primeiro passo para criar os nossos Linked Wearables.

Os Linked Wearables são agrupados em coleções que podem ser criadas, editadas e eliminadas pelos seus proprietários. Cada coleção pode conter um número arbitrário de Linked Wearables. Cada coleção será associada a uma coleção NFT; por essa razão, **é necessário um contrato NFT (compatível com ERC721 ou ERC1155) para criar uma Linked Wearable Collection.**

Para criar um novo Linked Wearable, siga estes passos:

1. Na secção da Collection, clique no **Criar coleção** botão.

   ![](/docs/_remote/4e2aa2f187322d76f74c1b4c.webp)
2. Selecione a opção Linked Wearable Collection clicando no **Criar coleção** botão na secção Linked Collections.

   ![](/docs/_remote/23dbdab92c38a595e6d609ae.webp)
3. Escolha um nome para a coleção e associe a coleção à sua coleção NFT definindo o endereço do contrato e a rede em que ela está. **O contrato será validado para garantir que cumpre os padrões dos contratos NFT.**

   ![](/docs/_remote/8365f405886986e6be3328d6.webp)
4. Clique no **Criar** botão para criar a coleção.

## Adicionar wearables à Linked Wearables Collection - Um a um

É possível, tal como já acontece com os wearables standard, carregar os modelos 3D dos seus wearables um a um.

Para tal, siga estes passos:

1. Clique no **Novos itens** botão.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. Selecione a **Itens individuais** opção.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. Siga os passos para carregar e configurar o seu wearable tal como é descrito nas [diretrizes de criação de wearables](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/) e configure como será ligado aos seus NFTs. Consulte a "[Como é que os Linked Wearables representam NFTs?](#how-do-linked-wearables-represent-nfts)" secção para mais informações sobre como configurá-lo.

   ![](/docs/_remote/3f230f70eb074d1a7d79fecf.webp)

## Adicionar wearables à Linked Wearables Collection - Em lote

Como as coleções Linked Wearable podem conter um grande número de itens, é possível carregar os modelos 3D e as informações dos wearables em lote. Este processo envolve criar um ficheiro zip com todos os assets de que um item precisa para cada um dos itens. **Carregar wearables em lote é recomendado apenas para coleções programáticas**.

### Construir o ZIP do wearable

Cada item exigirá a criação de um ficheiro ZIP incluindo os seguintes assets:

* Os **ficheiros** necessários do modelo 3D do wearable (GLB, GLTFs, ficheiros de textura, etc.).
* Um **ficheiros** `ficheiro wearable.json` que contém as informações do wearable.
* Um **opcional** `ficheiro thumbnail.png` que contém a miniatura do wearable que será vista no Builder e no mundo. Se não for fornecido, será gerada uma usando o modelo 3D.

Os modelos 3D e o opcional `ficheiro thumbnail.png` seguem a [secção Custom Thumbnails](/docs/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-wearables/#custom-thumbnails) no artigo Uploading Wearables sobre como criar uma miniatura personalizada.

Os `ficheiro wearable.json` que acompanha o conteúdo dos wearables tem o seguinte formato (escrito como seria em TypeScript):

```typescript
type WearableConfiguration = {
  /** O URN do wearable */
  id?: string
  /** Nome do wearable */
  name: string
  /** Descrição do wearable */
  description?: string
  data: {
    /** Wearables a substituir ao equipar o wearable */
    replaces: WearableCategory[]
    /** Wearables a ocultar ao equipar o wearable */
    hides: WearableCategory[]
    /** Tags para identificar o wearable */
    tags: string[]
    /** Representações do wearable */
    representations: WearableRepresentation[]
    /** Categoria do wearable */
    category: WearableCategory
  },
  /** A definição de mapeamento do wearable com NFTs */
  mapping: Mapping
}

type WearableRepresentation = {
  /** body shape da representação */
  bodyShapes: BodyShape[];
  /** Caminho do ficheiro para o ficheiro principal da representação (GLB, GLTF, etc.) */
  mainFile: string;
  /** Uma lista dos caminhos dos ficheiros que pertencem à representação */
  contents: string[];
  /** Wearables a ocultar ao equipar esta representação */
  overrideHides: WearableCategory[];
  /** Wearables a substituir ao equipar esta representação */
  overrideReplaces: WearableCategory[];
}

enum WearableCategory = {
  EYEBROWS = 'eyebrows',
  EYES = 'eyes',
  FACIAL_HAIR = 'facial_hair',
  HAIR = 'hair',
  HEAD = 'head',
  BODY_SHAPE = 'body_shape',
  MOUTH = 'mouth',
  UPPER_BODY = 'upper_body',
  LOWER_BODY = 'lower_body',
  FEET = 'feet',
  EARRING = 'earring',
  EYEWEAR = 'eyewear',
  HAT = 'hat',
  HELMET = 'helmet',
  MASK = 'mask',
  TIARA = 'tiara',
  TOP_HEAD = 'top_head',
  SKIN = 'skin'
}

enum WearableBodyShape {
  MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale',
  FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'
}

type Mapping = SingleMapping | AnyMapping | RangeMapping | MultipleMapping

enum MappingType {
  SINGLE = 'single',
  ANY = 'any',
  MULTIPLE = 'multiple',
  RANGE = 'range'
}

type SingleMapping = {
  type: MappingType.SINGLE
  id: string
}

type AnyMapping = {
  type: MappingType.ANY
}

type RangeMapping = {
  type: MappingType.RANGE
  from: string
  to: string
}

type MultipleMapping = {
  type: MappingType.MULTIPLE
  ids: string[]
}
```

Segue-se um exemplo de um `ficheiro wearable.json` ficheiro que contém um modelo para cada body shape:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "Special hat",
  "description": "Uma descrição do wearable",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat",
    "mapping": {
      "type": "single",
      "id": "123"
    }
  }
}
```

Este ficheiro será compactado em ZIP juntamente com o `aMaleModelFile.glb`, `aTextureFile.png`, `aFemaleModelFile.glb` e `anotherTextureFile.png`.

Para adicionar uma miniatura personalizada ao wearable, pode adicionar um `ficheiro thumbnail.png` ficheiro.

Alguns aspetos a considerar sobre o `ficheiro wearable.json` ficheiro:

* Todas as informações sobre as categorias de wearables e qual escolher podem ser encontradas na [diretrizes de criação de wearables](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/).
* O array representations conterá a informação sobre como cada body shape irá parecer. Cada wearable DEVE conter pelo menos uma representação (pode ter uma ou ambas), isto é, tendo em consideração os body shapes que temos atualmente, seja `urn:decentraland:off-chain:base-avatars:BaseMale` ou `urn:decentraland:off-chain:base-avatars:BaseFemale`. Cada representação descreverá quais modelos serão usados para cada body shape.
* O objeto mapping deve ser configurado como um dos mecanismos disponíveis para ligar o seu NFT ao seu wearable, seguindo a "[Como é que os Linked Wearables representam NFTs?](#how-do-linked-wearables-represent-nfts)" secção.

**Definir um ID ou URN personalizado para os itens**

Os `id` campo é opcional e pode ser usado para criar um wearable com um ID específico para ser atualizado no futuro em lote (o que é explicado mais à frente na [secção de edição de wearables em lote](#editing-wearables-in-bulk) ).

No caso de o `id` campo ser usado, deve conter o ID completo do wearable. O ID (ou URN) do wearable é escrito como `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id:item-id`. Onde, `urn:decentraland:matic:collections-thirdparty:third-party-id:collection-id` pode ser obtido na página da coleção e o `item-id` é o identificador personalizado do item que gostaria de usar.

Os IDs ou URNs seguem um formato específico; aceitam:

* Caracteres em minúsculas, de `a` até `z`.
* Todos os números.
* Não podem conter outro tipo de caracteres nem espaços em branco. Sugerimos que substitua os espaços em branco pelo `-`

Pode obter o ID (ou URN) na página da coleção seguindo os passos seguintes:

1. Entrando na vista da coleção da qual quer copiar o URN e clicando na opção Edit URN no menu de opções:

![](/docs/_remote/72142577aacca52f35921c2a.webp)

2. Copiando o identificador que está por baixo do campo de texto:

![](/docs/_remote/8d7643cea8e5a23b82de3fbb.webp)

Por exemplo, se o URN ou ID obtido na UI para a coleção for `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection` e estiver a identificar os seus wearables numericamente, o URN do exemplo seria `urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1`, sendo `1` o número do wearable.

### O processo de carregamento

Quando todos os ficheiros estiverem prontos, para carregar os wearables em lote, siga estes passos:

1. Clique no **Novos itens** botão.

   ![](/docs/_remote/e3843eed8d736f40c93c6f2e.webp)
2. Selecione a **Vários itens** opção.

   ![](/docs/_remote/444a582037b01c922c963b76.webp)
3. Clique no **Navegue no seu computador** ligação para abrir o seu gestor de ficheiros e selecionar todos os zips que contêm os seus wearables.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. Revise se todos os ficheiros estão corretos ou se precisam de ser corrigidos. Neste caso, o modelo do wearable não está definido ou o `ficheiro wearable.json` ficheiro tem uma representação configurada incorretamente.

   ![](/docs/_remote/9af2b80ef49b3ab34f730589.webp)
5. Corrija quaisquer erros clicando no **Adicionar mais** botão e recarregando os ficheiros com falha com o mesmo nome ou descartando os erros usando o ícone do caixote do lixo na secção superior direita do modal.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. Carregue todos os wearables clicando em **Carregar itens**.

   ![](/docs/_remote/56c36770612421c06d0c4eb5.webp)
7. Tenha paciência, isto pode demorar algum tempo!

   ![](/docs/_remote/2418ecc5ff115dc5b7837888.webp)
8. Sucesso! Os seus itens estão agora disponíveis na sua coleção.

   ![](/docs/_remote/e983b87c56423a0df39dd0f5.webp)
9. Selecione se a sua coleção é programática ou standard. Consulte a [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) secção para definir corretamente para que tipo de coleção está a criar itens.

   ![](/docs/_remote/e70222ca6d6afc784bbdbab3.webp)

### Erros comuns ao carregar itens em lote

* Os `id` campo está definido com um valor que já está a ser usado por outro wearable.
* Os `id` campo está definido com um valor que não é um ID válido. Por exemplo, o third party id ou collection id pertencem a outro third party ou coleção.
* Não existe nenhum `ficheiro wearable.json` ficheiro no zip.
* O ficheiro ZIP não tem no seu diretório raiz o `ficheiro wearable.json` ficheiro.
* Os `ficheiro wearable.json` tem um formato ou valores incorretos.
* O ficheiro é maior do que 3 MB. Linked Wearables têm a mesma limitação que os wearables regulares em termos de tamanho, tal como os standard.
* A imagem de miniatura personalizada opcional não é um ficheiro png.

## Ver os wearables em Decentraland

Os Linked Wearables podem ser vistos no mundo para rever como o modelo funcionará depois de publicado e aprovado.

Para conseguir ver um wearable no mundo, siga estes passos:

1. Clique no menu de três pontos no lado direito do item que quer ver no mundo. Surgirá um dropdown. Selecione **Ver no Decentraland**.

   ![](/docs/_remote/c816f508064933bb206516a8.webp)
2. O Decentraland World será aberto. Navegue até à sua mochila para ver o wearable.

   ![](/docs/_remote/e27530d66df008ecb1568921.webp)


> [!WARNING]
> **⚠️ Aviso**: A pré-visualização no mundo funciona com a versão Web desatualizada do Decentraland Client. Ainda não é possível testá-los no Decentraland Desktop Client 2.0.


## Editar Linked Wearables

## Editar o nome da coleção

Uma coleção pode ser renomeada pelo seu criador **apenas se a coleção não tiver wearables publicados**.

Para editar o nome de uma Linked Wearable Collection, siga estes passos:

1. Clique no nome da coleção.

   ![](/docs/_remote/39383f19e2bc56803e0a516b.webp)
2. Escolha um novo nome para a coleção e clique no botão guardar.

   ![](/docs/_remote/30585b27d6df4a4b988c3e13.webp)

## Eliminar a coleção

Uma coleção pode ser eliminada pelo seu proprietário **apenas se a coleção não tiver wearables publicados**.

Para eliminar uma Linked Wearable Collection, siga estes passos:

1. Clique no menu de três pontos no extremo direito do conjunto de botões. Surgirá um dropdown. Selecione **Eliminar**.

   ![](/docs/_remote/72142577aacca52f35921c2a.webp)
2. Será apresentado um modal de confirmação; se desejar prosseguir, clique em **Ok**, caso contrário clique em **Cancelar**.

   ![](/docs/_remote/7188871c151f9edf2199a42e.webp)

## Editar um wearable individual

### Editar propriedades do wearable

Para editar um único wearable, siga estes passos:

1. Clique no menu de três pontos no lado direito do item que quer ver no mundo. Surgirá um dropdown. Selecione **Abrir no editor**.

   ![](/docs/_remote/abfed6b24f3e8e71867a0391.webp)
2. Edite o wearable da mesma forma que os wearables standard são editados. Siga a **Editar itens** secção em [diretrizes de criação de wearables](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/) sobre como criar uma miniatura personalizada.

### Editando a vinculação do wearable

A vinculação dos wearables com a coleção de NFT é uma das propriedades mais importantes de um Linked Wearable. Para editar como eles estão vinculados aos NFTs, você pode alterar rapidamente o valor de vinculação na visualização da coleção, sem precisar navegar para outra página.

![](/docs/_remote/b9bb316d91d55debb38713cb.webp)

Confira a "[Como é que os Linked Wearables representam NFTs?](#how-do-linked-wearables-represent-nfts)" seção para mais informações sobre como vincular seus wearables.

## secção de edição de wearables em lote

Seguindo a mesma ideia vista anteriormente na [Criando wearables em lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) seção, administradores de terceiros podem fazer alterações nos wearables em lote.

Para fazer alterações em lote nos wearables, é necessário criar um arquivo ZIP para cada um dos wearables que serão alterados.

Esses arquivos ZIP devem ser criados seguindo o formato descrito em [Criando wearables em lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) com uma exceção, no `ficheiro wearable.json` arquivo, a `id` propriedade **DEVE** ser definida como o `id` ou URN do wearable que será alterado. Isso é obrigatório, pois é a única maneira de identificar o wearable a ser alterado. Se você criou seus wearables em lote fornecendo um id no `ficheiro wearable.json` arquivo, você pode reutilizar seus arquivos `ficheiro wearable.json` para atualizá-los.

Levando em consideração o exemplo na [Criando wearables em lote](#adding-wearables-to-the-linked-wearables-collection-in-bulk) seção, se quisermos alterar algumas das propriedades de um wearable, por exemplo, o nome ao qual esquecemos de adicionar um número, devemos incluir um `ficheiro wearable.json` arquivo no zip, como no exemplo a seguir:

```json
{
  "id": "urn:decentraland:matic:collections-thirdparty:my-third-party:my-collection:1",
  "name": "Um chapéu 1",
  "description": "Uma descrição do wearable",
  "data": {
    "replaces": [],
    "hides": ["hair"],
    "tags": ["special", "new", "hat"],
    "representations": [
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
        "mainFile": "aMaleModelFile.glb",
        "contents": ["aMaleModelFile.glb", "aTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      },
      {
        "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "mainFile": "aFemaleModelFile.glb",
        "contents": ["aFemaleModelFile.glb", "anotherTextureFile.png"],
        "overrideHides": [],
        "overrideReplaces": []
      }
    ],
    "category": "hat"
  },
  "mapping": {
    "type": "single",
    "id": "123"
  }
}
```

Onde o `id` campo é definido como o `id` ou URN do wearable que será alterado e o `nome` campo é definido com o novo nome do wearable.

Quando os arquivos ZIP estiverem prontos, siga estas etapas para editar os itens em lote:

1. Clique no menu de três pontos no extremo direito do conjunto de botões. Surgirá um dropdown. Selecione **Editar em lote**.

   ![](/docs/_remote/0c8a87dd214ea40c8a6ed252.webp)
2. Um modal semelhante ao da **Fazendo upload de modelos em lote** será exibido. Clique no **Procurar no seu computador** link para abrir o gerenciador de arquivos.

   ![](/docs/_remote/a39bb531aa4d0dbae170bd92.webp)
3. Selecione todos os arquivos ZIP dos itens que serão editados.

   ![](/docs/_remote/d5fe0416bee0c4f5d15fe1ac.webp)
4. Revise se todos os ficheiros estão corretos ou se precisam de ser corrigidos. Neste caso, o modelo do wearable não está definido ou o `ficheiro wearable.json` ficheiro tem uma representação configurada incorretamente.

   ![](/docs/_remote/4a7baded9874d3cc155f2fbd.webp)
5. Corrija quaisquer erros clicando no **Adicionar mais** botão e recarregando os ficheiros com falha com o mesmo nome ou descartando os erros usando o ícone do caixote do lixo na secção superior direita do modal.

   ![](/docs/_remote/bebf438cdc984a5547355b9e.webp)
6. Carregue todos os wearables clicando em **Carregar itens**.

   ![](/docs/_remote/42f683d2a6f504f05485aec8.webp)
7. Tenha paciência, isto pode demorar algum tempo!

   ![](/docs/_remote/5894f0726ee565293b5d41a2.webp)
8. Sucesso! Seus itens agora estão disponíveis na sua coleção. Consulte a postagem do fórum para ver quaisquer atualizações do curador.

   ![](/docs/_remote/fb076a01ef98b4db599d68cf.webp)

## Publicando seus Linked Wearables

Seus Linked Wearables precisam passar por um processo de publicação e curadoria, assim como os wearables regulares. Embora o processo de publicação e curadoria não seja o mesmo dos wearables regulares, ele mantém as mesmas etapas; todos os itens precisam primeiro ser publicados para depois serem curados por um curador designado.

As seções a seguir mostrarão como publicar seus Linked Wearables para serem curados.

## Custos

Criar Linked Wearables tem um custo, dependendo do tipo de Linked Wearable Collection que você escolheu criar:

1. **Padrão**: cada wearable publicado custa o mesmo que um wearable regular: USD 100 pagáveis em MANA.
2. **Programático**: uma taxa fixa, paga uma única vez para todos os wearables que você publicará. Essa taxa equivale à publicação de 20 wearables regulares: 2000 USD pagáveis em MANA.

Para mais informações sobre o tipo de coleção que você está criando, confira a [NFT Collections & Linked Wearables Collections](#types-of-linked-wearables-collections) seção.

## Publicando wearables para revisão

Quando seus wearables estiverem prontos, eles devem ser publicados para curadoria. Seus wearables são publicados em grupos de itens; você pode escolher quais itens estão prontos para serem curados selecionando-os e clicando no `Publicar` botão. Depois de publicar os itens, a publicação ficará bloqueada até que os itens que já estão publicados sejam curados.

Para publicar seus wearables, você precisa:

1. Selecione os itens a serem publicados clicando na caixa de seleção ao lado deles. Clique no **Publicar** botão quando sua seleção estiver pronta.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
2. Confirme o nome da sua coleção. Depois de publicar seus itens, não será possível alterar o nome da coleção, então certifique-se de verificá-lo cuidadosamente.

   ![](/docs/_remote/52a58c4e5c9954ebc8fc269b.webp)
3. Dê uma olhada nos itens que você está publicando. Clique no **Confirmar itens** botão quando estiver pronto.

   ![](/docs/_remote/8a8e23ad31bfa01cd5132c54.webp)
4. Assine a confirmação de publicação dos seus itens na sua Wallet.

   ![](/docs/_remote/88c6a6a00343c5fac618b93d.webp)
5. Leia e marque os Termos e Condições.

   ![](/docs/_remote/5eeaa4e5bf1e5c04ddcfe174.webp)
6. Confira suas taxas de publicação. As taxas necessárias para publicar os wearables estão descritas em [secção de Custos](#costs).

   ![](/docs/_remote/763b602c672547a4d2a6101b.webp)
7. Se esta for a sua primeira vez publicando Linked Wearables, você precisará autorizar o smart contract de Linked Wearables a operar MANA em seu nome. Esta etapa é necessária para deduzir o MANA usado para pagar as taxas de publicação da sua Wallet.

   ![](/docs/_remote/86176e8cb7ef6bc5bbe8c139.webp)
8. Pague a taxa de publicação e conclua a publicação realizando a transação. Dependendo do congestionamento da rede, isso pode levar algum tempo.

   ![](/docs/_remote/bdbd4028179d589523236310.webp)
9. Sucesso! Você publicou seus itens. Seus itens passarão pelo processo de curadoria como as coleções regulares fazem. Você pode se comunicar com o curador por meio da postagem no fórum.

   ![](/docs/_remote/1c4b8bde3d0569bee971bd77.webp)

## Enviando alterações para revisão

Wearables publicados e aprovados que forem editados precisam passar pelo processo de curadoria novamente. Não se preocupe, nenhuma taxa será aplicada a wearables já publicados.

Para enviar alterações para que sejam curadas, você precisa:

1. Selecione os itens com alterações clicando na caixa de seleção ao lado deles. Clique no **Enviar alterações** botão quando sua seleção estiver pronta.

   ![](/docs/_remote/2cb347394e74593c7becf04c.webp)
2. Prossiga com o processo de envio de alterações. Como o modal informa, essas alterações precisarão passar pela fase de curadoria novamente.

   ![](/docs/_remote/3a60e8ad8fb155002a8e4cef.webp)
3. Leia e marque os Termos e Condições. Ao aceitá-los, os itens estarão prontos para serem curados novamente.

   ![](/docs/_remote/14b970d046bc0d27c4efbb13.webp)

## Curadoria

Assim como os wearables regulares, seus modelos 3D precisarão da aprovação do Curators Committee. Você não está isento dessa regra, pois a estética e a jogabilidade de Decentraland ainda precisam ser protegidas.

O processo de curadoria será diferente de acordo com o processo usado para gerar os wearables. As coleções de Linked Wearables admitem wearables artesanais e gerados programaticamente.

### wearables artesanais

Para modelos 3D que foram feitos individualmente sem nenhum processo automatizado (o método usual para a maioria dos wearables regulares), o Curator precisará analisar todos os itens da coleção individualmente para garantir que todos estejam em conformidade com as [Wearable Guidelines](/docs/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables/).

### Coleções programáticas

Para coleções programáticas, nem todos os itens precisam ser curados individualmente. O número de itens a serem curados em cada coleção depende do tamanho da coleção; isso foi definido pela DAO em [esta proposta](https://governance.decentraland.org/proposal/?id=f69c4d40-aaaf-11ec-87a7-6d2a41508231).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/wearables/linked-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
