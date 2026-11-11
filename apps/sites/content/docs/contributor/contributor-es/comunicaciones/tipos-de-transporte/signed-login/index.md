# Inicio de sesión firmado

El transporte Signed Login puede ser usado por servidores que quieran personalizar la forma en que asignan clientes a islas, reemplazando o envolviendo Archipelago en su arquitectura.

En lugar de conectarse directamente a un backend en tiempo real, Signed Login hace una [signed fetch](/docs/contributor/contributor-es/autenticacion/signed-fetch/) para obtener la cadena de conexión. Este paso intermedio permite a los servidores emplear la estrategia de asignación que quieran.

Los URI de Signed Login tienen esta forma:

```
signed-login:https://comms.example.com/auth&param=value
```

La parte después de `signed-login:` prefijo puede ser cualquier URL válida.

### Uso

Una solicitud de Signed Login se construye usando el [signed fetch](/docs/contributor/contributor-es/autenticacion/signed-fetch/) mecanismo.

La respuesta, si tiene éxito, tendrá el estado `200` y un cuerpo JSON con *al menos* los siguientes campos:

| Campo          | Tipo     | Valor                                                          |
| -------------- | -------- | -------------------------------------------------------------- |
| `fixedAdapter` | `string` | El URI de transporte asignado (p. ej. `livekit:` o `ws-room:`) |

Por ejemplo:

```
{ 
  fixedAdapter: "ws-room:wss://comms.example.com/rooms/17"
}
```

En caso de fallo, la respuesta puede tener cualquier código de estado apropiado y un cuerpo JSON con *al menos* los siguientes campos:

| Campo     | Tipo     | Valor                                                   |
| --------- | -------- | ------------------------------------------------------- |
| `message` | `string` | (Opcional) una explicación o código de error del fallo. |

Las respuestas con campos adicionales en el cuerpo JSON son válidas, y el contenido del error `message` queda sin especificar. Las implementaciones pueden usar esta flexibilidad para integrar `signed-login:` en sus aplicaciones como consideren conveniente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/comunicaciones/tipos-de-transporte/signed-login.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
