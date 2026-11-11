> Importa tus propios modelos 3D, imágenes, sonido, etc., para usarlos en tus escenas.

# Importar assets personalizados

Puedes importar tus propios modelos 3D en el Scene Editor. Elige modelos de una amplia selección de fuentes gratuitas o de pago en internet, o crea tus propios modelos personalizados. También puedes importar otros assets como imágenes, archivos de sonido y videos.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/UepXpH-k0EI" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=UepXpH-k0EI>">Watch on YouTube</a></p>



## Importar un asset

Para importar un modelo 3D, una imagen, un archivo de sonido o un video a tu escena desde tu disco local:

1. Arrastra los archivos directamente al panel inferior. También puedes hacer clic en el **+ Import Assets** botón en la parte superior izquierda del panel inferior y seleccionar desde tu unidad local.

![](/docs/_remote/7a7e1bf757e28411bf4bd633.webp)

2. Revisa la miniatura del modelo y haz clic en **Import**. Al importar varios assets, usa los botones de flecha para recorrer cada asset.

![](/docs/_remote/114b427e0cc71cef23245e2a.webp)

Ahora puedes encontrar tu asset en la **Local Assets** pestaña. Los assets se ordenan en carpetas por tipo: los modelos 3D aparecen en la *assets/Models* carpeta, las imágenes en *assets/Images*, los archivos de sonido en *assets/Audio*, y los videos en *assets/Video*.

Los elementos de los asset packs gratuitos integrados se almacenan en *assets/asset-packs/*, y los elementos personalizados creados desde el editor en *assets/custom/*. Las escenas antiguas pueden tener importaciones de usuarios en *assets/scene/* en su lugar. Todas estas rutas funcionan en el código de tu escena; solo haz referencia a la carpeta en la que esté el asset.

* Para los modelos 3D, arrastra los `.glb` o `.gltf` archivos al Canvas para añadirlos como elementos en tu escena.
* También puedes arrastrar una imagen, un archivo de Audio o un archivo de Video al Canvas. El editor crea un elemento listo para usar que ya apunta a tu archivo:

  * A `.png`, `.jpg` o `.jpeg` archivo se convierte en un **Imagen** elemento.
  * Un `.mp3`, `.ogg` o `.wav` el archivo se convierte en un elemento de sonido ambiental.
  * Un `.mp4` el archivo se convierte en un **Pantalla de video** elemento.

  El nuevo elemento se nombra a partir del archivo y viene con los mismos components y ajustes que el elemento equivalente de los asset packs, así que puedes retocarlo desde el inspector de inmediato.
* Cualquier asset también puede arrastrarse a los campos de un elemento que ya está en la escena. Por ejemplo, puedes arrastrar un `.mp3` archivo al *Path* campo de un *Audio Source* component.


> [!NOTE]
> **💡 Consejo**: También puedes pegar archivos directamente en la carpeta del proyecto `assets` , desde Finder, Explorer o cualquier otro lugar. Aparecerán en la **Local Assets** pestaña por sí solos, poco después de que termine la copia. Si no aparecen, pulsa el **Refresh** botón junto a **Import Assets** botón.
> 
> <img src="/docs/_remote/bef94fa811ae48e5d4c837f0.webp" alt="" data-size="original">


### Formatos compatibles

#### Audio

Se admiten los siguientes formatos de Audio:

* *.mp3*
* *.wav*
* *.ogg*

#### Imagen

Se admiten los siguientes formatos de imagen:

* *.png*
* *.jpg*
* *.jpeg*

#### Video

Se admiten los siguientes formatos de Video:

* *.mp4*

#### Modelos 3D

Se admiten los siguientes formatos de modelos 3D:

* *.glTF*
* *.glb*

Ambos pueden incluir archivos de imagen de textura externos o binarios externos (*.bin*) archivos.

Puedes convertir otros formatos a estos formatos con distintos editores y herramientas. Consulta [modelado 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/) para obtener recomendaciones y consejos.

Todos los materiales de los modelos deben ser either *material básico* o *PBR*, y todas las texturas deben tener tamaños que sean potencias de dos (por ejemplo: 256, 512). Consulta [Limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para más detalles.

Cada archivo importado, de cualquier tipo, debe ocupar menos de 50 MB para poder usarse en una escena. Los archivos más grandes no son compatibles.

**Bibliotecas gratuitas de modelos 3D**

En lugar de crear tus propios modelos 3D, también puedes descargarlos de varias bibliotecas gratuitas o de pago, o generarlos con herramientas de IA. Consulta [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) para ver una lista de bibliotecas de assets recomendadas y herramientas de IA generativa.


> [!WARNING]
> **📔 Nota**: Presta atención a las restricciones de licencia que tenga el contenido que descargas.


Ten en cuenta que en varios de estos sitios puedes elegir en qué formato descargar el modelo. Elige siempre *.glTF* o *.glb* el formato si está disponible. Si no está disponible, debes convertirlos a *.glTF* o *.glb* antes de poder usarlos en una escena. Para ello, recomendamos importarlos en Blender y exportarlos con uno de los *.glTF* complementos de exportación.

### Colliders

Puede que, al ejecutar una Preview, el jugador pueda atravesar tus modelos 3D importados. Es probable que sea porque a los modelos les falta un *collider mesh* para definir una geometría de colisión. Consulta [colliders](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/) para más detalles e instrucciones.


> [!NOTE]
> **💡 Consejo**: En lugar de editar el modelo para añadir un *collider mesh*, una alternativa más sencilla es añadir un *Invisible wall* smart item con aproximadamente la misma forma para ocupar su lugar.


### Animations

Si un modelo importado incluye animaciones, la primera animación empaquetada en el modelo se reproducirá en loop.

Ten en cuenta que no tienes ningún control sobre cuándo empieza o se detiene la animación, ni sobre cuál se reproduce en caso de varias animaciones.

Si hay varios jugadores en la escena, puede que vean la animación desincronizada entre sí.

Para cambiar este comportamiento, puedes incluir un **Animator** componente. Consulta [Haz inteligente cualquier ítem](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/) para herramientas sin código para hacer que tu elemento sea interactivo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/construir/import-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
