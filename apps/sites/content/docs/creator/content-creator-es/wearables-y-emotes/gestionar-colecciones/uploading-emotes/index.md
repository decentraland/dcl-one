> Guías para subir Emotes al Editor

# Subiendo Emotes

Una vez que exportes tu emote, tendrás que subirlo a Builder. Este documento cubrirá el proceso de carga de emotes.

## **Subir tu archivo**

Recuerda que necesitas crear una colección antes de poder subir tu archivo. Si no sabes cómo hacerlo, consulta [Crear una colección](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/). Para subir tu emote, solo arrastra y suelta el archivo en la ***Nuevo ítem*** ventana o explora tu computadora. Detectará automáticamente si el archivo tiene alguna animación, identificándolo como un emote.

![](/docs/_remote/3db6fae4f154b1b73fd50dc4.webp)

Arrastra y suelta tu archivo de animación para subirlo.

Se te pedirá que ingreses un nombre para tu emote, definas su rareza, la categoría y el modo de reproducción. Debajo de la miniatura se muestra la cantidad de clips de animación en tu archivo, la duración de la animación en segundos, la duración en fotogramas y la tasa de fotogramas.

![](/docs/_remote/e376a1e332918af541d1a910.webp)

## **Carga de Emotes usando un archivo .zip**

Si el emote tiene sonido (*mp3* o *ogg*) debe comprimirse con el `.glb`. Después de eso, solo arrastra y suelta el `.zip` al Builder. Además, es posible agregar un `.json` archivo junto con los demás assets en el mismo `.zip` para agregar nombre, descripción, rareza, categoría, modo de reproducción y/o etiquetas. Estas son las definiciones de cada uno:

* `name`: Nombre del Emote
* `description`: Descripción de tu Emote (no más de 64 caracteres en total, contando espacios)
* `categoría`: Categoría del Emote ("dance", "stunt", "greetings", "fun", "poses", "reactions", "horror", "miscellaneous")
* `rareza`: Rareza del item ("unique", "mythic", "legendary", "epic", "rare", "exotic", "uncommon", "common")
* `play_mode`: Animación simple o en bucle ("simple", "loop")
* `tags`: Etiquetas para facilitar la búsqueda en el marketplace.

Para agregar esas definiciones al emote, solo crea un archivo de texto, nombrándolo **emote.json** y agrega las siguientes líneas como ejemplo:

```
{
  "name": "Golpe de tenis",
  "description": "Muéstrame que sabes jugar al tenis",
  "category": "fun",
  "rarity":"epic",
  "play_mode": "simple",
  "tags":["tenis", "emote", "golpe"]

}

```

De esta manera, Builder tomará toda la información del .json y la aplicará automáticamente al emote.

**Rareza**

Consulta la tabla de rarezas en [aquí](/docs/creator/wearables-and-emotes/manage-collections/creating-a-collection/#rarity).

## **Categoría**

Elige la categoría que mejor describa tu emote.

* Baile
* Acrobacia
* Saludos
* Diversión
* Poses
* Reactions
* Terror
* Misceláneo

## **Modo de reproducción**

Actualmente hay dos modos de reproducción:

* ***Reproducir una vez***: significa que tu emote se reproducirá solo una vez. Después de eso, el avatar volverá a la *Idle* posición.
* ***Bucle***: el emote seguirá reproduciéndose en bucle hasta que el usuario ingrese otra acción.

## **Miniaturas personalizadas**

Para los emotes, no tienes que subir ninguna imagen, ya que el editor ya tiene una herramienta integrada para crear una miniatura. Solo selecciona el fotograma que mejor represente la acción.

Las personas deberían poder identificar de qué trata la animación a través de la miniatura. Si una toma frontal no es suficiente, intenta rotar el modelo, acercar o alejar el zoom, desplazar hacia arriba o hacia abajo, o elegir cualquier fotograma de tu clip. ¡Es muy importante que selecciones la mejor toma!

![](/docs/_remote/9efbcbc5b36b6a3edfbcf38b.webp)

Rota, acerca o aleja el zoom, desplaza hacia arriba y hacia abajo. ¡Usa las herramientas para obtener la mejor toma de tu animación!

## El Editor

Una vez que configures la miniatura, tendrás el editor abierto. Aquí es donde puedes comprobar si la animación se reproduce bien, ya que un avatar 3D la estará ejecutando.

![](/docs/_remote/58fef5ae57e3864b91e06697.webp)

Al hacer clic en el icono de la esquina inferior izquierda podrás editar el avatar. Haz clic en el icono del cilindro en la esquina inferior derecha para comprobar si tu animación se mantiene dentro de los límites de altura y espacio.

![](/docs/_remote/c32c351247d541f21bafadb2.webp)

El icono de la parte inferior izquierda te permite editar el avatar.

![](/docs/_remote/f4f828d6e5577c46357b79cf.webp)

El icono del cilindro muestra los límites de la animación.

También puedes editar el nombre del emote, la miniatura, la categoría, la rareza y el modo de reproducción, además de agregar una descripción de la animación y etiquetas. Haz clic en ***Guardar*** cuando hayas terminado.

* **Descripción:** Esta es una breve descripción de tu ítem que se mostrará en el Marketplace.
* **Utilidad** Describe la utilidad en el mundo del Wearable o Emote.
* **Etiquetas:** Las etiquetas son simplemente palabras descriptivas que los usuarios pueden usar al buscar o filtrar ítems. ¡Son útiles para competiciones o eventos!

![](/docs/_remote/ae584da404403aeb0e47bcee.webp)

## Probando en World

Incluso después de probar la animación en el editor, es importante comprobar cómo se verá y se comportará realmente en Decentraland. Para probarla en el mundo, ve a la pestaña Collections, selecciona la colección deseada y se mostrarán todos los items que tienes en ella. Haz clic en See in Decentraland.

\
\\

![](/docs/_remote/7fc9c8d40af1b921993b8bfb.webp)

\
\\


> [!NOTE]
> Se te pedirá que elijas si quieres probarlo en un Empty Parcel o en Genesis Plaza. Seleccionar Empty Parcels te llevará a un lugar sin demasiado contenido, que cargará más rápido. Seleccionar Genesis Plaza te llevará a la plaza principal.


\
\\

![](/docs/_remote/d1c728c7c0ffee7fe42730da.webp)

\
\\


> [!NOTE]
> Después de elegir, se abrirá una nueva pestaña en tu navegador con un mensaje que te informa que estás a punto de usar un Catalyst personalizado. Haz clic en TRUST PEER-TESTING.DECENTRALAND.ORG para continuar.


\
\\

![](/docs/_remote/5e0eef48997d27eadb27149b.webp)

\
\\


> [!NOTE]
> Después de eso, verás una ventana emergente que te pedirá abrir Decentraland Explorer. Una vez que el Explorer esté en ejecución, haz clic en Jump Into Decentraland.


\
\\

![](/docs/_remote/0518f3adcaa8974ae07124e4.webp)

\
\\


> [!NOTE]
> Una vez en el mundo, presiona I para abrir la mochila y, en la pestaña de emotes, selecciona el emote para probarlo.


\
\\

![](/docs/_remote/80aeb1e924a01a378e86f206.webp)

\
\\

## **Antes de publicar**

Asegúrate de agregar una buena descripción y verificar si toda la información y los ajustes son correctos. Vuelve a comprobar también la miniatura. Si has completado toda la información necesaria, verás Ready to Submit como el estado de tu item. ¡Ahora tu emote está listo para publicarse!

![](/docs/_remote/e404c18c57fe014dd11e6091.webp)

¡El estado Ready to Submit significa que tu archivo está listo para publicarse!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/wearables-y-emotes/gestionar-colecciones/uploading-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
