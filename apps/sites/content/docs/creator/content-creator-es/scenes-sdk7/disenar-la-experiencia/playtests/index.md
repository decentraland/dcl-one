> Cómo realizar pruebas de juego de tu juego de Decentraland y convertir los comentarios de los jugadores en mejoras

# Pruebas de juego

Las pruebas de juego te ayudan a hacer mejores juegos al descubrir cómo los experimentan los jugadores reales.

No necesitas un juego terminado ni un gran grupo de testers para empezar. Puedes probar temprano, hacer mejoras y volver a probar a medida que tu juego evoluciona.

Esta guía explica cómo organizar playtests, recopilar comentarios de los jugadores y usar lo que aprendas para mejorar tu juego.

## ¿Por qué hacer playtest de tu juego?

Cuando construyes un juego, ya sabes cómo funciona. Los nuevos jugadores no.

Hacer playtesting te ayuda a descubrir cosas que de otro modo podrías pasar por alto:

* **Claridad:** ¿Los jugadores entienden qué hacer?
* **Facilidad de uso:** ¿Pueden los jugadores completar acciones sin atascarse?
* **Diversión:** ¿Qué partes son divertidas y cuáles son frustrantes?
* **Motivación:** ¿Los jugadores quieren seguir jugando o volver?

**Empieza a hacer playtesting lo antes posible.** Puedes aprender mucho observando a unas pocas personas probar tu primera versión jugable.

## Elige lo que quieres probar

Antes de cada playtest, decide qué quieres aprender.

Concéntrate en una o dos preguntas en lugar de intentar probar todo a la vez.

| Tu objetivo                   | Qué buscar                                                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Probar onboarding             | ¿Pueden los nuevos jugadores entender qué hacer sin ayuda?                                                                         |
| Probar la mecánica principal  | ¿Pueden los jugadores usar la mecánica principal del juego? ¿Es divertida?                                                         |
| Probar el bucle del juego     | ¿Entienden los jugadores cómo avanzar y obtener recompensas?                                                                       |
| Probar la dificultad          | ¿Dónde se atascan, se aburren o se frustran los jugadores?                                                                         |
| Probar la respuesta emocional | ¿Los jugadores se sienten entusiasmados, curiosos o con ganas de seguir jugando? ¿Qué momentos generan las reacciones más fuertes? |

Elige jugadores que no estén familiarizados con tu juego siempre que sea posible. Es más probable que encuentren problemas que los jugadores experimentados ya no notan.

### Adapta el playtest a la etapa de tu juego

Decide qué tipo de respuesta estás buscando. Al principio, sobre todo necesitas saber si los jugadores **entienden** el juego y pueden formar una **estrategia**: ¿saben qué hacer y pueden descubrir cómo hacerlo bien? Una vez que eso esté sólido, necesitas saber cómo les hace sentir el juego **sientan**: ¿es emocionante, satisfactorio o tenso en los momentos adecuados, y quieren volver?

El tipo de jugador que invites también cambia según la etapa. Alguien que esté cómodo con Decentraland pasará el client y entrará directamente en tu juego, así que es ideal cuando estás probando mecánicas. Un principiante total tropieza con todo, que es exactamente lo que quieres una vez que estás probando la experiencia completa de primera vez.

| Etapa                      | Lo que quieres aprender                                                                                      | A quién invitar                                                                                |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| V0 (vertical slice)        | ¿La mecánica principal es comprensible y usable? ¿Pueden los jugadores formar una estrategia a su alrededor? | Algunos jugadores que ya conocen Decentraland, para que el client no interfiera con la prueba. |
| V1 (primer bucle completo) | ¿Pueden los jugadores avanzar por su cuenta? ¿Dónde se atascan o pierden interés?                            | Una mezcla de jugadores de Decentraland y personas que nunca han jugado tu juego.              |
| V2 y más allá              | ¿Cómo hace sentir el juego a los jugadores? ¿Vuelven? ¿Lo recomendarían?                                     | Nuevos jugadores, incluidas personas de fuera de la comunidad de Decentraland.                 |

Escribe tus preguntas según la etapa en la que estás. Preguntar "¿te divertiste?" sobre un slice V0 te dice poco, porque todavía no hay suficiente juego para que la respuesta signifique mucho. Preguntar "¿entendiste qué hacer?" sobre un V2 que ya ha pasado por varias rondas desperdicia una sesión en algo que ya sabes.

## Recopila comentarios automáticamente con Scene Feedback

Scene Feedback es un módulo reutilizable que te permite hacer preguntas a los jugadores directamente dentro de tu juego de Decentraland.

Te ayuda a entender cómo experimentan los jugadores tu juego, incluso cuando no estás allí para verlos jugar.

Puedes:

* Hacer preguntas a los jugadores en momentos específicos durante el gameplay.
* Recopilar calificaciones de 1 a 5 y comentarios escritos.
* Descargar las respuestas como un archivo CSV desde el Scene Storage de tu World.

### Lo que necesitas primero

Scene Feedback almacena las respuestas en un servidor, así que tu escena necesita un [Servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/), habilitado mediante el uso de `@dcl/sdk@auth-server` y configurando `"authoritativeMultiplayer": true` en `scene.json`.

### Instala Scene Feedback

1. Abre el [repositorio de Scene Feedback](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback).
2. Sigue el README para instalar el módulo en tu escena. También puedes darle la indicación de instalación a tu agente de codificación con IA.
3. Elige un pequeño conjunto de preguntas que coincida con los objetivos de tu playtest.
4. Decide cuándo debe aparecer cada pregunta.
5. Descarga las respuestas de los jugadores como un archivo CSV. En Creator Hub, abre **Manage**, luego el **⋮** menú, luego **View Storage**. Las respuestas pueden estar repartidas entre varios archivos durante un tiempo (guardados más recientes que aún no se han fusionado, más un archivo único antiguo de versiones anteriores del módulo). Descarga todos los archivos cuyo nombre comience con el mismo `scene-feedback-<number>`, y elimina las filas con duplicados de `id`s.

El repositorio incluye preguntas listas para usar, ejemplos y orientación para elegir preguntas.

Si usas un asistente de codificación con IA, instala o actualiza los [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/). Incluyen una skill de Scene Feedback que puede ayudarte a decidir qué preguntar y cuándo, instalar el módulo y analizar las respuestas después:

```bash
npx skills add decentraland/sdk-skills --all
```

## Organiza tu playtest

Puedes organizar un playtest de dos maneras.

**Opción 1: Organiza un playtest en vivo**

Invita a todos a jugar al mismo tiempo. Puedes observar a los jugadores, comentar sus experiencias y recopilar comentarios después.

También puedes crear un evento en [Decentraland Events](https://decentraland.org/events/) para que los jugadores puedan encontrar y unirse a tu playtest.

**Opción 2: Realiza un playtest durante varios días**

Deja que los jugadores prueben tu juego cuando estén disponibles durante un período determinado.

Instala Scene Feedback para recopilar respuestas mientras juegan.

Esto es útil cuando quieres comentarios de más jugadores sin organizar una sesión en vivo.

## Organiza un playtest en vivo

Un playtest en vivo te permite ver cómo los jugadores experimentan tu juego y hacerles preguntas después.

### Paso 1. Prepara tu juego

Asegúrate de que los jugadores puedan acceder a tu juego y completar las acciones que quieres probar. Prepara dos o tres tareas para que los jugadores las intenten.

Por ejemplo:

* Descubre cómo empezar el juego.
* Completa una ronda.
* Compra una mejora.

### Paso 2. Invita a playtesters

Invita a un pequeño grupo de jugadores. De tres a cinco jugadores es un buen punto de partida. Intenta incluir personas que no hayan jugado tu juego antes.

Puedes encontrar jugadores interesados en el canal #playtesters de [Discord de Decentraland](https://discord.gg/decentraland). Consulta [Encontrar playtesters](#find-playtesters) para otros lugares donde buscar.

### Paso 3. Observa a los jugadores jugar

Pide a los jugadores que prueben el juego sin explicar cómo funciona todo. Observa qué hacen, dónde se atascan y qué disfrutan.

**No guíes a los jugadores a menos que no puedan continuar.** Si tienes que explicar algo, eso puede ser una señal de que tu juego necesita instrucciones más claras.

### Paso 4. Pide comentarios

Después de jugar, haz algunas preguntas abiertas:

* ¿Qué disfrutaste más?
* ¿Hubo algo confuso o frustrante?
* ¿Hubo un momento en el que no supiste qué hacer?
* ¿Volverías a jugar? ¿Por qué sí o por qué no?
* ¿Recomendarías este juego a un amigo? ¿Por qué sí o por qué no?

Evita preguntas dirigidas como "¿Te gustó el nuevo sistema de mejoras?" Estas pueden influir en las respuestas que recibes.

Si no estás seguro de qué preguntar, la skill de Scene Feedback mencionada arriba puede guiarte para elegir un enfoque y redactar las preguntas, tanto si planeas hacerlas en persona como dentro del juego.

Anota los problemas que experimentaron los jugadores, no solo sus sugerencias.

## Mejora tu juego con el método RITE

RITE significa **Pruebas e Evaluación Rápidas e Iterativas**.

Es un enfoque sencillo para el playtesting: encuentra un problema, arréglalo y vuelve a probar lo antes posible. En lugar de hacer un gran playtest y recopilar una larga lista de problemas, realizas varios pequeños y corriges los problemas más claros entre uno y otro.

### Sigue el ciclo RITE

1. **Probar:** Observa a un pequeño grupo de jugadores probar tu juego. De tres a cinco jugadores por ronda es suficiente.
2. **Identificar:** Encuentra problemas que afecten su experiencia.
3. **Arreglar:** Entre rondas, haz un cambio cuando tanto el problema como la solución estén claros.
4. **Repetir:** Realiza la siguiente ronda con nuevos jugadores. Continúa hasta que los jugadores puedan completar la experiencia sin problemas graves.

Por ejemplo, si los jugadores no pueden encontrar dónde empezar tu juego, haz que el botón Start sea más claro antes del próximo playtest. Luego observa si los nuevos jugadores pueden encontrarlo sin ayuda.

**Mantén el juego estable mientras una ronda está en curso.** RITE significa corregir entre rondas, no durante ellas. Si haces un redeploy mientras los jugadores están en medio de la sesión, puedes romper su juego y ya no podrás saber a qué versión se refiere un comentario. Recopila todo de una ronda, luego haz tus cambios y después vuelve a probar.

### Haz que RITE funcione en Decentraland

Algunos hábitos hacen que RITE sea práctico para una escena de Decentraland:

* **Corrige en pequeño, corrige rápido.** Entre rondas, cambia solo aquello que la ronda anterior mostró claramente como un problema. Si una solución necesita una mecánica nueva o un rediseño, déjalo para más adelante y mantén los cambios de una ronda a otra lo suficientemente pequeños como para publicarlos en un día.
* **Usa un World para probar.** Publica tus builds de prueba en un [World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/) en lugar de en tu LAND en vivo o en tu World principal. Los jugadores siempre llegarán a un build conocido, y puedes volver a publicar entre rondas sin tocar un juego ya publicado.
* **Mantén las mismas preguntas de Scene Feedback en todas las rondas.** Si cambias la redacción de una pregunta, no podrás comparar las calificaciones de una ronda a la siguiente. Añade preguntas si lo necesitas, pero no reescribas las que estás siguiendo.
* **Decide qué cuenta como "claro".** Un problema es claro cuando viste que ocurrió, puedes decir qué esperaba el jugador en su lugar y sabes qué cambiar. Si solo un jugador lo sufrió y no estás seguro de por qué, déjalo para la siguiente ronda y mira si vuelve a ocurrir.
* **Registra cada ronda.** Guarda una nota breve por ronda: qué build, quién jugó, qué viste, qué cambiaste. Para la tercera ronda, este registro te dirá si un cambio realmente funcionó.

RITE funciona especialmente bien durante el desarrollo temprano, cuando puedes hacer cambios rápidamente.

## Encontrar playtesters

Los jugadores habituales de Decentraland son los más fáciles de contactar, pero también busca [fuera de la comunidad de Decentraland](#outside-the-decentraland-community): detectan problemas que los jugadores experimentados pasan por alto.

### En Discord

El canal #playtesters en [Discord de Decentraland](https://discord.gg/decentraland) está para personas que disfrutan probar juegos y ayudar a los creadores a mejorarlos. Únete al servidor y busca el canal.

### Publica tu playtest

Cuando estés listo, publica un mensaje en #playtesters que incluya:

* **Juego:** El nombre de tu juego y una breve descripción.
* **Ubicación:** Un enlace a tu juego en Decentraland.
* **Cuándo:** La fecha y hora de tu playtest, o el período en que los comentarios estén abiertos.
* **Qué probar:** Las funciones o el gameplay específicos sobre los que quieres recibir comentarios.
* **Cómo dar feedback:** Mediante discusión en vivo, Discord o Scene Feedback.
* **Recompensas:** Cualquier incentivo que ofrezcas por participar.

### Anima a los jugadores a participar

Considera ofrecer una pequeña recompensa para agradecer a los jugadores su tiempo.

Por ejemplo:

* Regala a los participantes un Wearable o Emote especial.
* Ofrece una recompensa exclusiva dentro del juego. El ejemplo de Scene Feedback incluye una receta que muestra una **Playtest Contributor** etiqueta sobre el avatar del jugador con [`AvatarNametag`](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-nametags/) una vez que respondan a todas las preguntas de un formulario. La etiqueta solo es visible para ese jugador y dura durante su visita. Consulta [Una recompensa local de la escena por participar](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0%2C-1-scene-feedback/EXAMPLES.md#6-a-scene-local-reward-for-participation).
* Da Créditos de Marketplace a los jugadores elegibles.

**Regalar Wearables y Emotes.** Puedes dar cualquier Wearable o Emote que poseas a otro jugador directamente desde el cliente de escritorio: haz clic derecho sobre el jugador y elige **Regalar**. El ítem se transfiere a su cuenta y aparece en su Backpack. También puedes transferir un ítem que poseas desde el [Marketplace](https://decentraland.org/marketplace/) usando la dirección de wallet del destinatario. Si creaste la colección de Wearable o Emote, puedes mintear ítems directamente a cualquier dirección de wallet desde el Builder, incluso si no te encuentras con el jugador dentro del mundo. Consulta [Minting Wearables](/docs/creator/content-creator-es/wearables-y-emotes/publicacion-de-colecciones/publishing-collections/#minting-wearables).

**El envío de Marketplace Credits está actualmente en beta y disponible para estudios invitados.** Los estudios elegibles pueden regalar Credits a los jugadores, quienes pueden gastarlos en la Decentraland Shop. Consulta [Regalar Credits (Beta)](/docs/creator/content-creator-es/recompensas/gifting-credits/).

Las recompensas pueden incentivar la participación, pero deja claro que quieres comentarios genuinos, incluida la crítica.

### Fuera de la comunidad de Decentraland

También puedes invitar a personas que nunca han usado Decentraland. Los jugadores de Decentraland ya saben cómo funciona el client, así que irán directamente a tu juego. Las personas de fuera ven todo con ojos nuevos, incluidas las partes que has dejado de notar. Hacer playtesting con ellas también es una buena manera de presentar Decentraland a nuevos jugadores.

Los desarrolladores indie suelen reclutar entre:

* **Amigos y familiares.** Son fáciles de contactar y estarán encantados de darte una hora. Pídeles que sean honestos y que te digan cuándo se confunden.
* **Personas que normalmente no juegan.** Si pueden superar tu onboarding, casi cualquiera puede.
* **Comunidades de las que ya formas parte.** Un servidor de game jam, un foro de aficionados, un encuentro local o un grupo que coincida con el tema de tu juego.

Si tus testers son nuevos en Decentraland, ayúdalos a entrar: comparte el enlace web de tu escena (`https://decentraland.org/jump/?realm=<your-world-name>.dcl.eth` para un World, o `https://decentraland.org/jump/?position=<x>,<y>` para LAND), que Creator Hub muestra después de publicar, además de una breve nota sobre instalar el client e iniciar sesión. El enlace se abre en un navegador incluso antes de que el client esté instalado. Toma notas sobre dónde tienen dificultades antes de llegar a tu escena, pero trátalo por separado de los comentarios sobre tu juego.

## Usa los comentarios para mejorar tu próxima versión

Después de cada playtest, revisa lo que aprendiste antes de decidir qué construir después.

1. **Agrupa los problemas similares.** Busca problemas que hayan experimentado varios jugadores.
2. **Prioriza los problemas importantes.** Corrige cualquier cosa que impida a los jugadores entender, avanzar o disfrutar del juego.
3. **Haz mejoras.** Empieza con cambios que sean rápidos de implementar y que probablemente ayuden.
4. **Prueba de nuevo.** Comprueba si los nuevos jugadores tienen una mejor experiencia.
5. **Haz seguimiento de tus cambios.** Lleva un breve registro de lo que cambiaste y de lo que ocurrió en el siguiente playtest.
6. **Comparte lo que cambió.** Haz saber a tus playtesters lo que aprendiste y lo que mejoraste gracias a sus comentarios. Publica una actualización en tu hilo original de Discord para que puedan ver el impacto de su contribución.

Recuerda que lo que los jugadores **hacen** puede ser diferente de lo que **dicen**. Usa observaciones en vivo, respuestas de Scene Feedback y datos de gameplay juntos siempre que sea posible.

## Comparte tus mejoras con los playtesters

Es más probable que los jugadores participen de nuevo cuando ven que sus comentarios marcaron una diferencia. Después de hacer cambios, vuelve a tu publicación de playtest en #playtesters y comparte una breve actualización.

Usa este formato sencillo: **Tú dijiste, aprendimos, cambiamos.**

### Ejemplo: resultados del playtest

**Actualización del playtest: \[Nombre del juego]**

¡Gracias a todos los que probaron nuestro juego!

**Dijiste:** No estaba claro cómo empezar la primera ronda.

**Aprendimos:** Los nuevos jugadores no podían encontrar fácilmente el botón Start.

**Cambiamos:** Hemos hecho el botón más grande y añadido instrucciones más claras.

**Juega de nuevo:** \[Enlace a tu juego actualizado]

No necesitas corregir cada problema antes de publicar una actualización. También puedes explicar en qué sigues trabajando o qué te gustaría probar después.

## Sigue haciendo playtesting

El playtesting no es algo que haces una sola vez antes del lanzamiento.

Prueba cuando introduzcas una nueva mecánica, cambies el onboarding, ajustes la dificultad o añadas contenido nuevo. Incluso los playtests pequeños pueden ayudarte a descubrir problemas importantes antes de que afecten a más jugadores.

**El objetivo no es demostrar que tu juego funciona. Es descubrir cómo hacerlo mejor.**

## Recursos

* [Módulo Scene Feedback y guía de instalación](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)
* [Discord de Decentraland](https://discord.gg/decentraland)
* [Decentraland Events](https://decentraland.org/events/)
* [Método RITE: Rapid Iterative Testing and Evaluation](https://handbook.gitlab.com/handbook/upstream-studios/experience-research/rite/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/playtests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
