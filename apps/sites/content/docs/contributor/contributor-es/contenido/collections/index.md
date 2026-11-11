# Colecciones

Las colecciones son grupos de [wearables](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/) y [emotes](/docs/contributor/contributor-es/contenido/tipos-de-entidad/emotes/) definidos en el mismo namespace off-chain o contrato on-chain.

No son [entities](/docs/contributor/contributor-es/contenido/entities/) en sí mismas, pero sus items individuales pueden obtenerse de los servers de contenido normalmente usando [pointers](/docs/contributor/contributor-es/contenido/pointers/).

### URNs y Pointers <a href="#pointers" id="pointers"></a>

Las colecciones se identifican usando URNs, que son prefijos de sus URNs de items. Hay 4 tipos generales de URNs:

* `v1` colecciones con contenido aprobado por Foundation (deprecated).
* `v2` colecciones con contenido aprobado por la comunidad.
* `thirdparty` colecciones con items asociados a NFTs fuera de Decentraland.
* `off-chain` colecciones, que contienen principalmente items predeterminados para nuevos avatars.


> [!NOTE]
> A pesar de que `v1` las colecciones están deprecated y ya no se crearán ni actualizarán, siguen siendo válidas y sus items pueden usarse.


#### Colecciones de versión 1 y 2

Tanto el antiguo `v1` como el actual `v2` namespace de collections tienen URNs con la misma forma:

```
urn:decentraland:<blockchain>:<collections-version>:<contract-address>
```

Por ejemplo:

```
# Una colección en el espacio v1 obsoleto:
urn:decentraland:mainnet:collections-v1:DCL Test Masks

# Una colección en el espacio v2 aprobado por la comunidad:
urn:decentraland:matic:collections-v2:0x25a1d66891d44cdf7b8c45802489c1dea7aadf8b
```

Si el `:<id>` segmento se añade al final, la URN se convierte en un [pointer](/docs/contributor/contributor-es/contenido/pointers/) a un item dentro de la colección. Consulta [wearable pointers](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/#pointers) y [emote pointers](/docs/contributor/contributor-es/contenido/tipos-de-entidad/emotes/#pointers) para obtener información sobre estos.

#### Colecciones de terceros

Las colecciones de terceros, también conocidas como [linked wearables](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md), son diferentes de los wearables normales en que están vinculadas a NFTs existentes fuera de Decentraland. Permiten a los jugadores mostrar NFTs de su conjunto personal en sus avatars dentro del mundo.

Sus URNs tienen este aspecto:

```
urn:decentraland:<blockchain>:collections-thirdparty:<third-party-id>:<collection-id>
```

Puedes encontrar amplia información sobre estos en la [linked wearables](https://github.com/decentraland/docs/tree/main/creator/wearables-and-emotes/wearables/linked-wearables.md) documentación para creadores.

### Descubrir colecciones

Los servers de contenido pueden darte una lista de todas las colecciones conocidas, usando el [`/lambdas/collections`](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/getCollections) endpoint.

La respuesta será un objeto con una `collections` propiedad, que contiene un array de URNs y nombres. Por ejemplo:

```json
{
  "collections": [
    {
      "id": "urn:decentraland:amoy:collections-v2:0xff5d4ebc6bc1ff7262cab42d3c693d953f4614d2",
      "name": "Colección de ropa de invierno"
    },
    // ... muchas más
  ]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/collections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
