> Los tipos de proyectos disponibles que puedes crear en Decentraland.

# Resumen

Existen diferentes tipos de contenido que puedes crear para Decentraland.

![](/docs/_remote/0a4a8ae5cf4dbf5eb00b75db.webp)

## escenas

Las escenas de Decentraland pueden alojarse en Parcelas en Genesis City o en Worlds.

### Publicar en LAND

Las escenas que se publican en Parcelas pueden encontrarse en coordenadas específicas dentro del mundo abierto de Decentraland.

Estas escenas están vinculadas a LAND o a tokens Estate. Cada parcela ocupa 16x16 metros. Una sola escena puede ocupar varias parcelas adyacentes; estas pueden disponerse en cualquier forma, siempre que sus bordes se toquen.

Los tokens LAND se pueden comprar en el [Marketplace](https://decentraland.org/marketplace/lands). Existe una oferta limitada de ellos, que cubre el mapa de Genesis City.

Las escenas publicadas en LAND son más fáciles de descubrir, ya que los jugadores pueden encontrárselas mientras visitan contenido cercano o exploran.

Las escenas publicadas en LAND pueden usar hasta 15 MB de espacio por cada parcela de la escena. Cuantas más parcelas haya, más espacio disponible habrá. Esto es para evitar sobrecargar la CPU del jugador, ya que es posible que los jugadores estén experimentando muchas escenas cercanas al mismo tiempo. Ver [limitaciones de tamaño](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/).

### Publicar en Worlds

Se debe acceder a las escenas publicadas en un World mediante un enlace.

Estas escenas están vinculadas a tokens NAME. Los tokens NAME se pueden comprar en el [Marketplace](https://decentraland.org/marketplace/names/claim). Puedes reclamar el nombre que quieras siempre que aún no haya sido reclamado. Como alternativa, puedes usar un [dominio ENS](https://ens.domains) para crear un World.

Las escenas publicadas en un World tienen una limitación de parcelas de 300x300. Se aplican las mismas reglas por parcela que en las escenas publicadas en parcelas LAND, pero puedes añadir más parcelas a tu escena sin ningún costo. [limitaciones de tamaño](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) por parcela se aplica como en las escenas publicadas en parcelas LAND, pero puedes añadir más parcelas a tu escena sin ningún costo.

Si se trata de un WORLD de varias escenas, distintos Collaborators (addresses) pueden tener derechos de publicación sobre parcelas específicas del WORLD, definidos por el World Owner (la address que reclamó el NAME del World). Además, el acceso a un World puede ser **Público** o **Privado**. Más información sobre [Gestionar Worlds](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#managing-worlds)

Un World puede ser **Público** o **Privado**

#### Límites de tamaño

El tamaño máximo de archivo que puedes subir a tu World depende de si usas un Decentraland NAME o un dominio ENS.

**Worlds de Decentraland NAMEs**

Los titulares de Decentraland NAME disfrutan de una capacidad de almacenamiento dinámica dentro del Foundation Worlds Content Server, que depende de lo que tengan en su wallet. Las siguientes reglas rigen esta asignación:

* Cada Decentraland NAME que poseas te otorga 100 MB de capacidad de almacenamiento (así como un World).
* Cada parcela LAND de Decentraland que poseas te otorga 100 MB adicionales de capacidad de almacenamiento.
* Por cada 2.000 MANA que tengas en tu wallet, se otorgan 100 MB adicionales de capacidad de almacenamiento.

El espacio en el Foundation Worlds Content Server puede usarse para alojar escenas tan grandes como quieran los usuarios, utilizando los Decentraland NAMEs que poseen y el espacio combinado otorgado por sus activos conjuntos de Decentraland. Por ejemplo, un usuario con varios Worlds (otorgados por poseer varios NAMEs) y una capacidad de almacenamiento combinada de 500 MB puede optar por desplegar un World con un archivo de escena de 200 MB, otro con un archivo de escena de 200 MB y un tercero con un archivo de escena de 100 MB. Como alternativa, podría optar por desplegar un World con un archivo de escena de 300 MB y otro con un archivo de escena de 200 MB.

Puedes comprobar cuánto de tu presupuesto de almacenamiento se ha usado y cuánto queda en dos lugares:

* El **Manage** sección de Creator Hub. Haz clic en **View Details** para ver un desglose de cómo se suman tus activos de MANA, LAND y NAME.
* El **Worlds** pestaña de la [Builder](https://decentraland.org/builder/worlds), que también muestra cuánto espacio está usando cada uno de tus Worlds.

**Worlds de dominios ENS**

En cambio, los Worlds otorgados a partir de dominios ENS tienen un tamaño máximo fijo de archivo de escena de 36 MB por World, independientemente de los demás activos de Decentraland del usuario. Los usuarios con Worlds de dominios ENS no pueden aumentar su límite de tamaño de escena de World ENS comprando MANA o LAND adicionales.

Sin embargo, los Worlds otorgados por la propiedad de un dominio ENS sirven como el primer paso perfecto para hacer realidad la libertad creativa que ofrece Decentraland.

#### Diferencias con las escenas de LAND

Los Worlds ofrecen varias ventajas frente a las escenas de LAND:

* No necesitas poseer LAND: añade tantas parcelas como quieras a tu escena sin costo alguno (hasta 300x300)
* Acceso mediante enlace: los jugadores pueden visitar tu World directamente mediante una URL
* Más capacidad de almacenamiento: especialmente para los titulares de Decentraland NAME
* Control de acceso: puedes permitir que solo ciertos jugadores accedan a tu World

Consulta [Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para más información, o aprende sobre [publicar en Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/#publishing-to-worlds) y [configurar los ajustes del World](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#world-configuration).

## Escenas globales

Las escenas globales pueden transformar el paisaje ya existente de Decentraland, añadiendo capas de interactividad y jugabilidad. Estas son escenas que no están limitadas a ejecutarse solo en ciertas parcelas de LAND o en ciertos Worlds. Los jugadores las llevan consigo adondequiera que vayan.

### Wearables inteligentes

Los wearables inteligentes están vinculados a tokens Wearable. Estos se venden como NFTs y se compran en el [Marketplace](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Los Smart Wearables se activan cada vez que el jugador se pone el wearable asociado. Se desactivan si el jugador se quita el ítem, o también pueden desactivar manualmente la escena global mediante la UI. Además, la escena en la que está parado el jugador puede suspender cualquier Smart Wearable que esté activo en la escena.

* Aprende todo sobre [Creación de Wearables](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/).
* Aprende sobre [wearables inteligentes](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
