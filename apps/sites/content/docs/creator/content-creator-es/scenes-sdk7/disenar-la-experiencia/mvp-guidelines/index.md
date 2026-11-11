> Guías recomendadas para producir tu primera escena o experiencia MVP usando el SDK

# Guías MVP

El propósito de este documento es ayudarte a guiarte a través del proceso de construir las primeras iteraciones de escenas en Decentraland. Nos referiremos a estas como un Producto Mínimo Viable (MVP).

**Al crear el Producto Mínimo Viable (MVP) para tu escena, necesitas pensar en dos áreas de enfoque:**

1. La experiencia básica del usuario y la funcionalidad de tu proyecto.
2. La creación de un "pipeline" básico, o flujo de trabajo del equipo y sistema de gestión de contenido para construir tu experiencia y mejorarla iterativamente.

Un MVP no debe intentar demostrar todos los posibles resultados de cada posible experiencia. En cambio, un MVP debe ser la mejor primera impresión de tu experiencia que puedas crear usando el SDK de Decentraland.

Es importante considerar tus propias limitaciones, cómo planeas proporcionar contenido a tus usuarios y las expectativas de tus usuarios. Abordar tu MVP de esta manera requiere tres perspectivas diferentes:

1. Como desarrollador o productor, ¿cómo entrego una experiencia a mi usuario/jugador?
2. Como usuario o jugador, ¿qué espero de esta experiencia?
3. Como colaborador o parte interesada, ¿cómo contribuyo al pipeline o a la experiencia?

Es importante distinguir este enfoque del desarrollo ágil tradicional, porque puede que tengas que usar métodos no óptimos para cumplir tus objetivos de diseño.

Tendrás que examinar tus propios objetivos en el contexto de las expectativas de tus usuarios para decidir si un cierto lanzamiento está más enfocado en el jugador, en el pipeline y en los colaboradores de contenido, o en un poco de ambos.

Al planificar cada lanzamiento, es fundamental que establezcas conscientemente y deliberadamente tus prioridades según cada una de estas tres perspectivas.

Puedes esperar que tu backlog de desarrollo siga dos vías:

* El backlog de experiencias de usuario que quieres crear.
* El desarrollo de las herramientas e interfaces necesarias para construir tu pipeline de entrega. (O para optimizar tu pipeline existente tanto para los colaboradores como para tu equipo de desarrollo.)

Estas dos vías también seguirán dos enfoques diferentes para las pruebas:

* Probar tus experiencias de usuario se asemeja más a las pruebas tradicionales de interfaz de usuario y no requiere los mismos recursos de scripting.
* Probar tus herramientas e interfaces del pipeline requerirá más recursos técnicos.

Cuanto antes puedas presentar una propuesta de valor a tu usuario o jugador, antes podrás obtener feedback para confirmar o rechazar esa propuesta. Confirmar el valor rápidamente es fundamental. Muchos desarrolladores experimentados compartirán historias de cómo estaban absolutamente seguros de lo increíble que sería una nueva mecánica hasta que la usaron y se sintió torpe y con fallos, los jugadores no respondieron en absoluto o no resolvió una necesidad/deseo del consumidor. Quieres fallar rápido con el menor esfuerzo posible, para aprender de tu fallo y planificar la siguiente iteración.

¿Cómo fallas rápidamente? Haces lo mínimo necesario para que tu jugador interactúe con tu producto.

## Factores para Productos Mínimos Viables

Aquí está la lista de factores a considerar para tu MVP básico. Es aceptable indicar que usarás algo como marcador de posición y luego lo irás eliminando a medida que desarrolles un reemplazo más sólido.

1. Creación de arte
   * Primero, comienza con imágenes fijas básicas
   * Tu primera prueba debe ser de estilo: ¿el estilo que has elegido atrae a tus usuarios?
   * Esto podría ser el comienzo de una guía de estilo para proporcionar a un artista externo
2. Creación de la escena
   * Desarrolla una sensación básica de tu espacio
   * El jugador debe sentir que está en un espacio nuevo y único
   * Delimita tu espacio respecto a los espacios vecinos
   * Los bordes son evidentes y obvios – si no fuera por una línea dibujada
   * Cubre toda el área con contenido/arte estático
3. Arte renderizado en la escena
   * Usar billboards está bien u otra señalización (esto podría ser simplemente vallas publicitarias reales o sprites más sofisticados orientados hacia la cámara)
   * Establece el tono y la estética de tu espacio (es decir, estilo, luminoso, oscuro)
   * Anota tu proceso: ¿cómo se creó y desplegó el arte en la escena?
   * ¿Cómo quieres organizar tus archivos de arte para despliegues repetidos?
4. Experiencia del jugador
   * Los jugadores pueden visitar tu espacio/escena
   * Los jugadores pueden distinguir tu espacio de los espacios vecinos
5. Objetivos del pipeline
   * Desplegar escena estática de ejemplo: sin interacción con el jugador
   * Desplegar escena animada: elementos como fuentes de agua o banderas ondeando repiten sus animaciones
   * Desplegar escena interactiva: incluyendo la participación del jugador
   * Demostrar el pipeline de despliegue re-desplegando contenido: desde la creación del arte hasta su inclusión en la escena, incluyendo scripting + QA]
   * Exponer las brechas del pipeline: identificar los desconocidos en áreas específicas de despliegue de contenido

## Niveles de prototipos

Fallar rápidamente te permite desarrollar tu experiencia creando prototipos sucesivos, donde cada iteración se construye sobre la anterior.

**Empieza con un prototipo para un solo jugador. Luego puedes planificar las interacciones multijugador con scripting. Finalmente, puedes abordar tu bucle central persistente que demuestra capas transaccionales.**

**¿Qué es un bucle central persistente?**

En el diseño de juegos, un bucle central persistente es el "game loop" fundamental que impulsa las acciones del jugador y la respuesta del juego a esas acciones. Estos bucles persistentes se extienden a cualquier forma de experiencia virtual (como las proporcionadas por Districts).

**¿Qué son las capas transaccionales?**

Las capas transaccionales son las interfaces entre sistemas, como una actualización de la blockchain u otra aplicación que se ha integrado con tu experiencia para mantener un registro persistente de las acciones del jugador. Crear y mantener este registro persistente es lo que construye una experiencia más personal.

Recomendamos crear tu MVP como una experiencia de un solo jugador.

Por ejemplo, podrías diseñar una escena con las siguientes experiencias sucesivas:

* Un solo jugador puede entrar en el mundo.
* El jugador puede interactuar con una o dos entidades simples dentro de la escena.
* Otros jugadores pueden unirse e interactuar con el mundo y con el otro jugador.
* Por último, puedes agregar la capacidad de recordar que cada jugador ingresó a la escena, y de registrar los eventos y actividades de los jugadores.

## Cómo compartir tu MVP

Recomendamos probar los cambios con usuarios de prueba antes de subir una nueva versión de tu escena a Decentraland. Puedes publicar tu escena en un [World de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para compartirla mediante un enlace sin necesidad de ningún LAND, y sin afectar el contenido en producción.

## Consideraciones adicionales

Una vez cubiertos los casos de uso básicos, puedes empezar a sofisticar más tu estrategia de gestión de lanzamientos centrándote en las mecánicas. **Las mecánicas** son un término amplio que cubre todas las acciones que un jugador puede realizar y las respuestas que el sistema proporcionará en función de esas acciones del jugador.

**La interoperabilidad de dispositivos** es algo importante a tener en cuenta. Los usuarios de tu escena pueden estar accediendo a ella desde un escritorio, un dispositivo móvil o un casco de VR. Los usuarios deberían poder interactuar con tu escena razonablemente bien usando cualquiera de ellos. Para quienes usan un casco de VR, intenta evitar movimientos mareantes que puedan causar cinetosis.

**Audio** es otro aspecto crítico de la atmósfera de una escena. Sonidos de fondo como el viento, los grillos, conversaciones lejanas, quizá incluso música, pueden ser una forma muy poderosa de aumentar la inmersión y dar contexto. También puedes cambiar cómo los niveles de volumen se relacionan con la distancia a la fuente de sonido para dar más o menos énfasis a la ubicación de un sonido.

Lee [las restricciones de diseño para juegos](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games/) para una visión detallada de otras consideraciones.

Considera el MVP como uno de muchos prototipos que puedes usar para establecer tu cadencia de lanzamientos una vez que hayas establecido tu pipeline. El enfoque de cada lanzamiento puede variar, o puede ser un híbrido de cada aspecto de la experiencia. Sin embargo, debes aspirar a ofrecer experiencias cada vez más complejas, donde cada iteración se construye sobre la anterior.

1. **MVP**: Un solo jugador
2. **Lanzamiento 2**: Añadir soporte multijugador y/o de interacción
3. **Lanzamiento 3**: Introducir tu primera mecánica
4. **Lanzamiento 4**: Añadir soporte de audio
5. **Lanzamiento 5**: Finalizar tu pipeline de arte

Por ejemplo, supongamos que estamos construyendo un MVP para un juego de Frisbee golf. El MVP incluirá algunas imágenes fijas del recorrido. Incluso puede que el jugador pueda lanzar un disco, de una manera muy rudimentaria, estilo bloques. Esto nos permite definir nuestra mecánica básica de lanzamiento. El siguiente lanzamiento puede incluir un prototipo de soporte multijugador para que podamos demostrar y probar a dos usuarios conectados y jugando en nuestro LAND al mismo tiempo.

Recuerda que, aunque el objetivo final es un mundo 3D verdaderamente inmersivo, ahí no es donde comenzará tu MVP. Conseguir que un jugador entre en tu mundo lo antes posible debe ser tu primer objetivo. Tardar semanas, no meses, en probar tus lanzamientos es fundamental para aprender e iterar sin desperdiciar esfuerzo.

Te recomendamos encarecidamente que seas consciente de la primera impresión que presenta tu experiencia. Una experiencia vacía dejará decepcionados a los jugadores. Por otro lado, una escena con algo de contenido inicial y experiencias básicas muestra a los jugadores el potencial de lo que está por venir y los anima a interactuar con tu comunidad y volver para los siguientes lanzamientos.

## Factores de persistencia a considerar

En última instancia, quieres alcanzar un nivel de persistencia en el que puedas demostrar que las capas transaccionales de tu arquitectura están operativas. Transaccional no se limita a las acciones de los jugadores, sino también a las reacciones del sistema ante los jugadores.

1. **Información de la cuenta**: nombre de inicio de sesión, zona horaria, ubicación para tu experiencia/juego específico
2. **Estadísticas de la tabla de clasificación**: resultados previos de la partida, posiciones globales/regionales, competiciones
3. **Validación de identidad**: dirección de la Wallet de Ethereum, o cualquier otra gestión de identidad backend
4. **Actualizaciones de blockchain**: según sea necesario en función de tu experiencia/juego para actualizar el libro mayor de la blockchain para lograr transparencia transaccional
5. **Persistencia en tiempo de ejecución**: datos temporales para persistencia a través de una plataforma potencialmente distribuida (es decir, salud solo para la experiencia de juego individual)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/mvp-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
