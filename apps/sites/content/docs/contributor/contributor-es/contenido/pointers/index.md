# Punteros

Los punteros son cadenas únicas, que no distinguen mayúsculas de minúsculas, que hacen referencia a una entidad activa [entity](/docs/contributor/contributor-es/contenido/entities/). Los servidores de contenido pueden resolver estos punteros para obtener el identificador de la entidad.

Recuerda que tanto [entities](/docs/contributor/contributor-es/contenido/entities/) como sus [archivos](/docs/contributor/contributor-es/contenido/file-system/) son inmutables en el sistema de contenido de Decentraland, y sus identificadores cambian cuando se suben versiones de reemplazo. Los punteros, por otro lado, son referencias estables que persisten a través de los reemplazos. Esto se logra redirigiendo automáticamente el puntero a una nueva entidad cuando el propietario sube un reemplazo.

Varios punteros pueden resolver al mismo [entity](/docs/contributor/contributor-es/contenido/entities/), como suele ocurrir con [escenas](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/).

Como puedes deducir, el uso más común de los punteros es obtener el ID de la versión activa (es decir, el reemplazo más reciente) de un [entity](/docs/contributor/contributor-es/contenido/entities/), para descargarlo.

### Tipos de punteros

Hay 4 tipos de punteros que los servidores de contenido pueden resolver en entidades, cada uno con su propia sintaxis:

* [Punteros de escena](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#pointers) son coordenadas de parcela, como `"0,0"`.
* [Punteros de perfil](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/#pointers) son la dirección de Ethereum del propietario.
* [Punteros de Wearables](/docs/contributor/contributor-es/contenido/tipos-de-entidad/wearables/#pointers) y [Punteros de emotes](/docs/contributor/contributor-es/contenido/tipos-de-entidad/emotes/#pointers) son, ya sea [collection](/docs/contributor/contributor-es/contenido/collections/) URNs de ítem o URNs fuera de la cadena.
* [Punteros de tienda](/docs/contributor/contributor-es/contenido/tipos-de-entidad/stores/#pointers) son URNs fuera de la cadena con la dirección de Ethereum del propietario.
* [Punteros de outfits](/docs/contributor/contributor-es/contenido/tipos-de-entidad/outfits/#pointers) son la dirección de Ethereum del propietario seguida de `:outfits`.

Puedes encontrar más detalles en sus secciones específicas.

### Resolución de punteros

Los servidores de contenido tienen un endpoint que puede resolver punteros en el manifest de su entidad activa, realizando una `GET` petición a:

```
https://<content-base-url>/entities/<entity-type>?pointer=<pointer>
```


> [!NOTE]
> Por razones históricas, la respuesta de este endpoint es un array que contiene un elemento.


Algunos ejemplos útiles, usando el servidor de contenido de Foundation:

```bash
# Un puntero de colección on-chain a un wearable:
curl "https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:matic:collections-v2:0x30517529cb5c16f686c6d0b48faae5d250d43005:0"

# Un puntero de colección off-chain a un activo predeterminado:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=urn:decentraland:off-chain:base-avatars:BaseFemale

# Un puntero de parcela:
curl https://peer.decentraland.org/content/entities/wearables/?pointer=0,0

# Un puntero de perfil:
curl https://peer.decentraland.org/content/entities/profiles/?pointer=0xe2c2b80ca5ad868f4b30fa83cca2bf12cc95b4fe
```

Ten en cuenta que, si ya conoces el ID de la entidad, puedes [descargar el manifest](/docs/contributor/contributor-es/contenido/file-system/#downloading) archivo directamente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/pointers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
