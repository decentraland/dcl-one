# Entities

Cada peça individual de conteúdo no mundo (como uma scene ou um wearable) é chamada de uma Entity *entity*.

Entities são pacotes imutáveis de [files](/docs/contributor/contributor-pt/conteudo/file-system/) com um identificador de string único, derivado deterministamente dos dados contidos, que pode ser usado para descobrir e baixar os arquivos relacionados do servidor de conteúdo.

O arquivo principal de uma Entity é o *manifest*, um documento JSON que descreve as propriedades gerais da Entity, bem como atributos especiais para cada [tipo](#types). O identificador de uma Entity é, na verdade, o [identificador do arquivo](/docs/contributor/contributor-pt/conteudo/file-system/#identifiers) deste manifesto.

Como são imutáveis, as Entities não podem ser atualizadas no sentido tradicional. Em vez disso, elas são substituídas por novas Entities descobríveis usando o mesmo [pointer](/docs/contributor/contributor-pt/conteudo/pointers/). A versão mais recente de uma Entity é dita estar *ativa*.

Cada Entity é assinada por um proprietário (associado a uma conta Ethereum). O proprietário pode depois usar as mesmas chaves de assinatura para carregar uma nova versão da Entity e indicar que ela substitui a antiga. Os servidores de conteúdo validam essas assinaturas antes de aceitar novas Entities, sejam elas provenientes diretamente de um cliente ou encaminhadas por outro servidor.

Você pode ver Entities efetivamente implantadas em [secção de prática](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) secção.

### Entity Types <a href="#types" id="types"></a>

Há sete tipos de Entities:

* [**Scenes**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/): espaços virtuais no mundo com seus próprios objetos e comportamento.
* [**Profiles**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/): informações sobre um jogador específico, como seu nome e avatar.
* [**Wearables**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/wearables/): roupas e itens que os jogadores podem adicionar aos seus avatares.
* [**Emotes**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/emotes/): animações que o avatar de um jogador pode executar.
* [**Stores**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/stores/): sites de Marketplace para Wearables e Emotes que os jogadores podem comprar.
* [**Outfits**](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/outfits/): outfits salvos para um jogador específico.

Todos os tipos seguem os mesmos procedimentos para criação, identificação, propriedade e hospedagem.

### Propriedades Comuns <a href="#properties" id="properties"></a>

Toda Entity tem certas propriedades comuns em seu manifesto, aplicáveis a todos os tipos. Esses campos de nível superior estarão sempre presentes:

| Campo       | Valor                                                                                                                           |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `tipo`      | Um de `scene`, `perfil`, `wearable`, `emote`, `store` ou `outfits`.                                                             |
| `pointers`  | Um array de [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) associado a esta Entity.                               |
| `timestamp` | O timestamp Unix UTC em que esta Entity foi carregada.                                                                          |
| `content`   | Um array de referências a arquivos adicionais [files](/docs/contributor/contributor-pt/conteudo/file-system/) no pacote da Entity. |
| `metadata`  | Um objeto com informações específicas para este tipo de Entity.                                                                 |

A estrutura e os valores do `metadata` campo para cada tipo são detalhados em suas páginas específicas. O `pointers` array também tem conteúdos diferentes dependendo do tipo.


> [!NOTE]
> Manifestos antigos de Entities podem conter o `version` campo, descontinuado em [ADR-45](https://adr.decentraland.org/adr/ADR-45). Você pode ignorá-lo com segurança, já que o `timestamp` campo agora é usado para versionamento.


Este é um manifesto JSON típico que descreve uma Entity:

```json
{
  "type": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0xbdf21eaf54ebf4a6cadc2dcb371df7afce98bc1d:0"],
  "timestamp": 1628181913506,
  "content": [
    // ...referências de arquivos, veja abaixo
  ],
  "metadata": {
    // ...campos específicos para este tipo de Entity, veja a página relevante
  }
}
```

Você pode encontrar os schemas dessas estruturas JSON, junto com outros objetos no protocolo Decentraland, no [Common Schemas](https://github.com/decentraland/common-schemas) repositório.


> [!NOTE]
> Ao analisar manifestos de Entities, você pode encontrar campos não documentados. Isso ocorre porque o schema de Entity permite propriedades personalizadas adicionais, definidas livremente pelo proprietário.


### Ficheiros <a href="#files" id="files"></a>

Como mencionado acima, todas as Entities têm pelo menos um arquivo associado: o manifesto JSON que descreve a própria Entity. O identificador da Entity é, na verdade, o [identificador do arquivo](/docs/contributor/contributor-pt/conteudo/file-system/#identifiers) deste arquivo especial.

O `content` campo dentro de cada manifesto é um array de referências a arquivos adicionais. Estes geralmente são assets, como modelos 3D e animações, ou scripts para Scenes.

Todos os arquivos são armazenados no [sistema de arquivos distribuído](/docs/contributor/contributor-pt/conteudo/file-system/), e cada item no array tem duas propriedades:

| Campo  | Valor                                                                                                                              |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| `file` | O nome interno usado pelos arquivos nesta Entity para se referirem uns aos outros.                                                 |
| `hash` | O [identificador global deste arquivo](/docs/contributor/contributor-pt/conteudo/file-system/#identifiers), único em todo o conteúdo. |

É assim que normalmente aparece dentro do `content` campo:

```json
[
  {
    "file": "thumbnail.png",
    "hash": "bafkreiglecvpnqibvf6pltcnid5nhbx3caj77lu4ia4xitpmp3lrcouuhm",
  },
  {
    "file": "model.glb",
    "hash": "bafkreic2i3awiu7srhatf3k47l3c5lmadisznjigppor2a35saosjfbo25",
  },
  // ...mais arquivos
]
```


> [!NOTE]
> O `file` O valor do campo está sempre em minúsculas, para evitar problemas ao construir Entities em diferentes sistemas operacionais, onde a capitalização do nome do arquivo pode ser importante.


A vida útil de um arquivo está ligada à Entity que o contém. Para Entities ativas (ou seja, ainda não substituídas pelo seu proprietário), os servidores de conteúdo são obrigados pelo protocolo a preservar todos os arquivos associados. Se a Entity for excluída, os arquivos podem ser mantidos ou descartados a critério do servidor.

### Propriedade e Autenticação <a href="#ownership" id="ownership"></a>

Para provar a propriedade e autorizar ações em torno de Entities, o [cadeia de autenticação](/docs/contributor/contributor-pt/autenticacao/authchain/) mecanismo é usado.

O [`decentraland-crypto`](https://github.com/decentraland/decentraland-crypto) repositório contém a implementação de todos os procedimentos criptográficos.

### Descobrindo e Baixando Entities

Os servidores de conteúdo podem ser usados para localizar Entities usando [pointers](/docs/contributor/contributor-pt/conteudo/pointers/), e para baixar seus manifestos e quaisquer arquivos adicionais.

* Para resolver um pointer em um ID de Entity, você pode usar o [`/entities/active`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getListOfEntities) endpoint.
* Usando o ID da Entity, você pode baixar o manifesto com o [`/contents/<id>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) endpoint.
* Para obter todas as Entities ativas de um determinado tipo, comece baixando um [snapshot](/docs/contributor/contributor-pt/conteudo/snapshots/).

Confira a [secção de prática](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) seção para exemplos e guias.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
