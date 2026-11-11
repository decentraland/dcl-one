> Cómo usar la DAO de Decentraland

# Guía del usuario de la DAO

## Índice

* [Iniciar sesión](#logging-in)
* [Poder de voto](#voting-power)
* [Condiciones de aprobación/rechazo](#approvereject-conditions)
* [Navegar por las propuestas](#browsing-all-proposals)
* [Ver una propuesta](#viewing-a-proposal)
* [Añadir propuestas a tu lista de seguimiento](#adding-proposals-to-your-watch-list)
* [Votar](#voting)
* [Participar en el Forum](#participating-in-the-forum-discussions)
* [Crear una propuesta](#creating-a-proposal)
* [Categorías de propuestas](#proposal-categories)
* [Eliminar una propuesta](#deleting-a-proposal)

## Iniciar sesión

Para empezar con la DAO, visita [governance.decentraland.org](https://governance.decentraland.org). Verás una pantalla de bienvenida y un tutorial de inicio rápido.

Después de leer el tutorial de inicio rápido, haz clic en **Iniciar sesión**. Debes conectar una Wallet para usar la DAO. Actualmente, puedes usar cualquiera de **Metamask** o **Fortmatic**. Asegúrate de que la Wallet que usas contenga los tokens relevantes para participar en la DAO (MANA, NAMES o LAND)

Tras conectar tu Wallet, serás llevado a la página principal de la DAO, que muestra una lista de todas las propuestas actualmente activas.

## Poder de voto

El poder de voto se calcula a partir del saldo total de MANA, NAMES y LAND asociados a la Wallet conectada con la DAO. **La DAO tiene en cuenta tanto tus saldos wrapped como unwrapped, así que no necesitas hacer unwrap de ningún MANA para alcanzar tu poder de voto completo.** Sin embargo, puedes hacer unwrap de cualquier MANA previamente wrapped en cualquier momento desde la pestaña "Poder de voto". Aún así, sigue habiendo una comisión de gas asociada al unwrap de MANA, ya que es una transacción en la mainnet de Eth.

Tu voto en la DAO de Decentraland se pondera según el saldo de MANA, NAMES y LAND asociado a la cuenta con la que inicias sesión.

Para ver tu poder de voto actual, ve a la **Poder de voto** pestaña.

#### ¿Cómo se calcula el poder de voto?

El poder de voto se representa como **"VP"**. MANA, NAME, LAND y Legacy Wearables contribuyen a tu poder de voto total de la siguiente manera:

* 1 MANA aporta 1 VP
* 1 NAME aporta 100 VP
* 1 parcela de LAND aporta 2000 VP
* Las siguientes colecciones de Legacy Wearables aportan 1 VP por cada **uncommon**, 5 VP por cada **rare**, 10 VP por cada **epic**, 100 VP por cada **legendary**, y 1000 VP por cada **mythic**:
  * [Community Contest Collection](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
  * [DCL Public Explorer Launch Collection](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
  * [Exclusive Masks Collection](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
  * [Halloween 2019 Collection](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
  * [My Crypto Heroes Collection](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
  * [Xmass 2019 Collection](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* Cada Estate vale 2000 multiplicado por el número de parcelas individuales de LAND en ese Estate. Por ejemplo, un Estate con 2 parcelas aportará 4000 VP a tu poder de voto total.

#### ¿Qué sucede si tu poder de voto cambia antes de que se cierre una propuesta?

Si tu saldo de MANA o LAND cambia, afectará a tu poder de voto, pero solo en las propuestas que se creen después de que cambie tu saldo.

En el momento en que se crea una nueva propuesta, la DAO mira los saldos de MANA y LAND de todos los votantes para calcular su poder de voto. En otras palabras, cuando votas en una propuesta, el poder de voto de tu voto será igual al saldo que tenías en el instante en que la propuesta se creó originalmente. Puedes votar con ese VP y luego cambiar tu saldo de MANA/LAND sin afectar al peso de tu voto.

#### ¿Por qué la DAO de Decentraland usa votos ponderados?

Si la DAO de Decentraland diera a cada dirección de Ethereum un voto, y cada voto tuviera el mismo peso, los usuarios podrían crear tantas direcciones separadas como quisieran para obtener más poder de voto.

Determinar el poder de voto teniendo en cuenta los saldos de MANA, NAME y LAND es actualmente la forma más segura de limitar la cantidad de influencia que puede tener cada votante. Además, cuanto más MANA, NAME o LAND poseas en Decentraland, mayor se considera tu participación personal, y por tanto tu voto gana más influencia dentro de la DAO.

## Condiciones de aprobación/rechazo

Dependiendo del tipo de propuesta, se necesita una cierta cantidad de VP participante para considerar válida la votación. Verás esto referenciado en la dApp de Governance como el **Umbral de aceptación** para la propuesta. Una vez que se haya alcanzado el umbral de aceptación, una propuesta se aprueba si el poder de voto total a favor de la propuesta es mayor que el poder de voto total en contra de la propuesta. Este es el modelo típico de mayoría 50/50 que esperamos ver en la mayoría de las votaciones democráticas.

El único tipo de propuesta que no sigue esta regla son las **Encuestas previas a la propuesta**. Dado que estas propuestas se consideran un mecanismo no vinculante para recopilar comentarios de la comunidad sobre una idea, no tienen opciones Sí/No predefinidas para votar. Los usuarios podrían añadir más de dos opciones, convirtiéndolas en una encuesta de opción múltiple. Es importante mencionar que, si las encuestas previas a la propuesta alcanzan el Umbral de aceptación definido de 500k VP, podrían ser promovidas a una **propuesta de borrador** y terminar siendo una **propuesta de gobernanza**.

## Navegar por todas las propuestas

Para ver todas las propuestas en la DAO, visita [governance.decentraland.org](https://governance.decentraland.org). La página de inicio de la DAO muestra, de forma predeterminada, todas las propuestas añadidas recientemente ordenadas de la más reciente a la más antigua.

Puedes filtrar las propuestas por **Resultados**:

* **Activas** – propuestas en las que se está votando actualmente
* **Aprobadas** – propuestas que han sido aprobadas por la comunidad
* **Rechazadas** – propuestas que ya han sido votadas y que fueron rechazadas por la comunidad o no alcanzaron el umbral de aceptación
* **Ejecutadas** – propuestas que han sido ejecutadas on-chain por el Comité de la DAO
* **Finalizadas** – propuestas que están cerradas, pero no tienen resultados de sí/no, como las encuestas de opción múltiple

También puedes filtrar las propuestas por **Categoría** usando la columna de categoría en el lado izquierdo de la UI:

* **Todas las propuestas** – muestra todas las propuestas sin importar la categoría o el resultado
* **Nodo de Catalyst** – muestra solo propuestas para añadir nuevos nodos de Catalyst
* **Punto de interés** – muestra solo propuestas para añadir nuevos POIs
* **Prohibición de nombres** – muestra solo propuestas para prohibir un nombre
* **Solicitud de subvención** – muestra solo solicitudes de subvención
* **Encuesta previa a la propuesta** – muestra solo encuestas no vinculantes
* **propuesta de borrador** – muestra solo propuestas de borrador
* **propuesta de gobernanza** – muestra solo propuestas finales vinculantes de Governance

Para ver solo las propuestas que han sido aprobadas, haz clic en la **Ejecutadas** pestaña. Las propuestas ejecutadas en esta pestaña se ordenan de la más reciente a la más antigua de forma predeterminada.

## Ver una propuesta

Para leer una propuesta, solo tienes que hacer clic en el título de la propuesta para ver la página de detalles.

Cada página de detalles de propuesta incluye toda la información descriptiva proporcionada por la persona que envió la propuesta.

También encontrarás enlaces al hilo de discusión de la propuesta en el Forum, botones para añadir una propuesta a tu lista de seguimiento, los resultados de votación actuales, botones para votar, el umbral de aceptación, tu poder de voto actual y quién votó en la propuesta.

Las páginas de detalles de las propuestas también muestran el nombre de avatar único asociado a la dirección de Ethereum que abrió la propuesta, si existe, y las fechas de inicio y fin de la propuesta.

Por último, verás un enlace a la entrada de la propuesta en Snapshot – la plataforma de votación usada por la DAO de Decentraland.

## Añadir propuestas a tu lista de seguimiento

Para añadir una propuesta a tu lista de seguimiento, ve a la página de detalles de la propuesta y haz clic en **Añadir a mi lista de seguimiento**.

Para eliminar una propuesta de tu lista de seguimiento, haz clic en **Eliminar de mi lista de seguimiento** desde la página de detalles de la propuesta. También puedes hacer clic en el icono de bandera roja de cualquier propuesta que esté actualmente en tu lista de seguimiento para eliminarla.

## Participar en las discusiones del Forum

Como plataforma de gobernanza, la DAO de Decentraland es más efectiva cuando se combina con discusiones frecuentes dentro de la comunidad. Cada vez que se abre una nueva propuesta en **governance.decentraland.org**, se abre automáticamente un tema asociado en [**forum.decentraland.org**](https://forum.decentraland.org).

Antes de emitir tu voto, tómate el tiempo de ver y participar en estas discusiones del foro. Haz clic en el botón **Discutir en el foro** desde la página de detalles de cualquier propuesta de la DAO que quieras discutir. También puedes explorar los temas abiertos yendo a [forum.decentraland.org](https://forum.decentraland.org), dirigiéndote a la **Gobernanza** categoría, y explorando los temas abiertos.

¡No necesitas poseer tokens para participar en estas discusiones! Todo el mundo es bienvenido a contribuir a la conversación.

## Votar

Para votar en una propuesta, inicia sesión en la DAO en [governance.decentraland.org](https://governance.decentraland.org) con una Wallet que contenga MANA, NAME o LAND.


> [!NOTE]
> **Saldo mínimo necesario para votar:** Solo los titulares de MANA, NAME o LAND pueden votar en las propuestas de la DAO de Decentraland. **El saldo mínimo actual necesario para tener un voto ponderado en las propuestas de la DAO es 1 VP** Votar con un saldo de cero resultará en un voto con un peso de 0, lo que no afecta los resultados finales.


Para votar en una propuesta, una vez que hayas conectado tu Wallet, solo tienes que ver la página de detalles de la propuesta y hacer clic en el **VOTAR SÍ** o **VOTAR NO** botón, o seleccionar una de las opciones de respuesta múltiple si se trata de una encuesta, según cómo quieras votar.

Asegúrate de leer la propuesta completa para entender el asunto que se está discutiendo y qué sucederá si la propuesta se aprueba o se rechaza.

Después de hacer clic en el botón de votar, tu Wallet de Ethereum conectada te pedirá firmar la transacción. Recuerda que esto es solo una transacción firmada y no requiere comisión de gas.

Se te dará la opción de añadir la propuesta a tu lista de seguimiento; esta es una buena manera de hacer seguimiento de las propuestas que te interesan. Si no quieres añadir la propuesta a tu Watchlist, solo haz clic en **No, gracias**.

Después de enviar tu voto, puedes cambiarlo en cualquier momento hasta el final del periodo de votación, como se muestra en la página de detalles de la propuesta.

#### ¿Qué sucede si tu VP cambia antes de que se complete una propuesta?

La DAO calcula tu poder de voto para cada propuesta individual en el momento en que se crea cada propuesta. Si tu VP cambia después de ese momento, no afectará a tu voto en esa propuesta.

Para una explicación completa del poder de voto, y de cómo y cuándo se calcula, consulta la [Poder de voto](#voting-power) sección de la Guía de usuario.

## Crear una propuesta

Para crear una nueva propuesta en la DAO de Decentraland, empieza iniciando sesión en [governance.decentraland.org](https://governance.decentraland.org) y conectando una Wallet que contenga MANA, NAME o LAND.

Después de iniciar sesión y conectar tu Wallet, haz clic en **Enviar una propuesta** y selecciona la categoría de propuesta que quieras usar. Cada categoría proporcionará un formulario que te permitirá aportar la información relevante para tu propuesta.

El formulario de cada categoría es diferente, así que asegúrate de seleccionar la categoría correcta para tu propuesta. Los formularios de propuestas en la DAO admiten Markdown, así que puedes dar formato al contenido de tu propuesta para que sea más legible. Esto es especialmente útil para propuestas más largas.

Para previsualizar tu texto Markdown renderizado, activa el **Vista previa** interruptor. Si no estás familiarizado con Markdown, puedes usar texto simple sin formato.

Algunas categorías de propuestas tienen umbrales mínimos de VP para su presentación, lo que significa que tienes que tener al menos cierta cantidad de VP para enviar las propuestas. Se te informará de ello en la dApp de Governance.

Después de completar el formulario de propuesta de la categoría que hayas seleccionado, haz clic en **Enviar propuesta**. Después de enviar correctamente tu propuesta, serás llevado a la página de detalles de tu nueva propuesta, donde podrás añadirla a tu lista de seguimiento y emitir tu voto.

## Categorías de propuestas

### Propuestas vinculantes

#### 📍 Punto de interés

Los puntos de interés son ubicaciones destacadas en Decentraland. Estos "POIs" se promocionan en varias áreas de la UI del mundo virtual y se presentan como buenos lugares para que los usuarios exploren, especialmente quienes son nuevos en Decentraland.

Si has creado una ubicación, o has encontrado una ubicación que crees que debería estar en esta lista, o si crees que un POI actual debería eliminarse porque ya no es interesante, puedes usar esta categoría de propuesta para presentar tu sugerencia a la DAO.

#### 🚫 Prohibición de nombres

La lista de "nombres prohibidos" incluye nombres ofensivos o dañinos de avatares y ubicaciones que no están permitidos en Decentraland. Cualquier nombre de esta lista no puede reclamarse, usarse ni transferirse entre usuarios. Para sugerir la prohibición de un nombre, puedes usar la categoría de propuesta Name ban en la DAO.

#### 🌐 Nodos de Catalyst

Los nodos de Catalyst son los servidores gestionados por la comunidad que proporcionan el contenido y establecen las conexiones peer-to-peer necesarias para mantener en funcionamiento el mundo virtual de Decentraland. Cada vez que un usuario abre Decentraland, se conecta a uno de estos nodos. Sin embargo, solo los nodos aprobados por la DAO se usan en la red de Decentraland.

Para sugerir la adición de un nuevo nodo a la red, puedes usar la categoría Catalyst Node.

### Propuestas del proceso de gobernanza

Algunas propuestas no son tan simples como añadir o eliminar un elemento de una lista; requieren señalización comunitaria, discusiones y vías de implementación. Estas propuestas deben presentarse a través de un proceso de gobernanza de tres etapas que comienza con una encuesta y termina con una propuesta vinculante.

El proceso de votación incluye tres pasos: una encuesta previa a la propuesta, una propuesta de borrador y una propuesta de gobernanza. Cada nivel tendrá umbrales de presentación y aprobación progresivamente más altos para garantizar que las decisiones importantes de gobernanza sean tomadas por una mayoría representativa (basada en el poder de voto). Cada paso debe alcanzar el umbral de VP definido para ser promovido al siguiente.

#### 📊 Encuesta previa a la propuesta

Este es el primer paso para llegar a la propuesta final vinculante de Governance. Las encuestas en la DAO de Decentraland no son vinculantes; son cuestionarios de opción múltiple que pueden usarse para medir la opinión general o el sentimiento de la comunidad respecto a diferentes temas. No son vinculantes porque la DAO no actúa automáticamente en función de los resultados de ninguna de estas encuestas. Si una encuesta previa a la propuesta reúne suficiente VP participante (500k VP es el umbral de aceptación), podría ser promovida a propuesta de borrador.

#### 📊 Propuesta de borrador

Una Propuesta de borrador presenta una posible política a la comunidad en un formato estructurado y formaliza la discusión sobre los posibles impactos de la propuesta y sus vías de implementación. Una Propuesta de borrador que no tenga éxito o no alcance este umbral puede modificarse y volver a presentarse una vez. Si una propuesta de borrador reúne suficiente VP participante (1M VP es el umbral de aceptación), podría ser promovida a una propuesta vinculante de Governance.

#### 📊 Propuesta de gobernanza

Este es el último paso del proceso de Governance y es el único vinculante. Esta propuesta debe desarrollar todos los detalles, datos, métodos, evaluaciones o cualquier otra información relevante para implementar esta propuesta. Una propuesta de gobernanza se acepta si alcanza el umbral de aceptación de 6M VP y la opción Sí obtiene mayoría simple.

## Eliminar una propuesta

Las propuestas solo pueden ser eliminadas por la persona que las creó.

Para eliminar una de tus propuestas, ve a la página de detalles de la propuesta que quieres eliminar. Haz clic en **Eliminar propuesta** en la parte inferior de la columna derecha. Tendrás que confirmar la eliminación.

¡Ten cuidado! ¡Las propuestas eliminadas no se pueden restaurar!

Si eliminas una propuesta después de que la gente haya votado en ella, no pasará nada. Las propuestas eliminadas con acciones vinculantes no serán ejecutadas por el Comité de la DAO.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/dao/dao-userguide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
