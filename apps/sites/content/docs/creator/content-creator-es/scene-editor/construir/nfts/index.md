> Añadiendo retratos NFT a tus escenas

# NFTs

Puedes añadir NFTs (Tokens No Fungibles) a tu scene, mostrados como marcos de fotos.

Todos los formatos de imagen y gif compatibles en OpenSea también son compatibles con Decentraland mediante marcos de fotos. Los NFTs en formato de video o audio no son compatibles actualmente. Los NFTs que también tienen representaciones 3D, como los wearables de Decentraland, se muestran en los marcos de fotos como imágenes 2D.

## Añadir un NFT

Usa el smart item NFT, que puedes encontrar en el **Smart Items** asset pack, o simplemente buscando *NFT* en la barra de búsqueda de arriba. Una vez que arrastres una copia del item NFT a tu scene y lo selecciones, hay algunos campos que puedes configurar.

![](/docs/_remote/e6ffb898b183dc0d0a5e93be.webp)

Los campos principales para configurar determinan qué NFT mostrar:

* **Network**: La red blockchain en la que está tu NFT. Usa Ethereum mainnet por defecto, pero también puedes elegir Polygon (matic), o una de varias test networks.
* **Contract**: La dirección del contrato de la colección a la que pertenece este NFT (es decir: Cryptokitties, SuperRare, Decentraland Halloween Wearables 2019, etc.)
* **Token**: El id único de este NFT específico

Para obtenerlos, la forma más sencilla es buscarlos en el Marketplace de Decentraland y luego comprobar la URL. Por ejemplo, a partir de la URL del siguiente item:

*<https://market.decentraland.org/contracts/0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0/tokens/20175>*

Puedes inferir que el contrato es *0xb932a70a57673d89f4acffbe830e8ed7f75fb9e0* (refiriéndose a SuperRare) y el ID es *20175*.

De manera similar, también puedes obtenerlos a partir de la URL de OpenSea del token. Por ejemplo, a partir de la URL del siguiente item:

*<https://opensea.io/assets/0x31385d3520bced94f77aae104b406994d8f2168c/2614>*

Puedes inferir que el contrato es *0x31385d3520bced94f77aae104b406994d8f2168c* y el ID es *2614*.

Otros campos opcionales que se pueden configurar en el smart item NFT son:

* **Estilo del marco**: El estilo de marco predeterminado tiene un borde brillante que puede no coincidir con el estilo de la obra de arte o de tu scene. Hay varias otras opciones para elegir con estilos variados, desde barroco hasta minimalista, o incluso cinta en las esquinas del cuadro.
* **Color**: A los NFTs con fondo transparente se les asigna un color de fondo, violeta por defecto. Puedes elegir cualquier otro color. Ten en cuenta que algunos estilos de marco, como *Ninguno*, no incluyen ningún color de fondo.


> [!WARNING]
> **📔 Nota**: Solo puedes ver que estos cambios surtan efecto al entrar en la scene en modo Preview. Ninguno de estos cambios modifica la representación del smart item que arrastras en modo edición.


Consulta [Mostrar un NFT](/docs/creator/content-creator-es/scenes-sdk7/media/display-a-certified-nft/) para más detalles sobre cómo Decentraland maneja los retratos NFT.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/construir/nfts.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
