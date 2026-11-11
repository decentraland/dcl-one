> Qué debes tener en cuenta al diseñar un juego para Decentraland.

# Diseñar juegos

Este documento cubre algunos puntos clave a tener en cuenta al diseñar un juego para Decentraland. Consideraciones como la adyacencia de otras escenas y la propiedad distribuida de los LAND hacen de Decentraland un lugar único que requiere que reevalúes supuestos que quizá tengas de juegos anteriores.

Por ejemplo, debes entender que, a diferencia de otras plataformas de juegos, los juegos de Decentraland no existen en el vacío. No tienes control sobre lo que hay en las escenas adyacentes, y no tienes control sobre ciertos detalles como los avatares del jugador o los objetos que podrían traer de otros juegos. Esto abre la puerta a posibilidades emocionantes y requiere que pienses en las mecánicas de juego de manera diferente.

La cosa más parecida en los juegos convencionales en este momento es Roblox, donde el contenido generado por los usuarios de la comunidad puede luego convertirse en un lugar de encuentro para que otros exploren, jueguen e interactúen. A diferencia de Roblox, navegas por escenas no explorando un menú de experiencias no relacionadas, sino explorando físicamente un terreno donde todas las escenas están adyacentes entre sí. Decentraland también utiliza la blockchain como una forma de gestionar la propiedad de terrenos, avatares, activos, etc.

Estamos mejorando continuamente el SDK, así que algunas de las siguientes limitaciones podrían eliminarse con futuras actualizaciones.

## Límites de la escena

**Tu juego debe encajar por completo en el \_LAND**\_\*\* sobre el que se construye tu escena.\*\* Para escenas pequeñas, piensa en juegos como el fútbol, donde las reglas del juego mantienen la interacción relevante dentro de un espacio confinado, aunque los jugadores puedan salir del campo de juego. Los jugadores pueden caminar fuera de los límites de una escena, pero cualquier asset o entidad que pertenezca a la escena debe permanecer dentro de la escena.

Los jugadores que salen de tu escena siguen renderizándola mientras esté dentro del rango visible. Si se alejan demasiado, dejarán de renderizarla por completo.

También podrías construir un juego que se extienda por varios terrenos desconectados que sean desconocidos para los jugadores, y donde la exploración del resto del mundo se convierta en parte de la jugabilidad. Un juego así estaría formado por múltiples *escenas*, que podrían compartir datos entre sí a través de un servidor.

Si tu juego se publica en un [Decentraland World](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), tienes más control sobre lo que los jugadores ven más allá de los límites de tu escena. Los Worlds suelen estar rodeados por un paisaje autogenerado de pradera, árboles y mar, pero puedes [desactivar este paisaje](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#landscape-terrain) si choca con la ambientación de tu juego, por ejemplo, un juego que transcurre en mar abierto o en el espacio.

### Inventario del usuario

**Actualmente no existe un inventario donde los jugadores puedan almacenar objetos del juego mientras caminan entre escenas.** Hoy están disponibles las siguientes alternativas:

* Puedes almacenar la información del inventario en la propia escena y vincularla a la dirección de Ethereum de cada jugador (esto puede usarse como un ID persistente). Esta información solo podría leerse desde tu escena.
* Puedes usar un almacenamiento externo personalizado y sincronizar todas tus escenas con él. Esta es una solución más robusta que puede manejar volúmenes mayores de jugadores. También puede extender el acceso a este inventario a múltiples escenas separadas que sean tuyas o de otros.
* Usa tokens en la blockchain para gestionar la propiedad de los objetos.

Cuando se gana un objeto del juego, podría almacenarse como un token especial en la Wallet de Ethereum de un jugador. Cuando un jugador que posee el token entra en tu escena, tu escena podría otorgarle al jugador ciertas características dentro del juego.

Otras escenas también podrían responder potencialmente al mismo token de diferentes maneras, lo que puede dar lugar a interacciones interesantes entre juegos.

La desventaja de usar la blockchain para almacenar objetos del inventario es que todas las transacciones tienen un coste para el jugador y no son inmediatas. Lee más sobre la blockchain en una sección especializada más abajo.

En futuras versiones, los jugadores tendrán un inventario que llevarán a todas partes, que incluirá tanto activos on-chain como off-chain.

### Experiencias portables


> [!NOTE]
> **💡 Consejo**: Consulta [Experiencias portables](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/portable-experiences/) y [Smart wearables](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/) para saber cómo crearlos.


Las experiencias portables son partes de la jugabilidad que los jugadores se llevan consigo a medida que se mueven por el metaverso. No están ligadas a parcelas de terreno; a veces están ligadas a tokens, o a veces las lanza el Explorer. Por ejemplo, un jugador podría llevar una bola de nieve desde tu escena, alejarse a otra escena y lanzar la bola de nieve a otro jugador que también esté jugando el mismo juego.

Los Smart wearables son un tipo de experiencia portable que está vinculada a un token wearable y se activa cuando el jugador se pone la prenda. Los Smart wearables pueden otorgar a los jugadores nuevas habilidades, como un jetpack que les permita volar, o añadir una nueva capa de contenido sobre el resto del mundo, como colocar aleatoriamente monedas para recoger por todo Genesis City.

Ten en cuenta que los jugadores podrían estar usando la experiencia portable de otra persona mientras están en tu escena. Consulta [Datos del usuario](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-portable-experiences) para aprender a comprobar qué experiencias portables tiene activadas actualmente un jugador.

## Persistencia del juego

**Decentraland es un mundo persistente; tu escena puede ser visitada por jugadores en cualquier momento.** Tu escena no tiene fase de inicio ni final, así que deberías diseñar las mecánicas del juego de manera que permita que los jugadores que entren o salgan en cualquier momento también puedan participar.

Tu escena podría tener un mecanismo de reinicio que la lleve a un estado inicial, pero debes tener cuidado de no interrumpir el juego de los jugadores que ya están jugando.

### Sincronizar el estado de la escena

**Por defecto, los estados de la escena no se comparten entre jugadores.** Cada jugador ejecuta su propia copia local de la escena. Esta es la forma más sencilla de construir una escena, pero no es ideal para experiencias sociales.

La forma más fácil de compartir el estado entre jugadores es marcar las entidades como sincronizadas, usando la `syncEntity` función. Consulta [Multijugador sin servidor](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/). Con este enfoque, los cambios de estado no se almacenan en ningún sitio: si no hay jugadores cerca de la escena cargándola en ese momento, la escena se restablecerá a un estado predeterminado la próxima vez que se cargue.

**También puedes usar un servidor para almacenar información sobre tu escena y mantener a todos los jugadores sincronizados con ella.** Esto mantiene el estado de la escena coherente y persistente incluso cuando no hay jugadores cerca, y te permite validar los cambios para que los jugadores no puedan hacer trampas. La opción recomendada es usar un [Servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/), que Decentraland aloja e implementa por ti como parte de la publicación de tu escena. También puedes alojar [tu propio servidor](/docs/creator/content-creator-es/scenes-sdk7/networking/third-party-servers/), lo que te permite mantener cierta información, como tokens de seguridad, solo en el servidor, sin exponer nunca esa información al exterior.

### Temporización del juego

**Los juegos que usan la arquitectura de comunicaciones predeterminada deben tener en cuenta que podría haber lag entre los jugadores** y no deberían depender de reacciones rápidas entre las acciones de distintos jugadores. Recomendamos juegos por turnos o que se basen principalmente en interacciones jugador contra entorno.

Para los juegos en los que la sincronización de las acciones entre jugadores es crítica, como un shooter en primera persona, deberías implementar tu propio servidor como una fuente de verdad autoritativa en tiempo real entre todos los jugadores de tu escena.

## Jugadores en la escena

**Los jugadores se identifican en Decentraland usando la dirección de su Wallet de Ethereum.** Esta Wallet se usa como un ID persistente que ya está asociado con todos los tokens que posee el jugador.

**Actualmente no hay forma de limitar cuántos jugadores pueden estar presentes en Decentraland al mismo tiempo.** A diferencia de muchos otros juegos en los que puede haber distintas sesiones de juego alojadas en servidores separados, solo hay una instancia de Decentraland compartida entre todos los jugadores, al menos por ahora.

Debes tener en cuenta que puede haber varios jugadores moviéndose por tu escena en cualquier momento. Algunos podrían estar de paso y no participar en el juego. Asegúrate de que las mecánicas del juego no puedan verse fácilmente alteradas por esto.

**El bucle del juego de tu escena no puede afectar directamente a los jugadores**, la escena tiene un enfoque reactivo ante las acciones del jugador. Si un jugador está de pie sobre una entidad y la entidad se mueve o gira, el jugador se moverá con esta entidad. Esto es especialmente útil para ascensores, plataformas flotantes y similares.

Como propietario de una escena, no puedes empujar a la fuerza ni teletransportar fuera de tu escena a un jugador infractor. Sin embargo, podrás añadir jugadores a una lista negra en el servidor de señalización. También puedes implementar una lista negra en el código de tu escena y denegar ciertos servicios a los jugadores en lista negra.

## Construye interiores más grandes que en la vida real

La mayoría de los jugadores exploran Decentraland con la cámara predeterminada en tercera persona, que se sitúa detrás y por encima del avatar. Esa cámara necesita espacio. Un interior construido con dimensiones reales se percibe en pantalla como un pasillo estrecho: la cámara se pega al techo, el avatar llena el encuadre y el jugador no puede ver hacia dónde va.

Construye la estructura de una habitación aproximadamente de **1,5 a 2 veces** la altura libre real:

| Elemento                                             | Mundo real                                      | Constrúyelo a                                             |
| ---------------------------------------------------- | ----------------------------------------------- | --------------------------------------------------------- |
| Altura del techo                                     | 2,5 a 3 m                                       | **5 a 6 m**                                               |
| Puerta o apertura en una ruta principal              | aprox. 2,1 m x 0,9 m                            | **aprox. 4 m de alto, 3 m o más de ancho**                |
| Ancho del corredor                                   | 1,2 a 1,8 m                                     | **4 m o más de ancho**                                    |
| Cualquier hueco por el que tenga que pasar el avatar |                                                 | **1,2 m de espacio libre o más**                          |
| Escaleras                                            | aprox. 0,18 m de contrahuella, 0,28 m de huella | **0,25 m de contrahuella o menos, 0,3 m de huella o más** |
| Rampas                                               |                                                 | **30 grados o menos**                                     |

La cámara en tercera persona orbita a unos 4,5 m del avatar, con un campo de visión de 60 grados. Cuando una pared o un techo se interpone entre la cámara y el avatar, la cámara avanza para mantener al avatar a la vista, y ese salto repentino es lo que hace que una habitación se sienta estrecha. Por eso, las habitaciones donde los jugadores dan media vuelta se perciben mejor con una anchura de aproximadamente **9 m de ancho o más**, el doble de la distancia de la cámara. Para conocer los límites del motor sobre escalones, pendientes y altura libre, consulta [Restricciones espaciales del jugador](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/#player-spatial-constraints).

Algunas cosas a tener en cuenta:

* **Haz más grande la habitación, no su contenido.** Las sillas, escritorios, mostradores, barandillas y señalización se mantienen cerca del tamaño real. Si escalas los muebles junto con las paredes, recreas la misma sensación de agobio en un tamaño mayor, y el avatar deja de percibirse como una persona en la habitación.
* **Aplica esto cuando planifiques el área del suelo**, no después de haber colocado todo. Las habitaciones más grandes necesitan más parcelas, así que estos números cambian tu [presupuesto de parcelas](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/).
* **Juzga el resultado desde la cámara normal en tercera persona.** Una cámara libre o el viewport de Creator Hub favorecen a los interiores, porque pueden situarse fuera de la geometría y mirar hacia dentro. Recorre la ruta como lo haría un jugador. Si la cámara atraviesa el techo, o la cabeza del avatar ocupa demasiado la parte superior del encuadre, la habitación es demasiado baja.


> [!NOTE]
> **💡 Consejo**: Esto es una guía de diseño basada en la práctica, no un límite del motor. Toma los números como punto de partida y ajústalos a ojo.


## Limitaciones del contenido de la escena

**Por favor, construye tu escena prestando especial atención a la eficiencia de tu código.** Decentraland necesita ejecutarse en navegadores web y dispositivos móviles, y los jugadores estarán renderizando varias escenas al mismo tiempo mientras caminan por el metaverso.

**También deberías intentar que la escena sea ligera.** A diferencia de otros juegos en línea donde las mismas texturas y activos se repiten cómodamente por todo un gran mundo abierto, en Decentraland cada escena podría tener su propio conjunto de activos completamente diferente. A medida que los jugadores atraviesan varias escenas, deberían poder descargar la totalidad del contenido de la escena, incluidas texturas, archivos de sonido, etc., a una velocidad razonable.

Por ello, hemos impuesto algunos límites para evitar un uso excesivo de recursos computacionales. Consulta [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para conocer en detalle cuáles son esos límites.

## Acceso a las escenas

**El mapa de Decentraland está diseñado de modo que haya carreteras y plazas públicas,** estas garantizan un acceso fácil a varias partes del mapa, independientemente de lo que construyan otras personas. Las parcelas de terreno que no estén adyacentes a ninguna carretera o plaza corren el riesgo de quedar bloqueadas por escenas vecinas, aunque esperamos que la mayoría de las escenas sean transitables y no bloqueen a otras.

Los nuevos jugadores comenzarán su experiencia en Genesis Plaza, en el centro del mapa, donde se les animará a seguir algunas actividades del tutorial y luego a explorar el mundo.

Los jugadores también pueden escribir manualmente una URL para una coordenada específica en el mapa de Decentraland para aparecer en esa ubicación. También puedes compartir enlaces a URLs que tengan coordenadas iniciales codificadas.

Ten en cuenta que si un jugador comienza en una ubicación que está bloqueada o por debajo del nivel del terreno, no será una experiencia agradable. Para evitar esto, hay una forma de definir un conjunto de ubicaciones específicas en tu escena en las que sea seguro aparecer. Consulta [metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) para más detalles.

Los jugadores también pueden navegar por el mundo rápidamente usando el mapa y las listas de ubicaciones y eventos populares. Tu escena puede incluir teleports que transporten a los jugadores a otras partes del mundo; consulta [Teletransportes](/docs/creator/content-creator-es/scenes-sdk7/interactividad/external-links/#teleports).

## Interfaz de usuario

**La UI superpuesta predeterminada que los jugadores ven al entrar en Decentraland solo incluye lo esencial.** Puedes añadir elementos adicionales a esa UI mientras un jugador está en tu escena. Ten en cuenta que la UI predeterminada de Decentraland se muestra por encima de cualquier cosa de tu escena, así que diseña tu UI para que no se superponga con ella.

Cuando un jugador sale de la escena, todos los elementos de la UI se eliminan para no interferir con otras escenas. Los jugadores también tienen disponible en su pantalla un botón para desactivar todos los elementos de UI de la escena; esto es principalmente útil para evitar comportamientos abusivos por parte de escenas que puedan querer cubrir todo el campo de visión del jugador.

## Física

Ten en cuenta que el SDK no proporciona un motor de física completo para las entidades. Puedes aplicar fuerzas e impulsos al avatar del jugador; consulta [Física del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/player-physics/). Para la física entre entidades, como colisiones o gravedad, puedes importar una librería o programar tú mismo el comportamiento.

## Entradas del controlador

**Los controles de tu juego deberían limitarse al conjunto de acciones de entrada admitidas por el SDK**: las teclas de movimiento, saltar, apuntar y hacer clic, los botones principal (E) y secundario (F), y los botones de acción numerados (1 a 4). Estas entradas están abstraídas de las teclas físicas, por lo que también pueden asignarse a los controles en pantalla de la app móvil o a otros dispositivos; no asumas que todo el mundo tiene un teclado. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para la lista completa, y [Entrada en dispositivos móviles](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/) para saber qué conviene priorizar al dirigirte a jugadores móviles.

Todas estas entradas admiten tanto eventos globales de *soltar botón* y *pulsar botón* como eventos, y eventos de impacto que te permiten identificar si una entidad estaba en la mira del jugador.

## Avatares

**Los jugadores pueden construir sus avatares a partir de un amplio catálogo de wearables**, incluidos wearables creados y vendidos por creadores de la comunidad en el Decentraland Marketplace. Tu escena puede leer lo que lleva puesto un jugador y reaccionar en consecuencia; consulta [Datos del usuario](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data).

## Comunicación entre jugadores

**Los jugadores pueden chatear entre sí, hablar por chat de voz y transmitir lenguaje corporal reproduciendo emotes** como bailar, aplaudir o saludar, incluidos emotes vendidos por creadores de la comunidad.

Tu escena puede detectar cuándo un jugador reproduce un emote y también puede hacer que el avatar de un jugador reproduzca una animación. Consulta [El jugador reproduce una animación](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-plays-animation) y [Animaciones de avatar](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/).

## Notificaciones del juego

**Actualmente no existe un sistema de notificaciones entre escenas.** Cualquier juego que requiera notificaciones mostradas fuera de la escena actual tendrá que implementarlas utilizando un servicio externo.

## Uso de la blockchain

**En Decentraland, la blockchain se usa para almacenar información sobre la propiedad.** Hoy en día esto se refiere principalmente a la propiedad de LAND, pero también puede usarse para la propiedad de objetos del juego, wearables, avatares especiales, emotes y tokens que pueden garantizar ciertos privilegios del juego o acceso a juegos.

La blockchain no se usa para almacenar el estado del juego, la posición del jugador ni nada que necesite cambiar en tiempo real.

### LAND y MANA

**Los jugadores no necesitan poseer ninguna parcela de terreno para participar en el metaverso.** De hecho, la gran mayoría de los jugadores no lo hará. Los avatares de los jugadores y los tokens LAND que poseen no están conectados de ninguna manera directa.

**Los jugadores no necesitan poseer previamente una Wallet de Ethereum ni tokens MANA para entrar en Decentraland.** Si tu jugabilidad depende en gran medida de poseer tokens, estarías excluyendo a la mayoría de los jugadores. Un modelo de juego freemium podría ser una forma ideal de adaptarse a ambas bases de usuarios.

### Otros NFTs

**Puedes usar tokens no fungibles especiales (NFTs) para representar objetos del juego, avatares personalizados o wearables.** Si un jugador posee uno de estos tokens, tu escena podría responder a él de diferentes maneras.

Lee sobre qué son los NFTs en [este artículo del blog](https://decentraland.org/blog/technology/what-are-nfts/).

### Transacciones dentro del juego

**Tu escena puede admitir transacciones en la blockchain para que los jugadores compren o ganen tokens.**

Las transacciones en la blockchain no son inmediatas, requieren tiempos de verificación y tienen un coste en Ether; tanto el tiempo como el coste varían según el uso actual de la red.

Decentraland utiliza la side-chain Polygon para gestionar transacciones de forma más rápida y barata que la red Ethereum. Esta side-chain es ideal para las transacciones dentro del juego, ya que los cambios pueden producirse más cerca del tiempo real y a un coste muy bajo. La cadena principal de Ethereum sigue recomendándose para transacciones que requieren mayor seguridad y que pueden permitirse ser más caras y tardar más. Consulta [Segunda capa](/docs/creator/content-creator-es/scenes-sdk7/blockchain/second-layer/).

El jugador siempre debe aprobar explícitamente estas transacciones en su cliente de Ethereum. Por ejemplo, al usar MetaMask, MetaMask pide al jugador que acepte cada transacción antes de procesarla.

Los jugadores también podrían firmar un contrato que apruebe automáticamente todas las transacciones solicitadas por una dirección específica o dentro de ciertas restricciones, para evitar interrupciones al aprobar transacciones.

También puedes usar smart contracts para condicionar las transacciones en función de condiciones personalizadas. Por ejemplo, los jugadores podrían apostar por el resultado de un juego, y los pagos correspondientes se realizarían automáticamente en cuanto se conozca el resultado.

Para implementar interacciones con la blockchain en el código de tu escena, debes usar librerías externas que se conecten con la red Ethereum. Las futuras versiones del SDK proporcionarán una API personalizada para exponer estas funcionalidades de una forma más sencilla.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
