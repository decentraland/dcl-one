> Aprende cómo mostrar un NFT certificado que posees en tu escena

# Mostrar un NFT certificado

Puedes mostrar un NFT 2D (token no fungible) que posees en tus escenas de Decentraland.

La imagen del NFT y otros datos se obtienen de una API, según el contrato y el id del token. Cualquier NFT compatible con [OpenSea](https://opensea.io/) también puede mostrarse en un marco de imagen NFT en Decentraland.


> [!NOTE]
> **💡 Consejo**: En el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes usar un **NFT** [Smart Item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para una forma sin código de lograrlo.


El marco de imagen se muestra ajustándose a las dimensiones de la imagen del NFT. Si las dimensiones de la imagen son 512 x 512 píxeles, el marco conserva su tamaño original. Si la imagen tiene dimensiones diferentes, el marco se redimensionará y estirará para coincidir con estas dimensiones.


> [!NOTE]
> **💡 Consejo**: Si quieres estirar o redimensionar la imagen a partir de la que se genera por defecto, puedes cambiar la `scale` propiedad en la `Transform` component.


## Agregar un NFT

Añade un `NftShape` componente a una entidad para mostrar un token 2D en tu escena.

```ts
const nft = engine.addEntity()

Transform.create(nft, {
	position: Vector3.create(8, 1, 8),
})

NftShape.create(nft, {
	urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
})
```

El `NftShape` el componente debe instanciarse con un parámetro que incluya lo siguiente:

* `urn` campo. Este campo toma una cadena que debe seguir esta estructura:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

Esta cadena incluye:

* El *network* donde existe este token. Consulta la lista de valores compatibles a continuación.
* El *estándar de contrato* en el que se basa este token, por ejemplo `erc721`
* El *contract* del token (por ejemplo, el contrato de CryptoKitties)
* El *id* del token específico que se mostrará

Por ejemplo:

`urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536`

El ejemplo anterior obtiene un NFT con la dirección de contrato `0x06012c8cf97BeaD5deAe237070F9587f8E7A266d`, y el identificador específico `558536`. El activo correspondiente puede encontrarse en OpenSea en [https://opensea.io/assets/0x06012c8cf97BeaD5deAe237070F9587f8E7A266d/558536](https://opensea.io/assets/0x06012c8cf97BEaD5deAe237070F9587f8E7A266d/558536).

Los siguientes valores son compatibles con *network*:

* `ethereum`: Ethereum Mainnet
* `matic`: Polygon Matic
* `klaytn`: Klaytn
* `bsc`: BNB Chain
* `arbitrum`: Arbitrum
* `arbitrum_nova`: Arbitrum Nova
* `avalanche`: Avalanche
* `optimism`: Optimism
* `solana`: Solana
* `base`: Base
* `blast`: Blast
* `zora`: Zora


> [!WARNING]
> **📔 Nota**: Las imágenes deben pesar como máximo **6 MB**. Si la imagen es más grande, no se renderizará en la escena.


## Personaliza el marco

De forma predeterminada, la imagen tendrá un fondo morado y un marco con una textura emisiva pulsante alrededor. Puedes establecer las siguientes propiedades para que se adapten mejor al estilo del NFT y la escena:

* `color`: Determina la parte trasera del modelo y también el fondo de la imagen en caso de que la imagen del NFT tenga transparencia.
* `style`: Selecciona un modelo de marco de un enum con varias opciones predeterminadas. Usa un valor del enum `NftFrameType`, que contiene una lista de todos los estilos disponibles.

```ts
NftShape.create(nftEntity, {
  urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97bead5deae237070f9587f8e7a266d:558536',
  color: Color3.Green(),
  style: NftFrameType.NFT_GOLD_EDGES,
})
```

![](/docs/_remote/8698f94b4eeaa8c4125b5ae6.webp)

Aquí está la lista completa de estilos de marco compatibles:

* `NFT_CLASSIC`
* `NFT_BAROQUE_ORNAMENT`
* `NFT_DIAMOND_ORNAMENT`
* `NFT_MINIMAL_WIDE`
* `NFT_MINIMAL_GREY`
* `NFT_BLOCKY`
* `NFT_GOLD_EDGES`
* `NFT_GOLD_CARVED`
* `NFT_GOLD_WIDE`
* `NFT_GOLD_ROUNDED`
* `NFT_METAL_MEDIUM`
* `NFT_METAL_WIDE`
* `NFT_METAL_SLIM`
* `NFT_METAL_ROUNDED`
* `NFT_PINS`
* `NFT_MINIMAL_BLACK`
* `NFT_MINIMAL_WHITE`
* `NFT_TAPE`
* `NFT_WOOD_SLIM`
* `NFT_WOOD_WIDE`
* `NFT_WOOD_TWIGS`
* `NFT_CANVAS`
* `NFT_NONE`

Algunos marcos usan más materiales que otros. Por ejemplo, el marco predeterminado añade 1 material para el propio NFT, 1 material para un plano de fondo con color y 2 materiales para el marco (compartidos con otros marcos de imagen del mismo estilo). Si necesitas reducir la cantidad de materiales para [mantenerte dentro de las limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/), elige un estilo más simple. Por ejemplo, "none" usa solo 1 material para el propio NFT.


> [!NOTE]
> **💡 Consejo**: Usando Visual Studio Code (u otro IDE), ve la lista completa escribiendo `NftFrameType.` y esperando a que las sugerencias inteligentes muestren la lista de opciones. Usa `NftFrameType.NFT_NONE`para mostrar el NFT tal cual, sin marco ni color de fondo.


## Abrir una UI de NFT

Abre una UI preconstruida que muestra el nombre, el propietario y la descripción de un NFT. También incluye el precio actual del NFT y el precio de la última venta, si corresponde, y un botón que enlaza a la página del NFT en OpenSea, donde hay más información disponible y se puede comprar.

![](/docs/_remote/2e438b55141b8a0db1d92f61.webp)

Abre esta UI llamando a la función `openNftDialog()`. Esta función requiere un objeto como argumento que contenga un único `urn` campo. Este campo toma una cadena que debe seguir esta estructura:

`urn:decentraland:<CHAIN>:<CONTRACT_STANDARD>:<CONTRACT_ADDRESS>:<TOKEN_ID>`

Por ejemplo:

`urn:decentraland:ethereum:erc721:0x00...000:123`


> [!WARNING]
> **📔 Nota**: La UI debe abrirse como resultado de un evento de botón, para evitar spam abusivo. El evento de botón no necesariamente tiene que estar en el mismo picture frame o en un `NftShape`.


Para abrir esta UI como resultado de una acción de clic, añade lo siguiente:

```ts
import { openNftDialog } from '~system/RestrictedActions'

// Añade un collider de puntero para que se pueda hacer clic
MeshCollider.setBox(myEntity, ColliderLayer.CL_POINTER)

// Añade el evento de callback onPointerDown.
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Clic' },
	},
	function () {
		openNftDialog({
			urn: 'urn:decentraland:ethereum:erc721:0x06012c8cf97BEaD5deAe237070F9587f8E7A266d:558536',
		})
	}
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/media/display-a-certified-nft.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
