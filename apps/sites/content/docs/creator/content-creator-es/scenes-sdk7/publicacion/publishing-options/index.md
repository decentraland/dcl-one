> Cómo obtener permisos de Decentraland World o LAND para publicar tu escena

# Opciones de publicación

Para invitar a otras personas a visitar una escena que creaste, tendrás que publicarla en Decentraland. Para hacerlo, necesitarás tener permisos de deploy en un Decentraland World o ser propietario de parcelas de Decentraland LAND.

Este documento ofrece una visión general de las diferentes formas de obtener permisos. Consulta [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para obtener instrucciones sobre cómo publicar una vez que los hayas obtenido.

### Decentraland Worlds

Decentraland Worlds son tu propio espacio 3D personal en el ecosistema de Decentraland, separado del mapa público de parcelas de LAND de Genesis City.

Un World puede mantenerse privado o compartirse con cualquier persona con solo un enlace. Puede alojar hasta 100 usuarios concurrentes, y puedes usar tu World para:

* Alojar eventos
* Crear nuevas experiencias
* Compartir un portafolio de escenas
* Probar escenas antes de deployarlas en LAND de Genesis City

### Cómo obtener un World

Hay dos formas de obtener un Decentraland World:

* **Obtén un Decentraland NAME (recomendado).**\
  Puedes obtener tu propio Decentraland World obteniendo un [Decentraland NAME](https://builder.decentraland.org/names), que son NAME NFTs dentro del ecosistema de DCL. Un NAME cuesta 100 MANA y te da inmediatamente un World con ese nombre. Puedes [comprar un nuevo NAME](https://decentraland.org/shop/items?category=names) o comprar uno existente a un propietario anterior en el [Marketplace](https://decentraland.org/marketplace/names).
* **Compra un dominio ENS.**\
  También puedes obtener un World comprando un [dominio ENS](https://ens.domains/), un nombre descentralizado que puede usarse tanto en Web2 como en Web3. Los Worlds otorgados a partir de dominios ENS tienen un tamaño máximo fijo de archivo de escena de 36 MB por World, independientemente de tus otras participaciones en Decentraland.

Una vez que tengas un NAME o un dominio ENS, puedes publicar escenas en ese World usando el flujo estándar de [Publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) .\
También puedes permitir que otras personas publiquen en tu Decentraland World. Consulta la [documentación de Managing Worlds](/docs/creator/content-creator-es/scene-editor/publicar/publish-scene/#managing-worlds) para obtener más información sobre Multi Scene Worlds y otros temas como Private Worlds.

### Capacidad de almacenamiento de Worlds

Dentro de un World, puedes usar tantas parcelas como quieras. Para los titulares de Decentraland NAME, el límite de tamaño de tu escena en MegaBytes depende de cuántos NAMES, MANA y parcelas de LAND poseas. Consulta [límites de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits) para obtener información detallada sobre cómo se calcula la capacidad de almacenamiento.

Puedes consultar tu presupuesto de almacenamiento usado y restante en la sección **Administrar** de Creator Hub, o en la pestaña **Worlds** de [Builder](https://decentraland.org/builder/worlds).

### Obtén más información sobre Worlds

* [Worlds como tipo de proyecto](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#publish-to-worlds) - Aprende sobre los límites de tamaño y en qué se diferencian Worlds de las escenas de LAND
* [Publicación en Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/#publishing-to-worlds) - Instrucciones de publicación paso a paso
* [Configuración del World](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#world-configuration) - Configura el skybox, los ajustes de comunicación y más
* [Administración de Worlds](/docs/creator/content-creator-es/scene-editor/primeros-pasos/manage-scenes/#managing-worlds) - Visualiza el espacio de almacenamiento y administra los deploys
* [Preguntas frecuentes sobre Worlds](https://github.com/decentraland/docs/tree/main/creator/scene-editor/faq/README.md) - Preguntas comunes sobre Worlds

### Publicación en Decentraland LAND

Genesis City es el espacio compartido de Decentraland: el gran mapa público compuesto por parcelas de LAND. Cada parcela de LAND mide **16 metros × 16 metros**. Una escena se puede construir en una o más parcelas.

Si prefieres publicar en LAND, hay tres opciones:

* **Alquilar LAND**: Compra el derecho a deployar y mantener tu contenido en ciertas parcelas durante un período de tiempo fijo. Consulta [Alquileres](/docs/player/marketplace/rentals/).
* **Comprar tierra**: Compra parcelas de tierra y publica tu contenido durante el tiempo que quieras. Consulta [marketplace](/docs/player/marketplace/marketplace/).
* **Obtener permisos de un propietario**: Varios propietarios de tierra y distritos gestionan grandes extensiones de tierra y están dispuestos a alojar gratuitamente contenido de otras personas si encaja con su tema y el tipo de entorno que quieren crear. Ponte en contacto con ellos en el [servidor de Discord de Decentraland](https://dcl.gg/discord).

### Cómo funcionan los permisos de LAND

Los tokens de LAND pueden gestionar permisos que permiten los siguientes tipos de roles para hacer deploy en una parcela o estate en particular.

* **Propietario de LAND**: La persona que posee el token en su wallet de Ethereum. Este usuario siempre puede hacer deploy, asignar otros roles o transferir el token (a menos que la tierra esté alquilada actualmente por otra persona).
* **Operador de LAND**: El operador solo puede hacer deploy mientras tenga este rol. La dirección del operador es referenciada por el token, sin necesidad de poseer nada. El propietario puede asignar y revocar permisos de operador a cualquier número de direcciones. Consulta [Dar permisos](/docs/player/marketplace/land-manager/#give-permissions).

Cuando se alquila una tierra, el inquilino que paga el alquiler puede elegir una única dirección que tendrá un **rol de Operador de LAND** durante la duración del alquiler (puede ser su propia dirección o la de otra persona). Mientras el alquiler esté activo, ninguno de los titulares originales de los roles puede interferir con el contenido de ese LAND ni transferir el token.

### Cómo publicar tu escena

Cuando hayas obtenido tu World o tus permisos de LAND, sigue la guía estándar de [Publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para deployar tu escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
