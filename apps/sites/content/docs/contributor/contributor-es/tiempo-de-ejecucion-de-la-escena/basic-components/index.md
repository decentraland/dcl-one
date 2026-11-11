# Componentes básicos

World Explorer admite una lista de componentes básicos que comparten su definición con las scenes, ofreciendo soporte integrado para posicionamiento, animación, media y consultas del estado del mundo

Hay cinco tipos de componentes:

1. [Componentes de objeto](#object) agregan propiedades visuales y físicas a Entities.
2. [Componentes de juego](#game) pueden solicitar acciones y recibir información del motor del juego.
3. [Componentes de media](#media) pueden mostrar imágenes y reproducir sonidos.
4. [Componentes de área](#area-components) alteran el comportamiento de Entities en zonas específicas.
5. [Componentes de UI](#ui) permiten que las scenes rendericen interfaces flotantes.

Excepto [`Transform`](#transform), que se especifica en detalle a continuación, el estado de todos los components se serializa usando protocol buffers. Puedes seguir el enlace en cada título para ver la definición completa.

### Componentes de objeto <a href="#object" id="object"></a>

Los componentes más comunes son los que adjuntan propiedades visuales y físicas a un Entity. Pueden usarse para posicionar, redimensionar, pintar y añadir colisión a los objetos del juego.

***

#### `Transform` [↗ spec](https://adr.decentraland.org/adr/ADR-153) <a href="#transform" id="transform"></a>

Añade posición, rotación y escala a un entity.

Este componente es, con mucho, el más utilizado y actualizado con mayor frecuencia durante la vida útil de una scene. Por eso, `Transform` no se serializa usando protocol buffers, y en su lugar se empaqueta en una estructura personalizada.

`Transform` tiene un tamaño de `44` bytes, con este diseño exacto:

```
.-----------------------.-------------------------------.-----------------------.-----------------.
|   x   |   y   |   z   |   x   |   y   |   z   |   w   |   x   |   y   |   z   | parent (uint32) |
'-----------------------'-------------------------------'-----------------------'-----------------'
╵  position (3x float)  ╵      rotation (4x float)      ╵   scale (3x float)    ╵

```

Este enfoque permite al runtime (especialmente en entornos de código de bajo nivel) evitar pasos intermedios de serialización y deserialización. La memoria donde un `Transform` reside puede copiarse, compartirse y apuntarse directamente.

Las coordenadas pueden ser fraccionarias y, por tanto, usan un número de punto flotante de 4 bytes.

El `parent` campo indica que este Entity debe posicionarse en relación con otro, identificado por su ID.

Como el valor nulo de `parent` también es el [`RootEntity`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-entities/#rootentity) ID (`0`), cualquier `Transform` es relativo a él por defecto.

Por protocolo, `Transform` debe serializarse en orden de bytes big-endian.

***

#### `MeshRenderer` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_renderer.proto#L7) <a href="#meshrenderer" id="meshrenderer"></a>

Proporciona comportamiento básico de renderizado para un Entity.

Puede configurarse para renderizar un plano, una esfera, un cubo o un cilindro. Para renderizar formas 3D arbitrarias, usa [`GltfContainer`](#gltfcontainer).

***

#### `MeshCollider` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_collider.proto#L15) <a href="#meshcollider" id="meshcollider"></a>

Proporciona comportamiento básico de colisión y detección del puntero del mouse para un Entity.

Puede configurarse para comportarse como un plano, una esfera, un cubo o un cilindro. Para tener una mesh de colisión personalizada, usa [`GltfContainer`](#gltfcontainer).

***

#### `Material` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/material.proto#L19) <a href="#material" id="material"></a>

Establece las propiedades de texture, iluminación, color y transparencia de un Entity que también tiene el [`MeshRenderer`](#meshrenderer) component.

Es una estructura compleja diseñada para cubrir todos los casos comunes, pero cada campo tiene un valor predeterminado razonable y puede dejarse sin especificar.

Para personalizar por completo las propiedades visuales de un Entity, usa [`GltfContainer`](#gltfcontainer).

***

#### `GltfContainer` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/gltf_container.proto#L7) <a href="#gltfcontainer" id="gltfcontainer"></a>

Adjunta un modelo 3D a este Entity, dado el [ruta del archivo](/docs/contributor/contributor-es/contenido/entities/#files) de un `.gltf` asset en el manifest de la scene.

Como el modelo tiene sus propias meshes y materials, este componente anula cualquier comportamiento de [`MeshRenderer`](#meshrenderer), [`MeshCollider`](#meshcollider) y [`Material`](#material).

***

#### `Billboard` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/billboard.proto#L17) <a href="#billboard" id="billboard"></a>

Hace que un Entity reoriente automáticamente su `Transform` para mirar hacia la cámara, o opcionalmente hacia otro Entity referenciado mediante `target_entity`. Como indica el nombre, se usa para mostrar billboards dentro del juego y frecuentemente se combina con [`TextShape`](#textshape).

Afecta todas las direcciones por defecto, pero puede configurarse para rotar solo en un eje. Si el target Entity referenciado no existe, la reorientación se suspende hasta que exista.

***

#### `Animator` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/animator.proto#L7) <a href="#animator" id="animator"></a>

Define una o más animations que pueden aplicarse simultáneamente a un entity.

Las scenes pueden establecer el estado del componente para personalizar, reproducir y transicionar entre animaciones, así como leerlo para comprobar qué animaciones están disponibles o se están reproduciendo actualmente.

***

#### `AvatarAttach` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_attach.proto#L16) <a href="#avatarattach" id="avatarattach"></a>

Indica que la posición de un Entity debe seguir un punto de anclaje particular en el cuerpo de un avatar.

Puede afectar a cualquier avatar, no solo al del jugador, estableciendo un ID de usuario.

***

#### `TextShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/text_shape.proto#L11) <a href="#textshape" id="textshape"></a>

Renderiza texto en la posición de un Entity, dada por su `Transform`.

Es altamente configurable y admite parámetros como size, margin, padding, color, shadow y más.

***

#### `Visibility` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/visibility_component.proto#L7) <a href="#visibility" id="visibility"></a>

Establece si un Entity es visible (el valor predeterminado) o invisible.

Los objetos invisibles siguen existiendo y mostrarán el comportamiento de cualquier otro componente adjunto.

***

#### `AvatarShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_shape.proto#L8) <a href="#avatarshape" id="avatarshape"></a>

Contiene información sobre el avatar del jugador, incluyendo su body shape, colores, wearables y estado transitorio.

Este componente se adjunta a entities en la [scene del avatar](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/#avatarscene).

### Componentes de juego <a href="#game" id="game"></a>

Algunos componentes básicos pueden ser usados por la scene y el runtime para intercambiar información. Aprovechan el [mecanismo de sincronización ECS](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/engine-api/#synchronization) para garantizar la consistencia y el orden de cualquier cambio de estado o evento.

***

#### `CameraMode` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode.proto#L8) <a href="#cameramode" id="cameramode"></a>

Puede usarse para determinar si el jugador tiene una vista en primera persona o tercera persona.

***

#### `PointerLock` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_lock.proto#L7) <a href="#pointerlock" id="pointerlock"></a>

Puede usarse para determinar si el puntero del mouse sigue automáticamente el punto de enfoque de la cámara (bloqueado), o si puede moverse libremente por la pantalla (desbloqueado).

Está adjunto al [`CameraEntity`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-entities/#cameraentity), y su estado puede leerse (pero no escribirse) desde la scene.

***

#### `PointerEvents` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events.proto#L17) <a href="#pointerevents" id="pointerevents"></a>

Muestra retroalimentación visual cuando el puntero hace clic o pasa sobre un Entity.

***

#### `PointerEventsResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events_result.proto#L13) <a href="#pointereventsresult" id="pointereventsresult"></a>

Contiene información sobre la entrada reciente del jugador, incluyendo teclas y acciones del puntero.

Está adjunto al `RootEntity`, y actualizado por el runtime con cualquier evento nuevo en cada frame.

***

#### `Raycast` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast.proto#L15) <a href="#raycast" id="raycast"></a>

Puede adjuntarse a un Entity para solicitar un raycast al motor del juego. El `RaycastResult` component se adjuntará más tarde al mismo entity.

Se pueden configurar el origen, la dirección y la longitud máxima del ray.

***

#### `RaycastResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast_result.proto#L9) <a href="#raycastresult" id="raycastresult"></a>

Adjunto por el runtime a Entities que tienen un [`Raycast`](#raycast) component con resultados pendientes.

Contiene información sobre el ray original e identifica cualquier Entity que haya sido impactado.

### Componentes de media <a href="#media" id="media"></a>

Las scenes pueden adjuntar componentes especiales para mostrar imágenes, mostrar video o reproducir sonidos.

***

#### `AudioSource` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_source.proto#L7) <a href="#audiosource" id="audiosource"></a>

Reproduce un clip de audio incluido con la scene, dada la [ruta del archivo en su manifest](/docs/contributor/contributor-es/contenido/entities/#files).

El sonido se origina en la posición del entity asociado. Su tono, volumen y comportamiento de reproducción en bucle pueden configurarse, y puede leerse el estado del reproductor de audio.


> [!NOTE]
> En preparación para futuras mejoras, el campo en `AudioSource` se llama `audio_clip_url`, pero en la versión actual del protocolo en realidad es la ruta definida en el manifest.


***

#### `AudioStream` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_stream.proto#L7) <a href="#audiostream" id="audiostream"></a>

Similar a [AudioSource](#audiosource), pero el audio se transmite en tiempo real desde una URL externa.

A pesar de estar adjunto a un entity particular, el sonido no se ve afectado por su posición. Su volumen puede configurarse y puede leerse el estado del reproductor de audio.

***

#### `NftShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/nft_shape.proto#L36) <a href="#nftshape" id="nftshape"></a>

Muestra un NFT asociado a un asset de imagen o video.

Renderiza un canvas 2D con un marco decorativo configurable.

### Componentes de área

Estos componentes permiten que las scenes modifiquen el comportamiento predeterminado de Entities dentro de límites especificados.

***

#### `AvatarModifierArea` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_modifier_area.proto#L15) <a href="#avatarmodifierarea" id="avatarmodifierarea"></a>

Cambia el comportamiento de los avatars dentro de un espacio centrado alrededor de un Entity.

Se define con un vector de tamaño 3D y puede afectar si los avatars son visibles o se les puede hacer clic. Se pueden excluir avatars específicos de este efecto dado el ID de su usuario.

***

#### `CameraModeArea` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode_area.proto#L11) <a href="#cameramodearea" id="cameramodearea"></a>

Cambia el modo de cámara (primera persona o tercera persona) dentro de un espacio centrado alrededor de un Entity.

Se define con un vector de tamaño 3D y un modo de cámara deseado.

### Componentes de UI <a href="#ui" id="ui"></a>

Los siguientes componentes se usan para crear interfaces gráficas que flotan sobre el mundo del juego.

Suelen adjuntarse a conjuntos de Entities que tienen [`UiTransform`](#uitransform) components, relacionados entre sí a través del `parent` atributo.

Por ejemplo, una ventana flotante de opciones podría ser un Entity con un [`UiTransform`](#uitransform) y un [`UiBackground`](#uibackground) component, además de un Entity con un hijo [`UiTransform`](#uitransform) y un [`UiDropdown`](#uidropdown).

***

#### `UiTransform` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_transform.proto#L77) <a href="#uitransform" id="uitransform"></a>

Describe el tamaño, posicionamiento, margen y padding de un componente de UI.

Se basa en el modelo flexbox y es altamente personalizable.

***

#### `UiBackground` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_background.proto#L11) <a href="#uibackground" id="uibackground"></a>

Describe un color o Texture para usar como fondo en un UI Entity.

***

#### `UiDropdown` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown.proto#L11) <a href="#uidropdown" id="uidropdown"></a>

Define una lista de opciones mutuamente excluyentes para mostrarse en un widget dropdown.

***

#### `UiDropdownResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown_result.proto#L9) <a href="#uidropdownresult" id="uidropdownresult"></a>

Contiene el valor seleccionado de un [`UiDropdown`](#uidropdown), establecido por el runtime y leído por la scene.

***

#### `UiInput` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input.proto#L11) <a href="#uiinput" id="uiinput"></a>

Define un widget de entrada de texto, con cierto margen para personalización.

***

#### `UiInputResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input_result.proto#L9) <a href="#uiinputresult" id="uiinputresult"></a>

Contiene el valor de texto de un [`UiInput`](#uiinput), establecido por el runtime y leído por la scene.

***

#### `UiText` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_text.proto#L11) <a href="#uitext" id="uitext"></a>

Define una vista de texto simple, con cierto margen para personalización.

***


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
