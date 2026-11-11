> Edita los metadatos de tu escena

# Configuración de la escena

Haz clic en el **icono de lápiz** en la parte superior derecha de la pantalla. Esto abre una serie de propiedades a nivel de escena para editar.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

Aquí puedes configurar varias propiedades, incluido el título y la miniatura, el tamaño de la escena, las categorías de la escena y los interruptores de funciones.

Consulta [Metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/).

## Detalles de la escena

El **Detalles** la pestaña te permite configurar varios campos sobre tu escena. Estos campos se muestran a los jugadores que podrían visitar tu escena, por ejemplo al ampliar la ubicación en el mapa, cuando se les solicita teletransportarse, o al compartir un enlace a la escena en redes sociales. ¡Asegúrate de que la información aquí sea atractiva y precisa para atraer más tráfico a tu escena!

![](/docs/_remote/0d1050cf72d4ecbad89ddda0.webp)

Los siguientes campos están disponibles:

* **Nombre**
* **Descripción**
* **Miniatura**
* **Categorías**
* **Nombre del creador**
* **Correo electrónico de contacto del creador** (opcional)
* **Dirección de la wallet del creador** (opcional)

Consulta [metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) para más detalles sobre estos campos.


> [!NOTE]
> **📔 Nota**: Si publicas tu escena en un [World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), el World tiene su propio nombre, descripción y miniatura, separados de los de la escena. Si el World contiene una sola escena, los metadatos del World se actualizan para coincidir con los detalles de la escena en cada publicación, así que este panel es todo lo que necesitas editar. Si el World contiene varias escenas, los metadatos del World solo se pueden editar en la **Administrar** pestaña de Creator Hub. Consulta [Metadatos del World vs metadatos de la escena](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#world-metadata-vs-scene-metadata).



> [!WARNING]
> **📔 Nota**: El **Age Rating** no se edita en este panel. Puedes establecer el `clasificación` directamente en el campo `scene.json` archivo, o, para escenas publicadas en un World, cambia el **Content Rating** en la configuración del World de Creator Hub después de publicar. Decentraland es una plataforma para mayores de 18 años, así que la clasificación que debes establecer es `Un` para adultos. Consulta [Age Rating](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#age-rating).


### Miniatura de la escena

La miniatura debe ser una imagen .png o .jpg con una **16:9** proporción de aspecto. 1920x1080 píxeles es el tamaño al que debes apuntar. Otros tamaños funcionan siempre que mantengan las mismas proporciones 16:9.

Una imagen que no sea 16:9 es rechazada: al importar una aparece un error, y al elegir una imagen con una proporción incorrecta que ya está en tu proyecto aparece **Miniatura no compatible** y conserva la miniatura que tenías.

Algunas partes de la plataforma muestran una versión cuadrada reducida de la miniatura, recortada al cuadrado central. Eso recorta aproximadamente un 22% de cada lado. Mantén cualquier elemento esencial, como texto o logotipos, dentro de esa zona segura para que no se pierda.

El **Detalles** la pestaña muestra tu miniatura a todo ancho, con esas dos franjas laterales sombreadas para que puedas ver exactamente qué recorta una vista cuadrada, y debajo el tamaño real en píxeles de la imagen.

![Área segura de la miniatura de la escena: la imagen completa de 1920x1080, con el cuadrado central de 1080x1080 marcado como área segura y las franjas izquierda y derecha marcadas como recortadas](/docs/_remote/a4fca710bad41737e2da1248.webp)


> [!NOTE]
> **💡 Consejo**: Si no proporcionas una miniatura, la escena usa la captura automática que ves en la tarjeta de la escena. Esa captura se recorta desde el centro a 16:9 y nunca se amplía, por lo que puede acabar con menos de 1920 de ancho. Subir tu propia imagen casi siempre es mejor.


#### Mantén el archivo por debajo de 1 MB

Mantén la miniatura en 1 MB (1.048.576 bytes) o menos. Publicar una escena no comprueba el tamaño, pero [Configuración del World](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#world-settings) sí lo hace: cuando Creator Hub guarda la configuración de un World, una miniatura de más de 1 MB se rechaza con un error como este:

```
Miniatura no válida: el tamaño de 1185064 bytes supera el máximo de 1048576 bytes (1 MB).
```

Un PNG normal de 1920x1080 puede superar el límite. Dos formas de mantenerse por debajo:

* **Exporta un JPEG en lugar de un PNG**, con alta calidad. Por ejemplo, `ffmpeg -i cover.png -vf scale=1344:756 -q:v 2 cover.jpg` redimensiona la imagen a 1344x756 y produce un archivo de aproximadamente 100 KB.
* **Renderiza a 1344x756** en lugar de 1920x1080. Sigue siendo 16:9, y el archivo más pequeño deja mucho margen.

#### No nombres tu archivo `autogenerated-thumbnail.png`

Creator Hub captura la vista en `assets/Images/autogenerated-thumbnail.png` cada vez que genera una miniatura para ti, incluso cuando comienza una publicación. Sobrescribe lo que ya haya en esa ruta. Los proyectos creados antes de Creator Hub 0.52.0 pueden tener una `assets/images/` carpeta en minúsculas en su lugar, y Creator Hub sigue usándola.

Nada te avisa de que esto ocurrió: Creator Hub solo reescribe la entrada de la miniatura en `scene.json` cuando falta el archivo al que apunta, y después de la sobrescritura el archivo sigue ahí.

Guarda tu propia portada con un nombre diferente, como `assets/Images/cover.png`, y apunta el **Miniatura** campo a ese archivo.

### Propinas

Puedes recibir propinas de los jugadores que visiten tu escena. Para activar las propinas, ve a la **Detalles** pestaña de la configuración de la escena y proporciona una dirección de Ethereum en **Dirección de la wallet del creador**.

![](/docs/_remote/9c99df3549a6adf960b5d332.webp)

Cuando un jugador visite tu escena, verá un icono de alcancía en la parte superior izquierda de la pantalla. Al hacer clic en él se abre un modal donde puede enviarte una propina. Este menú también se puede acceder abriendo la información de tu escena en el mapa.

![](/docs/_remote/2e01580f2f84c6ab2edcbf00.webp)

El modal de propinas permite al jugador seleccionar la cantidad de MANA que desea enviar. El jugador debe tener MANA en su wallet para enviar una propina. Si la dirección que proporcionaste está vinculada a un NAME de Decentraland, este modal mostrará el nombre del propietario de la wallet junto a la dirección de Ethereum.

![](/docs/_remote/1cf2999882d7d551c8ac0687.webp)

Recibirás una notificación en la pestaña de notificaciones de Decentraland cada vez que un jugador te envíe una propina.

## Diseño

Puedes editar el tamaño de tu escena haciendo clic en el *icono de lápiz* y luego cambiando el número de filas y columnas.

Las escenas en Decentraland ocupan uno o varios parcels de LAND adyacentes. Cada parcel de LAND mide 16x16 metros.

Establece el número de parcelas para las filas y columnas y haz clic en **Aplicar diseño** para que afecte al aspecto de tu escena en el lienzo de Scene Editor.

![](/docs/_remote/7d1efe3c48ae28dafce2eb3d.webp)

Para construir algo para desplegar en parcelas de LAND que posees, asegúrate de que la forma de la escena coincida con la forma del lugar donde quieres desplegarla.


> [!NOTE]
> **💡 Consejo**: Puedes desactivar cada casilla de la cuadrícula haciendo clic en ella. Esto te permite dibujar formas no rectangulares para el diseño de tu escena.
> 
> <img src="/docs/_remote/6bf863ba796d30b4746af1be.webp" alt="" data-size="original">


Si posees un NAME de Decentraland, también puedes desplegar tu escena en un [Decentraland World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds). En ese caso, puedes usar cualquier diseño de hasta 300x300 parcelas sin necesidad de poseerlas, pero tendrás un límite de tamaño en MB.

Consulta [Tipos de proyecto](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/) para entender mejor las distintas opciones.

### Vista avanzada

También puedes hacer clic en el **Establecer coordenadas (Avanzado)** botón para enumerar manualmente las coordenadas de tu escena.

![](/docs/_remote/2393cd3a4a250cc4420ad0a4.webp)

En **Coordenadas personalizadas**, escribe las coordenadas de cada uno de los parcels en los que deseas publicar. Separa las coordenadas x e y con una coma, y cada conjunto de coordenadas separado por espacios. Recuerda que todas estas coordenadas deben ser adyacentes para ser válidas. Por ejemplo:

`78,-2 79,-2 78,-3 79,-3`

En el **Punto de origen** campo, define cuál de las coordenadas de la escena debe tratarse como el punto de origen. Debe ser una de las coordenadas que enumeraste en el **Coordenadas personalizadas** campo. Se recomienda establecer la parcela en la parte inferior izquierda de la escena.

## Restricciones de la escena

Puedes desactivar ciertas funcionalidades en tu escena si lo deseas, en caso de que puedan ser abusadas o entrar en conflicto con el tipo de experiencia que quieres crear.

![](/docs/_remote/5c421faac03084ef8453fff3.webp)

* **Silenciar Voice Chat**: Impide que los jugadores de tu escena usen voice chat.
* **Desactivar Nearby Voice Chat**: Impide que los jugadores de tu escena usen el voice chat cercano (basado en proximidad).
* **Desactivar Smart Wearables y Portable Experiences**: Impide que los jugadores usen [Smart Wearables](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/) o [Portable Experiences](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/portable-experiences/).

## Control de Skybox

Puedes controlar la hora del día del skybox en la **Configuración** pestaña. Puedes establecer una hora fija del día para tu escena. Todos los jugadores verán la escena con esa hora del día, y el skybox no seguirá el ciclo día/noche.

En Creator Hub, abre la configuración de la escena y haz clic en la **Configuración** pestaña para encontrar la **Skybox** sección. Desmarca la **Auto** opción para evitar usar el ciclo día/noche y establecer la hora del día que quieras.

![](/docs/_remote/6242ab0073574f8cc2c9e7d9.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/configurar/scene-settings.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
