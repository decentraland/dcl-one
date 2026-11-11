> La DAO permite a los usuarios crear y votar propuestas que dan forma al metaverso.

# Qué puedes hacer con la DAO

El DAO permite dos tipos generales de propuestas: **propuestas con acciones vinculantes directas**, y **propuestas de gobernanza**.

## Propuestas con acciones vinculantes directas

El DAO permite a la comunidad votar sobre **acciones vinculantes** que darán lugar a cambios realizados en los smart contracts de Decentraland en la red de Ethereum. Esas acciones vinculantes son:

* Financiar un proyecto comunitario transfiriendo una parte de los recursos del DAO a un contrato de vesting de subvenciones.
* Agregar un nodo Catalyst a la red de servidores que alojan y ejecutan el mundo virtual de Decentraland.
* Agregar o eliminar puntos de interés (POIs), o ubicaciones destacadas dentro del mundo virtual, a una lista que se muestra a los usuarios. Esta lista ayuda a los usuarios a encontrar ubicaciones populares e interesantes para explorar.
* Prohibir un nombre en Decentraland. Este tipo de propuesta permite a los usuarios asegurarse de que a los avatares no se les puedan asignar nombres ofensivos y dañinos.

## Propuestas de gobernanza

Algunas propuestas no son tan simples como agregar o eliminar un elemento de una lista; requieren señalización de la comunidad, discusiones y rutas de implementación. Esas propuestas deben presentarse a través de un proceso de gobernanza de tres etapas que comienza con una encuesta y termina con una propuesta vinculante.

El proceso de votación incluye tres pasos: una encuesta previa a la propuesta, una propuesta de borrador y una propuesta de gobernanza. Cada nivel tendrá umbrales de presentación y aprobación progresivamente más altos para garantizar que las decisiones importantes de gobernanza sean tomadas por una mayoría representativa (basada en el poder de voto). Cada paso debe alcanzar el umbral de VP definido para ser promovido al siguiente.

### Etapa 1: Encuesta de prepropuesta

* Umbral de presentación: 100 VP
* Umbral de aprobación: 500K VP (Una encuesta que alcance al menos 500K VP y no obtenga una mayoría del poder de voto participante aún puede avanzar a la etapa de Propuesta Borrador, lo que garantiza que todos los temas con suficiente apoyo tengan una vía inicial para convertirse en política.)
* Período de votación: 5 días
* Objetivo: Introducir un tema de gobernanza a la comunidad, medir el sentimiento de la comunidad y determinar si hay suficiente apoyo para seguir adelante con la redacción de una propuesta inicial.

### Etapa 2: Propuesta Borrador

* Umbral de presentación: 1,000 VP
* Umbral de aprobación: 1M VP y mayoría simple (51%) del poder de voto participante (Una Propuesta Borrador que no tenga éxito o no alcance este umbral puede ser modificada y reenviada una vez.)
* Período de votación: 1 semana
* Objetivo: Presentar una posible política a la comunidad en un formato estructurado y formalizar la discusión sobre los posibles impactos y las vías de implementación de la propuesta. Una Propuesta Borrador que no tenga éxito o no alcance este umbral puede ser modificada y reenviada una vez.

### Etapa 3: Propuesta de gobernanza

* Umbral de presentación: 2,500 VP
* Umbral de aprobación: 6M VP y mayoría simple del VP participante (o los criterios de aceptación requeridos para su categoría)
* Período de votación: 2 semanas
* Objetivo: Formalizar la versión aprobada de una Propuesta Borrador en un resultado de gobernanza vinculante.

Es importante señalar que cualquiera que cumpla el umbral de presentación puede llevar una propuesta aprobada (ya sea una Encuesta o un Borrador) a la siguiente etapa, no solo el autor de la propuesta.

## ¿Y qué pasa con la modificación de los smart contracts de LAND o Estate?

Ahora mismo, el DAO posee tanto los smart contracts de LAND como los de Estate. Cualquier modificación de cualquiera de estos contratos debe ser realizada por la Security Advisory Board (SAB) y el Comité del DAO — grupos de personas de confianza y elegidas, encargadas de garantizar la seguridad continua de estas importantes piezas de la infraestructura de Decentraland.

El SAB debe votar a través de una wallet multi-sig para aprobar cualquier cambio en los contratos de LAND o Estate, evitando que un miembro rebelde del SAB introduzca una vulnerabilidad.

Actualmente, no hay categorías de propuesta predefinidas para modificar los smart contracts de LAND o Estate a través de la UI del DAO. Eso no significa que sea imposible que un miembro que no pertenezca al SAB inicie cambios, pero es un proceso más largo que requiere obtener apoyo de la comunidad en general, además de contar con la experiencia técnica necesaria para aportar los cambios de código y con un tercero de confianza que audite esos cambios.

El código fuente de los contratos de LAND y Estate está disponible en GitHub [aquí](https://github.com/decentraland/land/tree/master/contracts).

En términos generales, el proceso para modificar cualquiera de los dos contratos sería:

* Consultar a la comunidad mediante la publicación por parte del DAO de una **encuesta de prepropuesta**, y discutir públicamente los cambios propuestos para obtener apoyo
* Después de obtener el apoyo inicial de la comunidad, publicar una **propuesta de borrador** detallando los cambios y obteniendo aceptación mediante una votación.
* Escribir el código actualizado para que se fusione en el contrato
* Obtener una revisión de código y una auditoría exitosas de un tercero de buena reputación
* Presentar el código auditado a la comunidad usando una propuesta vinculante **propuesta de gobernanza** y obtener su aprobación para que se fusione con el contrato
* Una vez aprobado por otra votación de la comunidad, el Comité del DAO o el SAB realizarían la actualización del contrato con el nuevo código


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/dao/dao/what-can-you-do-with-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
