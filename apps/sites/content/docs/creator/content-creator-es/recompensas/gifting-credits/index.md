> Gifting Credits es un programa beta solo por invitación: los estudios participantes regalan Credits a sus jugadores desde la página de Studio de la tienda.

# Regalar Credits (Beta)

Regalar Créditos es actualmente un programa beta solo por invitación. Los estudios participantes pueden regalar Créditos a sus jugadores.

Si tu estudio ha sido invitado a participar, puedes regalar Créditos a través de la página de Studio de la Shop en [decentraland.org/shop/studio](https://decentraland.org/shop/studio). Los jugadores pueden gastar estos Créditos en cualquier cosa de la Shop.

## Antes de empezar

* **Tu cuenta debe administrar el estudio.** Decentraland agrega las cuentas que pueden regalar para tu estudio. Si inicias sesión y ves *Esta cuenta no administra un estudio*, pídele a tu contacto de Decentraland que agregue esa cuenta.
* **Las cuentas del propio estudio no pueden recibir sus Créditos.** Usa una cuenta diferente para cualquier regalo de prueba.
* **Los regalos son definitivos.** Los Créditos regalados nunca caducan y no se pueden revertir, así que revisa la lista antes de confirmarla.

## Tu página de Studio

Inicia sesión en la página de Studio en [decentraland.org/shop/studio](https://decentraland.org/shop/studio). Si tu cuenta administra más de un estudio, elígelo en el **Studio** menú. La página muestra:

* **Presupuesto restante**: los Créditos que tu estudio aún puede regalar.
* **Regalados hasta ahora**: lo que tu estudio ha regalado y en cuántos regalos.
* **Máximo por jugador**: el regalo más grande que un solo jugador puede recibir a la vez.
* **Regalos**: cada regalo que hizo tu estudio, con el jugador, los Créditos, el motivo, la fecha y quién lo envió.

![La página de Studio, con el presupuesto restante, los Créditos regalados hasta ahora, el máximo por jugador y la lista de regalos](/docs/_remote/eca2a726b6b3c94c7710e0b5.webp)

Si la página dice que el estudio está en pausa, no puede regalar hasta que Decentraland lo reanude.

## Regalar Créditos

1. Presiona **Regalar Créditos**.
2. Escribe un **Motivo**. Se usa para cada jugador que no tenga un motivo propio: una línea, hasta 80 caracteres, como *Mejor jugador de la semana*.
3. Agrega a los jugadores, uno por fila: su cuenta (la `0x…` dirección) y los Créditos a regalar. Puedes asignar a una fila su propio motivo.
   * **Agregar jugador** agrega una fila.
   * **Pega una lista** toma un jugador por línea: cuenta, Créditos y un motivo opcional. Las columnas copiadas desde una hoja de cálculo también funcionan.

     ```
     0x…, 100, Mejor jugador de la semana
     0x…, 50
     ```
   * Escribe los importes sin separadores de miles (`1000`, no `1,000`).
   * Cada cuenta va solo en una fila: si un jugador recibe dos importes, súmalos.
4. Revisa el resumen debajo de la lista (jugadores, Créditos totales y el presupuesto restante después), y luego presiona **Revisar**.

   ![La ventana de Regalar Créditos con un motivo y dos jugadores, cada uno con sus propios Créditos y motivo](/docs/_remote/24ad20ec86951563169233a0.webp)
5. La revisión te advierte sobre cualquier jugador que haya recibido el mismo regalo (mismos Créditos y motivo) de tu estudio en los últimos 30 días. Vuelve atrás y elimínalo, a menos que quieras regalarle de nuevo.
6. Escribe el total de Créditos para confirmar y presiona **Regalar a … jugadores**.

   ![El paso de confirmación, con el total de Créditos escrito y el botón Regalar a 2 jugadores](/docs/_remote/f677deb1676df2cd4f1c5912.webp)

Los regalos se envían uno por uno y cada fila muestra cómo fue. Si la ventana se cierra o detienes la lista, abre **Regalar Créditos** de nuevo y presiona **Continuar**: los jugadores que ya recibieron su regalo nunca lo reciben dos veces.

## Lo que reciben tus jugadores

* Una notificación en Decentraland diciendo que tu estudio les regaló Créditos.
* Un correo electrónico, si han confirmado su correo en su cuenta de Decentraland.
* El regalo en su actividad de Shop y los Créditos en su saldo, listos para gastar.

## Si un regalo no se procesa

| Mensaje                              | Qué hacer                                                                                                                                                |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Esta cuenta pertenece a tu estudio   | Las cuentas del propio estudio no pueden recibir sus Créditos. Elimina la fila.                                                                          |
| Esta cuenta no puede recibir regalos | Este jugador no puede recibir Créditos. Elimina la fila.                                                                                                 |
| Por encima del máximo por jugador    | Reduce el importe al **Máximo por jugador** que se muestra en tu página de Studio.                                                                       |
| No queda suficiente presupuesto      | Regala menos, o habla con tu contacto de Decentraland sobre tu presupuesto.                                                                              |
| El estudio está en pausa             | Espera a que Decentraland reanude el estudio, luego presiona **Continuar**.                                                                              |
| No recibimos una confirmación        | Consulta **Regalos** la lista antes de enviarla de nuevo: es posible que el regalo ya esté allí. Pulsar **Continuar** nunca le regala dos veces a nadie. |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/recompensas/gifting-credits.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
