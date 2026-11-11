# Almacenes

Las tiendas están fuera de la cadena [entities](/docs/contributor/contributor-es/contenido/entities/) que representan sitios del Marketplace gestionados por usuarios. Contienen una descripción y una imagen para mostrar a los compradores, además de información sobre el propietario de la tienda.

### Punteros <a href="#pointers" id="pointers"></a>

Los punteros de tienda son URNs en el `off-chain` namespace, con la siguiente forma:

```
urn:decentraland:off-chain:marketplace-stores:<owner-address>
```

El segmento de dirección es la dirección de Ethereum del propietario de la tienda. Por ejemplo:

```
urn:decentraland:off-chain:marketplace-stores:0xa2a1dc503be6fdb7878f58f053ded40564e3b9b2
```

### Campos de Metadata

| Campo         | Valor                                                                                                                  |
| ------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `id`          | El [pointer](/docs/contributor/contributor-es/contenido/pointers/) que resuelve (o se usaba para resolver) a esta tienda. |
| `description` | El título visible de esta tienda.                                                                                      |
| `owner`       | La dirección de Ethereum del propietario.                                                                              |
| `images`      | Una matriz de `{ file, name }` objetos que hacen referencia a images (ver abajo).                                      |
| `links`       | Una matriz de `{ name, url }` objetos con enlaces a redes sociales o sitios web (ver abajo).                           |

#### Imágenes

Las tiendas tienen una imagen para mostrar a los compradores en el Marketplace, cada una con un nombre que identifica su función. El típico `images` array tiene un solo elemento con el nombre `cover`, así:

```json
[
  {
    "name": "cover",
    "file": "imgs/store-logo.png"
  }
]
```

#### Enlaces

Los propietarios pueden añadir enlaces externos a las tiendas, como perfiles de redes sociales y sitios web externos. Las URLs de Facebook y Discord se incluyen comúnmente. Un ejemplo:

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
GET https://docs.decentraland.org/contributor/contributor-es/contenido/tipos-de-entidad/stores.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
