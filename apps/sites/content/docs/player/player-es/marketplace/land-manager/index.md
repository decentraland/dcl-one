> Gestiona tokens de LAND y Estate

# Gestor de LAND

El Land Manager te permite gestionar tus activos de LAND y Estate.

Accede al Land Manager en <https://builder.decentraland.org/land>.

El Land Manager te permite:

* **Nombrar** tus parcelas y Estates y darles una descripción pública.
* **Fusionar** parcelas de LAND en un Estate.
* **Disolver** un Estate en parcelas de LAND separadas.
* **Transferir** tus parcelas y Estates a otro usuario.
* **Conceder permisos** a otros usuarios para editar las parcelas que posees.

## Gestiona tu LAND

Para ver tus tokens de LAND, haz clic en **Gestiona tu LAND**. Aquí encontrarás una lista de todas tus parcelas y Estates, incluidas las parcelas que has puesto a la venta.

Al hacer clic en una de las parcelas o Estates que aparecen debajo de **Land**, puedes editar su nombre, descripción, establecer un operador o transferirlo directamente a otra dirección de wallet.

![](/docs/_remote/2442e30de2eeb3bfbb62d341.webp)

## Crear un Estate

Los Estates de LAND permiten asociar dos o más parcelas de LAND directamente adyacentes para facilitar la gestión de tus grandes tenencias de LAND. Los Estates son especialmente útiles al construir escenas grandes que abarcan más de una parcela.

Las parcelas de un Estate deben ser directamente adyacentes y no pueden estar separadas por una calle, una plaza ni por ninguna otra parcela.

Para crear tu primer Estate, necesitas poseer dos o más parcelas de LAND adyacentes.

1. Abre **Mi LAND** y selecciona una de las parcelas que te gustaría añadir al Estate.
2. En la página de detalles de la parcela, haz clic en **Crear Estate**.
3. Se te mostrará una vista del Atlas centrada en la parcela que seleccionaste, con las demás parcelas adyacentes que posees resaltadas. Selecciona las diferentes parcelas que quieres incluir en tu Estate. ![](/docs/_remote/4ef95117e0b2e23f246636d8.webp) ![](/docs/_remote/5d4f1b545b9d7baf0d140f45.webp)
4. Haz clic en **Continuar**.
5. Introduce un nombre y una descripción para tu Estate. Estos detalles se mostrarán públicamente en el Atlas, igual que el nombre y la descripción de cualquier parcela individual.

   ![](/docs/_remote/75a307fdd4e3535bf406e003.webp)
6. Confirma esta transacción en tu cliente Ethereum y espera a que la red la verifique.

Una vez que hayas creado tu primer Estate, verás una nueva pestaña titulada Estates. Desde esta página puedes ver y gestionar todos tus Estates.

Cuando creas un nuevo Estate, en realidad estás transfiriendo tus parcelas a un nuevo token. Estos Estates están representados por tokens ERC721 (como cualquier otro NFT). Ya no verás las parcelas individuales en *Mi LAND*, y no aparecerán en las wallets MetaMask, Mist, Trezor o Ledger, ni en Etherscan bajo tu dirección.

## Editar parcelas o Estates

Puedes editar el nombre y la descripción de cualquier parcela o Estate que poseas. Estos detalles se mostrarán públicamente en el Atlas.

Para editar una parcela o Estate:

1. Ve a la página de detalles de la parcela o del Estate que quieras editar y haz clic en **Editar**.

   ![](/docs/_remote/3dc2640de68eb6f3a61f97b6.webp)
2. Haz clic en **Enviar**.
3. Confirma esta transacción en tu cliente Ethereum y espera a que la red la verifique.

## Dar permisos

Puedes dar a otro usuario permisos para editar el contenido de una parcela o Estate. Esto permite que ese usuario despliegue código en la escena, sin tener la capacidad de vender el token.

El usuario con permiso también puede cambiar el nombre o la descripción en el Marketplace.

Para conceder permisos sobre tu LAND:

1. Ve a la página de detalles de la parcela o Estate y haz clic en los tres puntos y luego en **Establecer operador**. ![](/docs/_remote/1ebd7dd1f5d8b72647811f3b.webp)
2. Completa el formulario con la información deseada.

   ![](/docs/_remote/8de80dd1532ffea8a620d5b2.webp)
3. Haz clic en **Enviar**.
4. Confirma esta transacción en tu cliente Ethereum y espera a que la red la verifique.

## Consulta tu historial de actividad

Abre la página de notificaciones haciendo clic en el icono de la campana en la parte superior de la pantalla.

![](/docs/_remote/350d35176446e1c73654bfae.webp)

La página de notificaciones muestra una lista de todas las transacciones recientes que has realizado, junto con su estado.

Haz clic en una transacción para ver más detalles sobre ella en Etherscan.

## Transferir LAND

Para transferir una parcela de LAND o un Estate a otro usuario:

1. Ve a la página de detalles de la parcela o Estate que quieras transferir y haz clic en **Transferir**.

   ![](/docs/_remote/d4abb6744a7112af5514a3f1.webp)
2. Introduce la dirección pública de la wallet Ethereum del destinatario.


> [!WARNING]
> **📔 Nota**: Por favor, comprueba dos veces esta dirección, ya que no puedes cancelar la operación. Aunque el destinatario siempre podría transferirte de vuelta el LAND, el propietario original no puede revertir la acción.


![](/docs/_remote/186df165a7bc48578054cfb9.webp)

3. Haz clic en **Enviar**.
4. Confirma esta transacción en tu cliente Ethereum y espera a que la red la verifique.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/marketplace/land-manager.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
