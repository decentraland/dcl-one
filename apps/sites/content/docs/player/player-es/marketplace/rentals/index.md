> Alquileres de LAND

# Alquileres

## Glosario

**Propietario de LAND:** Cuenta (dirección) que posee LAND; puede ser un Parcel, un Estate o ambos.

**Inquilino:** Cuenta (dirección) que alquila LAND a un Land Owner. Esta es también la única cuenta que puede cambiar la dirección que tiene Operator Permissions.

**Permiso de operador:** La dirección con este permiso es la única que puede desplegar scenes en ese LAND.

**Transacciones:** Transacciones en la blockchain de Ethereum que cuestan gas.

## Introducción

El nuevo sistema de alquiler permite a los LAND Owners y Tenants **alquilar LAND de forma segura y sin necesidad de confianza** mediante una combinación de firmas que se almacenan en un servidor administrado por la Decentraland Foundation (off-chain) y transacciones de Ethereum (on-chain).

Por ejemplo, un DJ podría encontrar un Parcel atractivo de LAND, alquilarlo y desplegar una discoteca para tocar todos los sábados. Una universidad podría alquilar un Estate y construir un campus para sus estudiantes.

Abajo encontrarás todos los pasos que debes seguir para alquilar un LAND, y las transacciones involucradas para ambas partes.

## Para los LAND Owners

### Listar LAND en alquiler

Como Land Owner, puedes listar tu LAND (Parcels o Estates) en alquiler en [Marketplace](https://market.decentraland.org/) > My Assets > LAND.

Para hacer esto on-chain, el Land Owner debe aprobar al Rent Smart Contract para usar el LAND en su nombre. Luego, cada publicación también necesitará una firma del Owner.

![](/docs/_remote/1a34b8c7a8542b15716940f1.webp)

Puedes establecer un precio de alquiler por día en MANA y la cantidad de días durante los que quieres permitir que las personas lo alquilen. El precio por día multiplicado por el número de días del período es lo que el tenant pagará **por adelantado, y en total** por ese alquiler.

![](/docs/_remote/d0b5696229c03215afc73455.webp)

Después de definir el precio por día, debes seleccionar el número de días durante los cuales los Users pueden alquilar tu LAND. Por ejemplo, si seleccionas solo 7 y 30 días, el Tenant solo podrá elegir entre esas 2 opciones. En caso de que el Tenant seleccione la opción de 30 días, esa será la duración del alquiler desde el día en que se confirme.

![](/docs/_remote/f0859bad5d58c27a481d636a.webp)

También puedes establecer una fecha de expiración para la publicación. Esto significa que, si el LAND no se alquiló hasta la fecha seleccionada, la publicación se eliminará del Marketplace. Además, el smart contract rechazará la firma caducada para que nadie pueda alquilarlo por el precio y la duración de la publicación seleccionados anteriormente. Esta es una medida de seguridad para evitar que se alquile por un precio o una duración no deseados.

![](/docs/_remote/f4c314e8b64de94a0f2045e5.webp)

Después de establecer el precio, el período de alquiler y la fecha de expiración de la publicación, tu LAND aparecerá como disponible para alquilar en el Marketplace.


> [!NOTE]
> 💡 Cuando LAND es alquilado por un Tenant, no se puede vender hasta que se recupere. Tampoco se pueden recibir ofertas de compradores potenciales.



> [!NOTE]
> 💡 El Voting Power lo conserva el Land Owner, aunque esté alquilado.


### Editar o cancelar una publicación

Después de que el LAND se haya publicado para alquiler en el Marketplace, y antes de que alguien lo alquile, puedes editar las condiciones de la publicación haciendo clic en el icono del lápiz en el detalle del LAND. También puedes eliminar la publicación del Marketplace y de la blockchain.

![](/docs/_remote/89bcf7626763f867ddbc8c00.webp)


> [!NOTE]
> 💡 Editar y cancelar requieren una transacción, que cuesta gas. Consulta la sección Transactions más abajo para más detalles.


### Después de que termine el alquiler

Después de que termine el alquiler, puedes **Reclamar de vuelta tu LAND o volver a listarlo para alquilar**.

**Operator Permissions no se transfieren automáticamente de vuelta al Land Owner**. Para recuperarlos, el Land Owner tiene que reclamar el LAND enviando esa transacción y pagando la comisión de gas. Confirmar la transacción quitará los Operator Permissions del Tenant y se los devolverá al Land Owner.

![](/docs/_remote/f43022686a5bd15b6c02ac8a.webp)

La otra posibilidad es volver a listar el LAND para alquilar, en lugar de reclamarlo de vuelta. Esto no requerirá pagar otra transacción, pero **los Operator Permissions se mantendrán en manos del Tenant anterior hasta que un nuevo Tenant confirme un nuevo alquiler.**

El Land Owner puede editar el precio, el período de alquiler y la fecha de expiración de la publicación para la nueva publicación.

![](/docs/_remote/6f6b27950ff42602c5b34fda.webp)

Ambas acciones se pueden realizar desde la página de detalles del LAND en el Marketplace.

![](/docs/_remote/2b6877fde55556d4100fd7e0.webp)

### Estado de alquiler

Puedes comprobar el estado de cualquier LAND alquilado en My Assets > Store > On Rent. Los posibles estados son:

* Listado para alquilar - La publicación fue confirmada y está disponible para que los usuarios la alquilen en el Marketplace
* Período de alquiler finalizado - En esta etapa, el LAND está disponible para ser reclamado de vuelta o volver a listarlo para alquilar por el Land Owner
* Alquilado hasta *"date"* - El LAND ya está alquilado y el Tenant tiene Operator Permissions hasta que se reclame de vuelta o lo alquile otro usuario

![](/docs/_remote/486197d9f7fa915ceb7f0183.webp)

## Para los Tenants

### Alquilar LAND

Todos los usuarios pueden encontrar LAND listado para alquilar en el Marketplace bajo la sección LAND.

![](/docs/_remote/95cd665129b4cbe803dc76df.webp)

Hay LANDs que están disponibles para Sale o Rent. En caso de que ambas opciones estén disponibles, puedes ver las condiciones de cada una haciendo clic en el toggle Sale/Rent.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

Una vez que encuentres el LAND que quieres alquilar, debes seleccionar el Rent Period; estos son los días que tendrás el LAND. Después de seleccionar el Rent Period, verás el precio total que se debe pagar por el alquiler.

![](/docs/_remote/bbaf9e44f6b62ace814692a3.webp)

Tendrás que aprobar al Rent Smart Contract para que tome el MANA de tu cuenta antes de continuar.

Antes de confirmar el alquiler, puedes decidir quién administrará el LAND (Operator Permission). Puedes ser tú mismo o cualquier otra dirección que elijas.

![](/docs/_remote/2d83c8c5fc075dc07df8532c.webp)

El Operator Permission puede ser cambiado más adelante por el Tenant (la dirección que alquiló el LAND en primer lugar) desde el [Builder](https://builder.decentraland.org/).

![](/docs/_remote/506dbf3015f1fd2ea5698b48.webp)

Después de seleccionar todos los detalles y aprobar al Rent Smart Contract para manejar tu MANA, puedes confirmar el alquiler enviando una transacción.

¡Y ya está! Puedes empezar a trabajar en tu LAND y desplegar una scene usando el Builder o el SDK.

![](/docs/_remote/76317d506da755beb44eeaf7.webp)

Nota: después de que termine el alquiler, el Tenant seguirá teniendo Operator Permissions hasta que el Land Owner lo reclame de vuelta, o alguien más lo alquile. **Asegúrate de guardar tu contenido antes de que termine el alquiler; de lo contrario, podría perderse.**


> [!NOTE]
> 💡 Alquilar LAND no transfiere Voting Power al Tenant. El Voting Power lo conserva el Land Owner, según lo definido por la DAO en esta [Proposal](https://governance.decentraland.org/proposal/?id=c98bd010-74b1-11ed-a9bf-f772a12a0556)


## Transactions

Por el bien de **la seguridad y la descentralización**, el sistema de alquiler depende de la blockchain de Ethereum como fuente de verdad.

Pero no todas las acciones involucradas requieren una entrada en la blockchain. Si ese fuera el caso, sería demasiado costoso para ambas partes.

Las transacciones en la blockchain son mínimas para proporcionar un **sistema robusto y sin necesidad de confianza para el alquiler de LAND, manteniéndolo asequible.** Estas son todas las transacciones que hay que considerar:

### Para los Land Owners

#### Listar para alquilar

Antes de listar el primer Parcel o Estate para alquilar, los LAND Owners necesitan permitir que el Rent Smart Contract opere el LAND en su nombre. Esto solo tiene que hacerse una vez para los Parcels y solo una vez para los Estates.

![](/docs/_remote/24e324959f412a5ef38535ed.webp)

#### Reclamar LAND de vuelta o volver a listarlo para alquilar

Después de que termine el período de alquiler, **Operator Permissions no se transfieren automáticamente de vuelta al Land Owner**. Para recuperarlos, el Land Owner tiene que reclamar el LAND enviando esa transacción y pagando la comisión de gas.

![](/docs/_remote/9b7b0037cff4903f1d9e7022.webp)

Otra posibilidad es volver a listar el LAND para alquilar, en lugar de reclamarlo de vuelta. Esto no requerirá pagar otra transacción, pero Operator Permissions se mantendrán en manos del Tenant anterior hasta que un nuevo Tenant confirme un nuevo alquiler.

#### Editar publicación

Si se cambia el precio, el período de alquiler o la fecha de expiración, el Land Owner debe enviar una transacción para protegerse de que alguien use directamente la firma de la publicación anterior en el Smart Contract (no desde la UI del Marketplace) y lo obtenga a un precio más bajo del deseado o por una duración no deseada.

![](/docs/_remote/985e0faa31bf88ac207e0846.webp)

### Para los Tenants

#### Permitir que el Rent Contract opere tu MANA

Tanto si se trata de un Parcel como de un Estate, todo usuario que quiera alquilar LAND tiene que enviar una transacción para permitir que el Rent Smart Contract opere MANA en su nombre. Esto es necesario porque el Smart Contract tiene que tomar el MANA y transferirlo al Land Owner cuando se activa el alquiler. Esto se hace solo una vez para todo el LAND que se vaya a alquilar a partir de ese momento.

#### Alquilar LAND

Después de aprobar al Rent Smart Contract para operar tu MANA, estás listo para confirmar tu primer alquiler. Una vez que encuentres el LAND que quieres, elige el período de alquiler y confirma la transacción de alquiler; los Operator Permissions se transfieren a la dirección seleccionada.

Si quieres alquilar otro Parcel o Estate, solo necesitas enviar una transacción para confirmarlo; no es necesario aprobar de nuevo al Smart Contract para que opere tu MANA.

![](/docs/_remote/34fb306b490b37363a4d6562.webp)

#### Cambiar operador

En el momento de alquilar el LAND, el usuario puede elegir qué dirección tendrá Operator Permissions para ese LAND. Si esa dirección quiere cambiarse, se debe enviar una transacción.

![](/docs/_remote/e38a6d9159fa451ba7b2570b.webp)

## Carteras de Smart Contract

La función de Rentals depende en gran medida de firmas off-chain. Las acciones off-chain permiten a los Land Owners listar LANDs en alquiler sin pagar el costo de la transacción.

Al firmar una publicación, el Rent Smart Contract puede verificar que la publicación fue creada por quien la firmó.

La firma tiene la particularidad de que requiere una clave privada. Todas las EOA (Externally Owned Accounts) tienen una, y pueden firmar publicaciones con ella. Luego, el Rentals Smart Contract verificará la firma generada por la EOA al ejecutar un alquiler.

Las Smart Contract Wallets, que son Smart Contracts, no tienen una clave privada y, por lo tanto, no pueden firmar mensajes. En su lugar, una EOA autorizada por la Smart Contract Wallet debe firmar.

Para admitir estas firmas, el Rent Smart Contract verifica con la Smart Contract Wallet si la firma es válida siguiendo el [EIP-1271](https://eips.ethereum.org/EIPS/eip-1271) estándar. Si la firma es válida, el alquiler puede ejecutarse.

La Smart Contract Wallet no solo debe tener definido el método estándar de verificación de firmas en el EIP-1271, sino también el método receptor de tokens definido en el [estándar EIP-721](https://eips.ethereum.org/EIPS/eip-721). Esto es necesario al reclamar de vuelta LAND porque el Rent Smart Contract llamará a un `safeTransferFrom` para devolver el NFT a la Smart Contract Wallet, y si no ha implementado la `onERC721Received` función apropiada, fallará al recuperar el LAND.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/marketplace/rentals.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
