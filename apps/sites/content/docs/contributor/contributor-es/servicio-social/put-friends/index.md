# PUT de amigos

Interactúa con la API de Social Service para realizar acciones relacionadas con eventos y usuarios. Estas acciones incluyen crear nuevas solicitudes de amistad, aceptar o rechazar solicitudes entrantes y eliminar amistades. Social Service permite a los usuarios gestionar sus relaciones e interacciones dentro de la plataforma.

### Amistades y acciones

Las amistades dentro de Social Service son relaciones bidireccionales entre usuarios. Se pueden realizar acciones para actualizar estas amistades, como enviar solicitudes de amistad, aceptar o rechazar solicitudes, cancelar solicitudes y eliminar amistades. Cada una de estas acciones tiene implicaciones específicas para la relación entre usuarios.

### Implementación de actualizaciones de amistad

Para realizar estas acciones relacionadas con la amistad, se debe utilizar el `UpdateFriendshipEvent` mensaje. Este mensaje permite ejecutar varias acciones relacionadas con las amistades, como solicitar, aceptar, rechazar, cancelar y eliminar.

La estructura exacta del payload y los parámetros requeridos para cada acción se especifican en el `UpdateFriendshipEvent` mensaje.

**Ejemplo de uso**

Aquí tienes un ejemplo de cómo podrías usar el `UpdateFriendshipEvent` para enviar una solicitud de amistad:

```proto
message UpdateFriendshipEvent {
  string userAddress = 1;
  string targetUserAddress = 2;
  FriendshipAction action = 3;
}
```

En este ejemplo, `userAddress` representa la dirección del remitente, `targetUserAddress` representa la dirección del destinatario, y `action` indica el tipo de acción que se va a realizar (solicitar, aceptar, rechazar, cancelar o eliminar).

#### Envío de solicitudes de amistad

Cualquier usuario puede enviar una solicitud de amistad a otro usuario. Sin embargo, para establecer una amistad, el usuario receptor debe aceptar la solicitud.

```javascript
requestFriendship: async (address: string, message?: string) => {
  const response = await service.updateFriendshipEvent({
    event: { request: { user: { address }, message } },
    authToken: { synapseToken }
  })
  processErrors(response)
  return response.event
}
```

#### Aceptación o rechazo de solicitudes

Cuando un usuario recibe una solicitud de amistad, tiene la opción de aceptarla o rechazarla. Aceptar una solicitud establece una amistad mutua entre los usuarios. Rechazar una solicitud deniega el establecimiento de una amistad.

```javascript
acceptFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      event: { accept: { user: { address } } },
      authToken: { synapseToken }
    })
  )
  processErrors(response)
  return response.event
}
```

```javascript
rejectFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      authToken: { synapseToken },
      event: { reject: { user: { address } } }
    })
  )
  processErrors(response)
  return response.event
}
```

#### Cancelación de solicitudes de amistad

Si un usuario decide retractarse de una solicitud de amistad enviada antes de que sea aceptada, puede cancelarla. Esto evita que el destinatario vea o responda a la solicitud.

```javascript
cancelFriendshipRequest: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      event: { cancel: { user: { address } } },
      authToken: { synapseToken }
    })
  )
  processErrors(response)
  return response.event
}
```

#### Eliminación de amistades

Cualquiera de los usuarios en una amistad mutua tiene la opción de eliminarla. Cuando se elimina una amistad, se borra de los registros de ambos usuarios, poniendo fin a la relación mutua.

```javascript
removeFriend: async (address: string) => {
  const response = await service.updateFriendshipEvent(
    UpdateFriendshipPayload.create({
      authToken: { synapseToken },
      event: { delete: { user: { address } } }
    })
  )
  processErrors(response)
  return response.event
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/servicio-social/put-friends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
