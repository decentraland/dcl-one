# Ejemplos de CLI

Esta práctica demuestra cómo interactuar con la API de contenido en una terminal. Usaremos la instancia de la Decentraland Foundation en `peer.decentraland.org`, y todo lo que necesitas es el `curl` herramienta de línea de comandos.


> [!NOTE]
> Puedes usar [`httpie`](https://github.com/httpie/httpie) o formatear las respuestas con [`jq`](https://github.com/stedolan/jq) para obtener JSON más legibles con cada solicitud. En esta guía, mostraremos documentos formateados por claridad.


Esto es lo que haremos:

1. Consultar el estado del servidor de contenido.
2. Localiza y descarga un snapshot diario [instantánea](/docs/contributor/contributor-es/contenido/snapshots/) con una lista de entities.
3. Obtén el manifiesto de un entity.
4. Descarga uno de los archivos del entity.

Empecemos consultando el estado del server usando `/about`:

```bash
curl "https://peer.decentraland.org/about"
```

```js
{
  "healthy": true,
  "content": {
    "healthy": true,
    "version": "6.5.0",
  },
  // ... más información del server (feature flags, versiones, rutas, etc.)
}
```

Parece que el server está activo y funcionando normalmente (`"healthy": true`), y nos está dando información sobre la versión que implementa para cada conjunto de features, además de algunas opciones de configuración de la instancia.

Nos interesa descargar algo de contenido, así que exploremos los [instantáneas](/docs/contributor/contributor-es/contenido/snapshots/) para hacernos con algunos identificadores a través de `/content/snapshots`.

```bash
curl "https://peer.decentraland.org/content/snapshots"
```

```js
[
  {
    "hash": "bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224",
    "timeRange": {
      "initTimestamp": 1689120000000,
      "endTimestamp": 1689206400000
    },
    "replacedSnapshotHashes": [],
    "numberOfEntities": 981,
    "generationTimestamp": 1689219353866
  },
  // ...más archivos de snapshot
]
```

Cada elemento del array describe un [instantánea](/docs/contributor/contributor-es/contenido/snapshots/). Vamos a tomar un `hash` y descargar el archivo desde el `/content/contents` endpoint.

```bash
curl "https://peer.decentraland.org/content/contents/bafybeia6qoum64psaooiqo3f45i6hykfwx723uc236waub3gng2naof224" > snapshot
```


> [!NOTE]
> Puedes experimentar con snapshots más grandes, como en el [ejemplo avanzado de Python](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py). Para probar esto de forma interactiva, probablemente quieras uno de los más pequeños.


Podemos comprobar que descargamos el archivo correcto en un formato que conocemos, mirando la primera línea:

```bash
head -n1 snapshot
```

```
### instantánea JSON de Decentraland
```

¡Genial! Ahora tenemos un resumen local de todos los entities que fueron capturados en ese snapshot. Tomemos el primero de la lista (la segunda línea del archivo), un `perfil`:

```bash
tail -n+2 snapshot | head -n1 
```

```js
{
  "entityId": "bafkreif7hjremkxlvixyxoxnoo7bdcnf7qqp245sjb2pag2nk3n6o6yc4c",
  "entityType": "profile",
  "pointers": [
    "0x271cdb3b1c792c336c4b2bdc52c4f415d0046b92"
  ],
  "authChain": [
    // Ver https://docs.decentraland.org/contributor/auth/authchain/
  ],
  "entityTimestamp": 1689120135624
}
```


> [!NOTE]
> Dado que los snapshots expiran y los entities se reemplazan, los identificadores en este artículo no funcionarán. Sigue en tu línea de comandos para obtener IDs de archivo reales y activos.


Esta es información que podríamos guardar. Usaremos el `entityId` para descargar el manifiesto JSON del entity, pero conservar los [pointer](/docs/contributor/contributor-es/contenido/pointers/) es una buena idea si queremos localizar este entity y cualquier versión actualizada en el futuro.

También tenemos el [cadena de autenticación](/docs/contributor/contributor-es/contenido/entities/#ownership) usado para firmar este entity, y podríamos validar las firmas listadas para verificar la autenticidad de cualquier archivo relacionado que descarguemos.

Vamos a hacernos con el manifiesto del entity. Recuerda, el `entityId` es el [identificador del archivo](/docs/contributor/contributor-es/contenido/file-system/#identifiers) archivo que necesitamos, y podemos usar el `/content/contents` endpoint de nuevo:

```bash
curl "https://peer.decentraland.org/content/contents/bafkreigcreq7rv6b2wf4zc4fsnif43ziwb4q46v4qhsewpf7gbsyxew3om"
```

```js
{
  "version": "v3",
  "type": "profile",
  "pointers": [
    "0x273cdb3b1c791c336c4b2bcc52c4f415d0046b91"
  ],
  "timestamp": 1689120135624,
  "content": [
    {
      "file": "body.png",
      "hash": "bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u"
    },
    // ...más archivos
  ],
  "metadata": {
    // ...avatares y otra información
  }
}
```

Este `perfil` entity tiene toda la información que World Explorers usa para renderizar y animar a un jugador. Si nos interesa obtener uno de los archivos empaquetados, podemos seguir usando el `/content/contents` endpoint.

Si miramos el `content` campo, podemos ver el `hash` del archivo llamado internamente `body.png`. Vamos a obtenerlo:

```bash
curl "https://peer.decentraland.org/content/contents/bafybeibzaqkirz7fk474xvyhurho5xviphs7anawaceb6gscuigia4x33u" > body.png
```

Podemos abrir este `png` archivo en un visor de imágenes o navegador web, y echar un vistazo al trabajo del autor. ¡Genial!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/practica/cli-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
