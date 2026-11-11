> La programación orientada a datos es un enfoque potente para programar que saca el máximo partido del rendimiento.

# Programación orientada a datos

La Programación Orientada a Datos es un enfoque poderoso para programar que da como resultado grandes mejoras de rendimiento. Se centra en tratar *los datos* como el elemento central; todo lo demás se organiza a su alrededor, para acceder a esos datos o modificarlos. Este enfoque también es muy favorable para el modo multijugador, ya que hace que los datos que deben sincronizarse entre jugadores sean más fáciles y rápidos de acceder.

La Programación Orientada a Datos te anima a pensar en todo en tu escena como datos que deben copiarse y mutarse a través de los distintos sistemas. El principal beneficio de este enfoque es optimizar la velocidad con la que los datos pueden leerse desde la memoria, que suele ser el principal cuello de botella al ejecutar aplicaciones y juegos modernos.

Debido a esta notable mejora en el rendimiento, gran parte de la industria del desarrollo de videojuegos ha ido adoptando este enfoque en los últimos años.

El SDK de Decentraland ejecuta escenas en JavaScript, y una desventaja de este lenguaje es que no ofrece control sobre la asignación de memoria. El motor que ejecuta Decentraland, sin embargo, usa C#, que sí se beneficia mucho de seguir principios orientados a datos. El SDK y el motor se están enviando mensajes constantemente entre sí. Para que esta comunicación sea lo más eficiente posible, tiene sentido mantener las estructuras de datos en ambos lados lo más similares posible, para evitar tener que reorganizar estos datos constantemente.

## Cómo se ve

La Programación Orientada a Datos es diferente de la Programación Orientada a Objetos, un enfoque con el que muchos desarrolladores están familiarizados actualmente. En la Programación Orientada a Objetos, el código se estructura siguiendo abstracciones que intentan replicar construcciones del mundo real: objetos. Cada uno de estos objetos puede contener tanto datos como funcionalidad. Las aplicaciones que usan este enfoque suelen ser fáciles de planificar conceptualmente, pero también más ineficientes de ejecutar.

En la Programación Orientada a Datos, los datos no se estructuran alrededor de objetos, sino para optimizar su facilidad de acceso. Las construcciones del mundo real que representan los datos no intervienen en los diferentes flujos que mutan esos datos.

El modelo de Entity Component System (ECS) sobre el que se construye el SDK de Decentraland es muy compatible con el enfoque de Programación Orientada a Datos. Cada componente forma parte de una colección estructurada de datos. Los componentes pertenecen a una Entity por referencia, pero los datos no se estructuran alrededor de la Entity, sino como una colección de componentes similares. Por ejemplo, todos `Transform` components en una escena son iguales. Uno de estos Transform podría pertenecer al modelo de tu edificio principal, otro al cristal que es hijo de una mesa. Luego, los sistemas en la escena procesan la lista de `Transform` componentes uno por uno, sin hacer distinciones. Todos los `Transform` componentes tienen los mismos campos y pasan por las mismas comprobaciones.

![](/docs/_remote/29703f638d6018eac3f30fbf.webp)

Imagina una escena que tiene una docena de puertas que pueden estar abiertas o cerradas. Puedes representar el estado de todas estas puertas con un simple componente "isOpen" que contiene un valor booleano. Si "isOpen" es true, la puerta debe estar abierta; si es false, la puerta debe estar cerrada. Si un jugador hace clic en una puerta, esta debería cambiar de estado, y los demás jugadores también deberían verlo. Mientras tu escena procesa un cambio en el estado de una puerta y lo sincroniza con otros jugadores, realmente no le importa qué representa "isOpen". Todo el conjunto de componentes no es más que una colección de booleanos que deben sincronizarse con otros jugadores. Luego, un sistema separado en tu escena puede encargarse de hacer coincidir regularmente el estado de cada "isOpen" con la rotación correspondiente de su puerta.

La Programación Orientada a Datos no es necesariamente más difícil, pero es un enfoque diferente que hay que aprender y adoptar. Los desarrolladores que no están acostumbrados a este enfoque podrían necesitar algo de tiempo para familiarizarse con él; te animamos a explorar y jugar con ejemplos para hacerte una mejor idea.

## Por qué funciona

Para entender por qué la programación orientada a datos marca una diferencia tan grande, necesitamos echar un vistazo al hardware.

Cuando el procesador necesita obtener datos de la memoria, recupera un bloque completo de datos en la caché, incluidos datos que casualmente se escribieron en la memoria junto al valor que queríamos. Cuantos más datos pueda usar tu código que ya estén en caché, más rápido se ejecutará tu código.

Imagina que la memoria de la máquina es un almacén literal, donde los datos se guardan en muchas cajas apiladas. Cada vez que necesitamos cierta información, tenemos que llamar a una carretilla elevadora para que vaya a buscar la caja donde se encuentran esos datos y la traiga al mostrador para inspeccionarla. Ese mostrador solo puede contener unas pocas cajas a la vez, así que no puedes tener mucho dato cerca.

La carretilla elevadora tarda mucho en ir hasta el fondo del almacén y traernos una caja. Si los datos que quieres de esas cajas están dispersos, eso significará pedirle a la carretilla que haga muchos viajes. La mayor parte del tiempo, estarás cruzado de brazos junto al mostrador, esperando a que llegue la siguiente caja que pediste.

Puedes evitar gran parte de ese tiempo desperdiciado si apilas esas cajas de forma inteligente y organizas los datos para que las cosas que probablemente necesites al mismo tiempo queden, en su mayoría, agrupadas. Si agrupas los datos con inteligencia, a menudo descubrirás que lo siguiente que necesitas ya está en una de las cajas del mostrador. Podrás ir directo a ello sin molestar al operador de la carretilla elevadora.

Por ejemplo, si tienes una escena que necesita obtener el estado de una puerta para comprobar si está abierta o cerrada, el hardware no solo está recuperando el valor del booleano específico que describe el estado de esa puerta, sino que también está recuperando muchísimos otros datos que pueden o no ser relevantes.

Supón que tu escena tiene un sistema que necesita actualizar el estado abierto/cerrado de una docena de puertas, una vez por frame. Si tu código está organizado siguiendo un enfoque de Programación Orientada a Objetos, no hay forma de saber cómo podrían agruparse los distintos fragmentos de información relevantes. Quizá una "caja" de nuestro almacén contiene el estado "isOpen" de la puerta A y también la textura de la puerta A y el audio que reproduce al abrirse. Puede que tengas que hacer un viaje para obtener una nueva "caja" de datos por cada una de las doce puertas. Y, por supuesto, todo este proceso tiene que volver a ocurrir en cada frame. Así que eso son 360 (30 x 12) viajes metafóricos al fondo del almacén por segundo, incluso cuando ninguna de las puertas ha cambiado de estado.

Si tu código sigue, por otro lado, un enfoque Orientado a Datos, es muy probable que esos 12 booleanos estén en la misma caja. Esto se debe a que todos esos booleanos forman parte de un único array que se almacenó en memoria de una sola vez. Nunca organizas explícitamente cómo encajan estos datos en la memoria, así que, en el peor de los casos, el array podría acabar dividido entre dos cajas. Pero incluso en ese peor escenario, 2 viajes es mucho mejor que 12.

Comprobar el estado de cada puerta en cada frame suena a mucho trabajo, pero en realidad es súper rápido si todos los datos ya están en la caché de memoria. Si tus datos están bien organizados, tu escena puede ejecutar procesos como estos sobre muchas entidades y aun así seguir funcionando muy rápido.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
