# Stores

As lojas são off-chain [entidades](/docs/contributor/contributor-pt/conteudo/entities/) que representam sites de marketplace geridos pelos usuários. Elas contêm uma descrição e uma imagem para mostrar aos compradores, além de informações sobre o proprietário da loja.

### Ponteiros <a href="#pointers" id="pointers"></a>

Os ponteiros das lojas são URNs no `off-chain` namespace, com a seguinte forma:

```
urn:decentraland:off-chain:marketplace-stores:<owner-address>
```

O segmento do endereço é o endereço Ethereum do proprietário da loja. Por exemplo:

```
urn:decentraland:off-chain:marketplace-stores:0xa2a1dc503be6fdb7878f58f053ded40564e3b9b2
```

### Campos de Metadata

| Campo         | Valor                                                                                                             |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| `id`          | O [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) que resolve (ou costumava resolver) para esta loja. |
| `description` | O título de exibição desta loja.                                                                                  |
| `owner`       | O endereço Ethereum do proprietário.                                                                              |
| `imagens`     | Um array de `{ file, name }` objetos que referenciam imagens (veja abaixo).                                       |
| `links`       | Um array de `{ name, url }` objetos com links de redes sociais ou de sites (veja abaixo).                         |

#### Imagens

As lojas têm uma imagem para mostrar aos compradores no marketplace, cada uma com um nome que identifica a função. O array típico `imagens` tem um único elemento com o nome `cover`, assim:

```json
[
  {
    "name": "cover",
    "file": "imgs/store-logo.png"
  }
]
```

#### Links

Os proprietários podem adicionar links externos às lojas, como perfis de redes sociais e websites externos. URLs do Facebook e do Discord são comumente incluídas. Um exemplo:

```json
[
  {
    "name": "facebook",
    "url": "https://www.facebook.com/mydclstore"
  },
  {
    "name": "website",
    "url": "https://mydclstore.com"
  }
]
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/stores.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
