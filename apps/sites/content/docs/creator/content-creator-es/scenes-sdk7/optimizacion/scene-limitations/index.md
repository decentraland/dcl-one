> ¿Cuántas cosas puedo poner en mi escena?

# Limitaciones de la escena

Para mejorar el rendimiento en el metaverso, hemos establecido un conjunto de límites que toda escena debe seguir. Estos límites son por parcela. Cuanto más grande sea la escena, más altos serán estos límites.

Cuando trabajas con el [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/), puedes ver estadísticas sobre los recursos utilizados por los modelos 3D en tu escena, junto con los límites de tu escena.

![](/docs/_remote/ab87e37107598ee524831639.webp)

Puedes expandir este menú para ver los detalles.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 Consejo**: Para ver una tabla de referencia con todos los números específicos según la cantidad de parcelas, consulta:
> 
> [Tabla de referencia](https://docs.google.com/spreadsheets/d/1BTm0C20PqdQDAN7vOQ6FpnkVncPecJt-EwTSNHzrsmg/edit#gid=0)


## Reglas de limitación de la escena

A continuación se muestra la cantidad máxima de elementos que una escena puede renderizar al mismo tiempo:

> *n* representa la cantidad de parcelas que ocupa una escena.

* **Triángulos:** `n x 10000` Cantidad total de triángulos de todos los modelos de la escena.
* **Entidades:** `n x 200` Cantidad de entidades en la escena.
* **Mallas:** `n x 300` Cantidad de mallas en la escena.
* **Materiales:** `log2(n+1) x 20` Cantidad de materiales en la escena. Incluye los materiales importados como parte de los modelos.
* **Texturas:** `log2(n+1) x 10` Cantidad de texturas en la escena. Incluye las texturas importadas como parte de los modelos.
* **Altura:** `330 metros` Altura máxima de la escena, sin importar cuántas parcelas ocupe.

  > Advertencia: las entidades colocadas a más de 200 metros pueden sufrir problemas de sincronización en multiplayer. Mantén cualquier contenido con el que interactúen los jugadores o que se sincronice entre jugadores por debajo de esa altura.

  > Importante: para estos límites solo se cuentan las entidades que se están renderizando actualmente en la escena. Si tu escena cambia entre modelos 3D, lo que importa son los modelos renderizados en cualquier momento, no la suma total. Los avatares de los jugadores y cualquier objeto que un jugador traiga desde fuera de la escena tampoco cuentan para calcular estos límites.
* **Tamaño total del archivo:** En Genesis City -`15 MB por parcela - 300 MB máximo`. Para Worlds, consulta [tamaño del World](#world-size). Tamaño total de los archivos subidos al content server. Incluye modelos 3D y audio. No incluye archivos que no se suben, como los paquetes de node.js. Puedes ver la lista completa de archivos que se publican y sus tamaños antes de confirmar un deployment.
* **Cantidad de archivos:** `200 archivos por parcela` Cantidad total de los archivos subidos. Incluye modelos 3D y audio. No incluye archivos que no se suben, como los paquetes de node.js.
* **Tamaño máximo del archivo** `50 MB por archivo` Ningún archivo individual de ningún tipo en la escena puede superar los 50 MB. Las escenas pequeñas tienen restricciones adicionales porque el archivo no debe superar su límite de Tamaño total de archivos (por ejemplo, una escena de una sola parcela está limitada a 15 MB en total).


> [!WARNING]
> **📔 Nota**: los límites de tamaño de archivo y cantidad de archivos se aplican al hacer deployment; superarlos bloquea el deployment. Los demás límites (triángulos, entidades, mallas, materiales, texturas, altura) son límites suaves: el Creator Hub los muestra como advertencias, pero superarlos normalmente solo provoca una degradación del rendimiento en lugar de un fallo grave. Tómalos como recomendaciones firmes.



> [!NOTE]
> **💡 Consejo**: no todos los archivos de la carpeta de tu proyecto de escena cuentan para el límite de tamaño de archivo, solo los que se suben a los servidores. Todo el contenido de la *node\_modules* folder, que es muy grande, son dependencias que no se suben y, por lo tanto, no cuentan. Lo mismo aplica a cualquier archivo de la `/src` folder, ya que el código fuente no se sube.
> 
> Puedes listar cualquier otro archivo o carpeta que quieras excluir de la subida en el `.dclignore` archivo de tu proyecto. Ten en cuenta que `.dclignore` también se aplica a la vista previa local, así que no incluyas nada que tu escena realmente cargue. Consulta [El archivo dclignore](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-files/#the-dclignore-file).


## Optimizando

Consulta [Optimización del rendimiento](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/) para ver consejos sobre cómo puedes mantener tu escena por debajo de estos límites y hacer que funcione más fluidamente para los jugadores.


> [!NOTE]
> **📱 Móvil**: los límites estrictos de esta página se aplican a todos los clientes, pero los dispositivos móviles tienen menos margen que el escritorio. Procura mantenerte holgadamente por debajo de estos límites cuando apuntes a la [app móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), y prefiere menos triángulos, menos materiales y texturas más pequeñas.


## Límites de la escena

Al ejecutar una vista previa, cualquier contenido que esté ubicado fuera de los límites de la parcela se resalta en rojo al renderizarse. Si algún contenido está fuera de esos límites, esa parte de tu contenido no se renderizará cuando los jugadores visiten tu escena.

Si la punta de un objeto grande sale de los límites, esa punta se recortará del objeto.

Una escena de una sola parcela mide 16 metros x 16 metros. Si la escena tiene varias parcelas, las dimensiones varían según la disposición de las parcelas.

Es posible posicionar entidades bajo tierra, ya sea para ocultarlas o para que solo emerja una parte de ellas. Una escena no puede tener túneles que bajen por debajo de la altura predeterminada del suelo; los jugadores no pueden viajar por debajo de la `y = 0` altura.

## Limitaciones de shaders

Los modelos 3D usados en Decentraland deben usar shaders y materiales compatibles. Consulta [materiales de modelos 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/materials/) para ver una lista de shaders compatibles.

## Compresión de mallas


> [!WARNING]
> **📱 Las mallas comprimidas con Draco en el** [**cliente móvil**](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) **requieren optimización de assets.** Los modelos glTF/GLB que usan [Draco](https://google.github.io/draco/) mesh compression se cargan en la app móvil de Decentraland solo después de haber sido procesados a través del pipeline de optimización de assets. Si tus modelos comprimidos con Draco no se cargan en móvil, verifica que se hayan optimizado — o exporta tus modelos **sin** compresión Draco para evitar por completo ese paso. Otras formas de optimización de glTF (quantization, texture compression, mesh decimation) no se ven afectadas.


## Iluminación

Las condiciones de iluminación de la escena no pueden cambiarse para todos los jugadores desde la configuración predeterminada, aunque cada jugador individual es libre de cambiar sus propios ajustes de skybox desde la UI de Explorer.

## Restricciones de tamaño de texturas

Los tamaños de las texturas deben usar números de ancho y alto (en píxeles) que coincidan con los siguientes números:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> Esta secuencia está compuesta por potencias de dos: `f(x) = 2 ^ x` . 1024 es el número máximo que permitimos para el tamaño de una textura; las texturas más grandes se reducen mediante el proceso de conversión de asset bundle. Este es un requisito bastante común entre otros motores de renderizado; existe debido a optimizaciones internas de los procesadores gráficos.

El ancho y el alto no necesitan tener el mismo número, pero ambos deben pertenecer a esta secuencia.

**El tamaño recomendado para las texturas es 1024x1024**, hemos comprobado que este es el tamaño óptimo para transportarse a través de redes domésticas y para ofrecer experiencias razonables de carga/calidad.

Ejemplos de otros tamaños válidos:

```
32x32
64x32
512x256
512x512
1024x1024
```


> [!WARNING]
> **📔 Nota**: aunque a veces funcionan texturas de tamaños arbitrarios, también suelen renderizarse con errores y son más inestables. Recomendamos encarecidamente que todas tus texturas coincidan con estos tamaños.


## Tamaño del World

Decentraland [Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) tienen limitaciones diferentes, ya que se cargan como escenas individuales.

* Los Worlds publicados en Decentraland NAMEs obtienen presupuesto de almacenamiento que se comparte entre todos los Worlds propiedad de la misma wallet. Cada NAME que posees otorga `100 MB`, cada parcela de LAND otorga un adicional de `100 MB`, y por cada 2.000 MANA retenidos otorga otro `100 MB`. Puedes distribuir este presupuesto entre tus Worlds como quieras.
* Los Worlds publicados en dominios ENS tienen un límite de `36 MB` que no puede ampliarse.

Puedes comprobar cuánto de tu presupuesto se usa y cuánto queda en la **Administrar** sección del Creator Hub, o en la **Worlds** pestaña del [Builder](https://decentraland.org/builder/worlds). Si superas tu presupuesto (por ejemplo, después de vender o transferir assets), tienes 48 horas para liberar espacio antes de que tus Worlds queden inaccesibles.

Consulta [Límite de tamaño de Worlds](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/#size-limits) para más detalles.

Tamaño total de los archivos subidos al content server. Incluye modelos 3D y audio. No incluye archivos que no se suben, como los paquetes de node.js. Puedes ver la lista completa de archivos que se publican y sus tamaños antes de confirmar un deployment.

Todos los demás límites en Worlds son por parcela, incluidos triángulos, materiales, etc. Como añadir más parcelas a un World es gratis, puedes añadir hasta 300x300 parcelas (90.000 en total) a tu escena, y tener los límites correspondientes a esa cantidad de parcelas.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
