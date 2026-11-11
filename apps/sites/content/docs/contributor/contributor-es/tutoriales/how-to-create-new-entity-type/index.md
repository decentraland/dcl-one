# Cómo crear un nuevo tipo de entidad

#### ¿Cuándo es necesario crear una nueva entidad?

Primero, verifica si una entidad existente puede resolver el caso de uso necesario para evitar sobredimensionar la solución. Si ninguno de los tipos de entidad existentes definidos en <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13> satisface los requisitos, entonces puede que necesites crear un nuevo tipo de entidad.

#### Primera respuesta:

* ¿La nueva entidad es una colección de NFT?
* ¿Cómo se validará la propiedad?
* ¿Cuál será su tamaño?
* ¿Cuál es la cantidad de entidades que se crearán?

#### Pasos

Primero crea un ADR en <https://github.com/decentraland/adr> donde definirás el schema y la fecha para comenzar a validar la nueva entidad.

**Repositorios**

**@dcl/schemas**

1. Crea el nuevo tipo de entidad en el enum: <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L19>
2. Crea la carpeta y los archivos en <https://github.com/decentraland/common-schemas/tree/main/src/platform> generando el type y el esquema JSON, por ejemplo: <https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts#L20>

**@dcl/content-validator**

Implementa las validaciones definidas en el ADR, teniendo en cuenta la fecha de ese ADR en <https://github.com/decentraland/content-validator/blob/main/src/validations/timestamps.ts>. Por ejemplo: <https://github.com/decentraland/content-validator/blob/main/src/validations/items/emotes.ts#L8>

1. Agrega el código para verificar la propiedad del nuevo tipo de entidad en <https://github.com/decentraland/content-validator/tree/main/src/validations/access-checker>
2. Implementa el validationForType para el nuevo type, por ejemplo: <https://github.com/decentraland/content-validator/blob/main/src/validations/profile.ts#L112>
3. Aquí necesitarás definir el tamaño máximo por entidad en <https://github.com/decentraland/content-validator/blob/main/src/validations/ADR51.ts>.

**@dcl/urn-resolver**

1. Si la nueva entidad corresponde a una Collection, entonces no hay que agregar nada.
2. Si no, entonces crea un nuevo urn correspondiente a la nueva entidad y defínelo en <https://github.com/decentraland/urn-resolver> agregando un resolver en <https://github.com/decentraland/urn-resolver/blob/main/src/resolvers.ts#L23>

**Catalyst: Content Server**

1. No se necesita ningún cambio en la base de datos.
2. Actualiza todas las librerías modificadas detalladas arriba.

**Catalyst: Lambdas Server**

1. Actualiza todas las librerías modificadas detalladas arriba.
2. Comprueba si es necesario actualizar la sanitización del perfil (`/lambdas/profiles` endpoint)
3. Comprueba si es necesario actualizar el endpoint de colecciones ERC (`/lambdas/collections` endpoint)
4. Crea un endpoint para mostrar esa entidad, por ejemplo `/lambdas/wearables-by-owner`


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tutoriales/how-to-create-new-entity-type.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
