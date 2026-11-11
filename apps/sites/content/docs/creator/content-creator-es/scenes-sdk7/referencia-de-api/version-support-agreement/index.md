> Acuerdo de soporte de versiones

# Acuerdo de soporte de versiones

Este documento describe la política de versionado aplicada tanto al Scene Editor en Creator Hub como al framework de scripting (el `@dcl/sdk` paquete).

El objetivo de esta política de versionado es establecer un contrato entre el equipo de SDK de la Fundación Decentraland y los creadores de contenido, para fijar expectativas claras y permitir que los creadores de contenido planifiquen su trabajo en consecuencia.

## Definiciones

* *Número de versión* -- un identificador único para una versión de software disponible públicamente. Este identificador consta de un *número de versión mayor* y un *número de versión menor* , separados por puntos (por ejemplo, 7.2).
* *Familia de versiones* -- todas las versiones que tienen la misma versión mayor forman una familia. Llamamos a la versión A *sucesora* de la versión B si A y B pertenecen a la misma familia y el número menor de A es mayor que el de B (por ejemplo, 7.11 es sucesora de 7.10).
* *Cambio incompatible* -- un cambio que obliga a un usuario a modificar su código o sus assets para que sigan funcionando. Por ejemplo, una propiedad cambia de nombre y obliga al usuario a cambiar ese nombre de propiedad cada vez que se usa en su código.
* *Cambio no incompatible* -- un cambio que no requiere ninguna acción por parte del usuario. El comportamiento y las propiedades del código y los assets del usuario permanecen sin cambios.
* *Versión estable* -- una versión que prohíbe cambios incompatibles en todos sus sucesores (que también se consideran estables). Cualquier cambio incompatible debe introducirse solo mediante la creación de una nueva familia de versiones incrementando el número de versión mayor; sin embargo, véase la salvedad en la sección de [cambios incompatibles](#breaking-changes) . Los cambios no incompatibles se reflejan incrementando el número de versión menor.
* *Versión inestable* -- una versión que permite cambios incompatibles en sus sucesores. Los cambios incompatibles pueden introducirse incrementando el número de versión menor.

## Política de soporte

En cada familia de versiones estables, la Fundación Decentraland solo da soporte a la última versión menor. En un momento dado, debe haber como máximo dos familias con soporte.

Por ejemplo, si la última versión menor de la familia 6.x es 6.11, y la última versión menor de la familia 7.x es 7.3, se espera que los creadores de contenido desarrollen sus escenas en 6.11 o 7.3. Es muy probable que las escenas que se desarrollaron y publicaron con la versión 6.10 o anterior sigan funcionando y que los jugadores puedan disfrutarlas. Sin embargo, si estas escenas antiguas presentan algún problema, primero deben actualizarse a una versión compatible, y el problema solo se investigará si sigue ocurriendo en una versión compatible.

El Scene Editor actualiza automáticamente el paquete de scripting de todas las escenas dentro de la misma familia de versiones, para que todos los desarrolladores usen la última versión compatible al desarrollar sus escenas.

## Desarrollo de funciones

Las nuevas funciones solo se lanzarán en la última versión en desarrollo. Tan pronto como el equipo de desarrollo empiece a trabajar en una nueva familia de versiones, las familias de versiones anteriores que sigan con soporte solo recibirán correcciones importantes de errores, y no se implementarán funciones adicionales.

## Cambios incompatibles

Los cambios incompatibles solo deben producirse en versiones mayores. No debe haber cambios incompatibles dentro de las versiones menores estables de la misma familia de versiones, salvo en caso de emergencia, cuando no haya otros medios para abordarlo. Los cambios incompatibles dentro de una versión menor son una medida drástica que los desarrolladores evitarán a toda costa. Las nuevas versiones menores ampliarán las capacidades de la sintaxis existente, pero nunca deben cambiar lo que produce la sintaxis establecida, salvo al corregir errores.

### Cambios aislados

En ocasiones muy raras, puede ser preferible hacer un pequeño cambio incompatible e aislado si solo va a incomodar a un pequeño subconjunto de usuarios. (Crear una nueva versión mayor incomoda a todos los usuarios). En este caso, el SDK podría marcar una función como obsoleta, pero debe seguir dándole soporte durante un tiempo razonable.

### Cambios de emergencia

En ciertos casos excepcionales, como problemas de seguridad o requisitos regulatorios, cualquier función puede modificarse de manera incompatible sin importar su nivel de estabilidad, y en estas situaciones no se promete una obsolescencia.

## Lanzamientos estables e inestables

### Alpha

Cada vez que se introduce un nuevo lanzamiento mayor, algunas de las primeras versiones menores pueden etiquetarse como **alpha** versiones inestables. Deben permitirse y esperarse cambios incompatibles en los lanzamientos alfa, y los usuarios no deben esperar estabilidad alguna.

Los desarrolladores son libres de experimentar con estas versiones alfa, pero no se les anima a publicar contenido creado con versiones alfa inestables, ya que no hay garantía de que el contenido siga funcionando tras cambios posteriores. Tampoco se recomienda iniciar migraciones grandes y complicadas en este punto, ya que podrían ser necesarios más cambios antes del próximo lanzamiento estable.

### Beta

Una vez que una familia de versiones alcanza un cierto nivel de madurez, se pone a disposición un **beta** lanzamiento. Un lanzamiento beta se considera completo y listo para ser declarado estable, sujeto a pruebas públicas.

Las familias de versiones beta deben ser lo más estables posible; sin embargo, se permite que cambien con el tiempo. Estos cambios deben ser mínimos, pero pueden incluir cambios incompatibles. Los cambios incompatibles solo pueden introducirse después de un período razonable de obsolescencia para dar a los creadores de contenido la oportunidad de migrar sus escenas. Este período de obsolescencia debe definirse cuando se introduce el cambio incompatible.

Las familias de versiones beta solo deben permanecer en beta durante un período limitado, especificado en el momento de marcarlas como beta. Deben promoverse a estables si no se encuentran problemas en ese período. La duración de este período puede variar según el caso, pero una buena regla general es 90 días.

El contenido escrito con la sintaxis de una versión beta debería ser fácil de migrar a versiones posteriores dentro de esa familia. Aun así, no es recomendable desarrollar contenido para eventos importantes con una versión beta, ya que las pruebas siguen en curso y es probable que haya errores.

### Estable

Una vez que el período beta termina sin problemas importantes, la familia de versiones se considera **estable** y no debería haber más cambios en la sintaxis, aparte de la adición de nuevas funciones. A partir de este momento, la versión se considera la opción recomendada y alentada para que la usen todos los desarrolladores.

Una familia de versiones estable debe contar con soporte completo durante todo su ciclo de vida. No debe haber cambios incompatibles, sujeto a las salvedades descritas a continuación.

## Funciones inestables en versiones estables

Algunas funciones concretas de una versión pueden tener niveles de estabilidad distintos del de la versión en su conjunto. Esto puede deberse a que la función se ha introducido recientemente y requiere más pruebas, o porque está destinada a ser reemplazada pronto.

Por ejemplo, podría introducirse como alfa un nuevo tipo de componente en una versión ya estable del framework de SDK, ya que este componente concreto aún puede requerir su propio ciclo de pruebas. Luego puede seguir el flujo de versionado descrito arriba, pasando de alfa a beta y luego a estable.

Cualquier función de una versión estable que no se considere estable debe estar claramente etiquetada como tal en la documentación. Los creadores que hagan uso de funciones inestables deben ser conscientes de que la función podría sufrir cambios incompatibles. Cualquier cambio incompatible se comunicará claramente, incluidas guías de migración, y habrá un período de transición para que los creadores ajusten el código de sus escenas.

## ¿Durante cuánto tiempo damos soporte a una familia de versiones estables?

Una vez que una nueva familia de versiones se vuelve estable (7.x), el equipo se compromete a dar soporte (correcciones importantes de errores) a la versión anterior (6.x) durante varios meses, para dar a los creadores tiempo de sobra para migrar. La duración se determina caso por caso, en función del esfuerzo de migración requerido.

## Versiones prepublicadas

Siempre es posible acceder a las adiciones más recientes del framework de scripting instalando la `@next` versión del `@dcl/sdk` paquete en una escena.

Las funciones de esta rama pueden ser inestables o no estar documentadas, ya que no se publican como parte de una versión oficialmente compatible del SDK.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/referencia-de-api/version-support-agreement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
