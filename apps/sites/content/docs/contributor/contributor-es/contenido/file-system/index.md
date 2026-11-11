# Sistema de archivos

Detrás de las diferentes API de contenido en el protocolo de Decentraland hay una implementación de un sistema de archivos distribuido. Cada servidor de contenido mantiene una copia de todo el almacenamiento y sincroniza las actualizaciones que recibe con otras instancias.

Los archivos subidos al sistema son **inmutables**. Una vez desplegados, ni su identificador ni su contenido cambian. Solo pueden actualizarse en el sentido de que se sube una nueva versión y las aplicaciones eligen usarla, mientras que la anterior puede descartarse.

Puedes experimentar con archivos en la [práctica](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) sección.

### Identificadores de archivo <a href="#identifiers" id="identifiers"></a>

El sistema de archivos de Decentraland está organizado en un índice plano (es decir, no existe una jerarquía natural de directorios), donde cada archivo se identifica mediante una cadena única.

Las cadenas de identificadores tienen como prefijo [base32](https://en.wikipedia.org/wiki/Base32) codificaciones del hash SHA-256 del contenido del archivo, usando el [IPFS CID v1](https://docs.ipfs.tech/concepts/content-addressing/) algoritmo. Se ven así:

```
bafybeicgclohdfaccu2sqqkzrzuenjxzcry3m5vcb4mpxgucjl3oheq5tq
```

El prefijo en los primeros bytes indica la codificación, la versión y el tipo de hash del propio identificador.


> [!NOTE]
> Por razones históricas, algunas API llaman al identificador del archivo *hash* en lugar de *id*, pero se refieren a lo mismo. También puedes encontrar identificadores heredados en contenido antiguo que se ven un poco diferentes.


En la práctica, estos detalles no son necesarios para descubrir y descargar contenido. La mayoría de los clientes del protocolo pueden tratar los identificadores como cadenas opacas sin perder capacidades.

Ten en cuenta que los identificadores CIDv1 modernos son compatibles con IPFS, lo que genera el mismo CID para el mismo archivo, por lo que IPFS puede usarse como el proveedor de archivos.

### Propiedad y persistencia

Excepto por [instantáneas](/docs/contributor/contributor-es/contenido/snapshots/), los archivos almacenados en la red están asociados a una [entity](/docs/contributor/contributor-es/contenido/entities/) propiedad de una cuenta de Ethereum. El propietario es el único autorizado a actualizar la entity y los archivos relacionados.

Los servidores de contenido están obligados por el protocolo a almacenar siempre la última versión de una entity y sus archivos, pero pueden elegir si conservar o no las versiones anteriores según su configuración individual.

### Descargando archivos <a href="#downloading" id="downloading"></a>

El [`/contents/<fileId>`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getContentFile) endpoint del servidor de contenido puede usarse para descargar cualquier archivo. Sus identificadores se encuentran en el manifest de la [entity](/docs/contributor/contributor-es/contenido/entities/)padre.

### Subiendo archivos

Los archivos no se suben de forma independiente. En su lugar, se empaquetan dentro de entities y se suben durante el despliegue de la entity al servidor de contenido.

Consulta la [documentación para creadores de contenido](https://github.com/decentraland/docs/tree/main/creator/README.md) para obtener más detalles.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/file-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
