> Usa la sidechain de Polygon en tu escena para habilitar transacciones blockchain mucho más rápidas y baratas.

# Segunda capa

## Acerca de las soluciones de segunda capa

Cualquier transacción que afecte a la blockchain lleva tiempo en completarse y cuesta gas. Ambas cosas son obstáculos para que los juegos en blockchain se vuelvan populares, porque los jugadores normalmente no tienen la paciencia de esperar tanto a que sus acciones surtan efecto, y no están dispuestos a gastar dinero en muchas transacciones mientras juegan.

Una solución habitual es mantener la mayor parte de la jugabilidad fuera de la blockchain, y realizar transacciones en blockchain solo para eventos clave, como ganar un objeto del juego o registrar una puntuación alta.

Sin embargo, otra forma de superar estas limitaciones, así como otros problemas de escalabilidad inherentes a las blockchains, es recurrir a una *segunda capa* blockchain, también llamada una *cadena lateral*.

Una segunda capa es otra blockchain que actúa como intermediaria entre una aplicación descentralizada y la cadena principal. Esta capa es más ligera y, por lo tanto, puede ofrecer respuestas más rápidas y con un coste de gas mucho menor.

En lugar de realizar transacciones directamente en la cadena principal, las transacciones se hacen en la cadena lateral, y luego es responsabilidad de la cadena lateral sincronizar eventualmente estos cambios con la cadena principal.

La cadena lateral puede ofrecer respuestas más rápidas porque es una red más pequeña con menos nodos. Las transacciones que se llevan a cabo en la segunda capa son inicialmente menos seguras, pero finalmente se registran en la cadena principal en lotes, y allí pueden aplicarse todas las comprobaciones de seguridad de la cadena principal.

La cadena lateral también puede reducir significativamente el coste de cada transacción individual porque agrupa muchas transacciones en una sola al sincronizarse con la cadena principal. Así, el gas que habría que pagar por una transacción en la cadena principal puede repartirse entre varios miles de transacciones.

Además, al sincronizarse con la cadena principal, se pueden evitar por completo varias transacciones redundantes, reduciendo aún más los costes de transacción. Por ejemplo, si Alice transfiere 1 ETH a Bob, y luego Bob transfiere 1 ETH a Carol, esas dos transacciones pueden simplificarse en una sola, registrando que Alice transfiere directamente a Carol.

Decentraland tiene una asociación con [Polygon](https://polygon.technology/), que proporciona su propia cadena lateral en Ethereum. Su cadena lateral puede ser utilizada por las escenas para soportar transacciones rápidas y muy baratas.

## Prueba la Testnet de Polygon

Para usar la Matic Testnet de Polygon, primero debes tener:

* Ether en la red Sepolia. Puedes obtenerlo gratis de varios faucets externos como [este](https://www.alchemy.com/faucets/ethereum-sepolia).

Luego debes configurar tu cuenta de MetaMask para incluir la Matic Testnet.

Para hacerlo:

1. Cambia de red en MetaMask haciendo clic en el nombre de la red actual y seleccionando *Custom RPC* en la parte inferior del dropdown.
2. Rellena 'Polygon Amoy Testnet' como *Nombre de la red*, [**https://rpc-amoy.polygon.technology**](https://rpc-amoy.polygon.technology) como *Nueva URL de RPC* y [**https://amoy.polygonscan.com**](https://amoy.polygonscan.com) como *URL del explorador de bloques*. Para *Símbolo* establece **POL**
3. Haz clic en 'Save' para añadir esta nueva red a MetaMask.


> [!WARNING]
> **📔 Nota**: Obtén más información sobre cómo configurar Polygon en su [página de documentación](https://docs.polygon.technology/)



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/blockchain/second-layer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
