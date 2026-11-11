# Wearables

Wearables são os [entidades](/docs/contributor/contributor-pt/conteudo/entities/) objetos que contêm itens para os jogadores vestirem e personalizarem seus avatares.

O `wearables` campo no avatar de um jogador [perfil](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/) contém uma lista de ponteiros para os itens que estão vestindo no momento.

Todo Wearable pertence a uma *categoria*, indicando o espaço que ocupa no corpo do avatar e pode ocultar ou substituir outras categorias (por exemplo, um vestido longo pode ser usado como roupa superior, mas cobrir completamente as pernas).

Há 18 categorias de Wearable, referenciadas em vários campos de metadados: `body_shape`, `hair`, `eyes`, `eyebrows`, `facial_hair`, `mouth`, `upper_body`, `lower_body`, `feet`, `hat`, `helmet`, `mask`, `tiara`, `top_head`, `earring`, `eyewear`, `skin` e `hands_wear`.

As entidades Wearable incluem arquivos no formato GLB para diferentes formas de corpo.

### Ponteiros <a href="#pointers" id="pointers"></a>

Wearables têm exatamente um [pointer](/docs/contributor/contributor-pt/conteudo/pointers/), que indica a [collection](/docs/contributor/contributor-pt/conteudo/collections/) à qual pertencem e o seu identificador dentro dela.

Quando o wearable é um ativo on-chain, o ponteiro é um URN desta forma:

```
urn:decentraland:<blockchain>:collections-v2:<collection address>:<item index>
```

Para ilustrar, o `pointers` array para um wearable on-chain é assim:

```json
{
  "pointers": [
    "urn:decentraland:matic:collections-v2:0x2d9560df9dd8ba8b2dc3746bc1d217698d258fb5:0"
  ],
  // ... outras propriedades da entity
}
```

Quando o wearable é um ativo off-chain, como os wearables padrão para novos avatares, ele terá esta forma:

```
urn:decentraland:off-chain:<collection>:<item name>
```

A coleção, neste caso, é um nome, não um endereço, e o identificador do item também é um nome, em vez de um índice.

O `pointers` o array para um wearable off-chain tem este aspecto:

```json
{
  "pointers": [
    "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  // ... outras propriedades da entity
}
```

### Campos de Metadata

Wearables compartilham a maior parte dos seus campos básicos com [emotes](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/emotes/). O `data` property é onde as informações específicas do wearable estão localizadas.

| Campo               | Valor                                                                                                                  |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `id`                | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) que resolve (ou resolvia) para este wearable.            |
| `name`              | O título de exibição deste wearable em um [collection](/docs/contributor/contributor-pt/conteudo/collections/).           |
| `description`       | Uma descrição estendida para este wearable.                                                                            |
| `imagem`            | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) de uma imagem deste wearable.      |
| `miniatura`         | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) de uma versão 256x256 da `imagem`. |
| `raridade`          | Um de `comum`, `incomum`, `raro`, `épico`, `lendário`, `exótico`, `mítico` ou `único`.                                 |
| `i18n`              | Um array de traduções para o `name` campo.                                                                             |
| `collectionAddress` | O endereço Ethereum da coleção que contém este wearable.                                                               |
| `metrics`           | Algumas medidas úteis sobre as animações (veja abaixo).                                                                |
| `data`              | Os metadados estendidos para este wearable (veja abaixo).                                                              |

Na forma JSON típica:

```json
{
  "id": "urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632b4e7226816a16139fb33:0",
  "name": "Vestido Bonito",
  "description": "Muito agradável de usar",
  "image": "image.png",
  "thumbnail": "thumbnail.png",
  "rarity": "incomum",
  "i18n": [
    { "code": "es", "text": "Vestido Bonito" }
  ],
  "collectionAddress": "0x11a6879861f36cbad632b4e7226816a16139fb33",
  "metrics": {
    // Objeto de medidas (veja abaixo).
  },
  "data": {
    // Metadados estendidos (veja abaixo).
  }
}
```

### Métricas

Na `metadata.metrics` object, você encontrará algumas medidas simples para os modelos empacotados com este wearable. Um exemplo:

```json
{
  "entities": 1,
  "bodies": 12,
  "materials": 2,
  "meshes": 12,
  "textures": 2,
  "triangles": 3321
}
```

### Dados

O `metadata.data` object é onde as propriedades específicas do wearable estão localizadas.

| Campo            | Valor                                                                       |
| ---------------- | --------------------------------------------------------------------------- |
| `categoria`      | Uma das categorias de wearable listadas acima.                              |
| `Representações` | Um array de arquivos associados a diferentes formas de corpo (veja abaixo). |
| `oculta`         | Um array de categorias que este wearable oculta.                            |
| `substitui`      | Um array de categorias que este wearable substitui por completo.            |
| `tags`           | Um array de rótulos de string descritivos deste wearable.                   |

#### Representações

Cada item no array de representações associa uma forma de corpo a uma coleção de modelos e pode adicionar regras sobre quais wearables são exibidos em cada categoria.

| Campo              | Valor                                                                                                                                                                |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bodyShapes`       | Um array de [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) a entidades de forma de corpo.                                                              |
| `mainFile`         | O arquivo do modelo 3D para iniciar a renderização.                                                                                                                  |
| `contents`         | Um array de nomes de arquivos (presentes no [`content` campo](/docs/contributor/contributor-pt/conteudo/entities/#properties)de nível superior) usados pelo `mainFile`. |
| `overrideHides`    | Substitui a `oculta` lista acima para esta representação.                                                                                                            |
| `overrideReplaces` | Substitui a `substitui` lista acima para esta representação.                                                                                                         |

Este é um `Representações` item típico:

```json
{
  "bodyShapes": [
      "urn:decentraland:off-chain:base-avatars:BaseFemale"
  ],
  "mainFile": "male/190.glb",
  "contents": [
      "male/190.glb"
  ],
  "overrideHides": [ "lower_body" ],
  "overrideReplaces": []
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
