# Ejemplos de Python

Esta práctica muestra cómo escribir un programa simple que descarga y analiza algo de contenido usando el [instantáneas](/docs/contributor/contributor-es/contenido/snapshots/) proporcionado por los servidores de contenido.


> [!NOTE]
> Puedes encontrar el [script completo](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) en GitHub, junto con un [ejemplo más avanzado](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


Usaremos el servidor de la Fundación Decentraland en `peer.decentraland.org`, y Python 3 como nuestro lenguaje de preferencia.

Esto es lo que haremos:

1. Consultar el estado del servidor de contenido.
2. Seleccionar y descargar una [instantánea](/docs/contributor/contributor-es/contenido/snapshots/) con una lista de entities.
3. Imprimir el tipo y el ID de todas las entities referenciadas.

Empecemos nuestro script con algunos preparativos. Usaremos solo módulos de la biblioteca estándar, pero en código real probablemente querrás un cliente HTTP más cómodo (como la [requests](https://github.com/psf/requests) librería).

```py
# Realizar una solicitud HTTP GET, devolver una respuesta HTTP similar a un archivo.
def fetch(path):
    url = f"https://peer.decentraland.org/{path}"
    headers = { "User-Agent": "urllib" } # importante en algunos servidores (si está vacío, 403 Forbidden)

    request = urllib.request.Request(url, headers=headers)
    response = urllib.request.urlopen(request)

    return response
```

Nuestra simple ayuda realiza una `GET` HTTP, y devuelve el objeto de respuesta similar a un archivo. Nada sofisticado. Usemos esto para llamar al `/about` endpoint y comprobar el estado del servidor:

```py
# Comprobar el estado del servidor:
about = json.load(fetch('about'))

if not about['healthy']:
    print("¡El servidor no está saludable!")
    sys.exit(1)
```

Si superamos este punto, el servidor está en funcionamiento (obtuvimos una `200` respuesta) y reporta estar operativo. Podemos solicitar el conjunto actual de instantáneas (que viene en formato de arreglo JSON):

```py
# Obtener la lista de instantáneas:
all_snapshots = json.load(fetch('content/snapshots'))
```

Los archivos de instantáneas (especialmente para los rangos de tiempo más largos) pueden ser muy grandes. Para un experimento rápido, tomemos la más pequeña de la lista por `numberOfEntities`:

```py
# Tomar la instantánea más pequeña, en términos de entities incluidas:
snapshot = min(all_snapshots, key=lambda s: s['numberOfEntities'])
```

Para descargar el contenido, necesitamos el `hash` campo de `instantánea`. Obtenemos la URL del archivo añadiéndola a la raíz del contenido:

```py
# Solicitar el archivo desde la API de contenido:
response = fetch('content/contents/' + snapshot['hash'])
```

El archivo que seleccionamos es lo suficientemente pequeño como para almacenarlo en memoria, pero fingiremos que no lo sabemos y lo transmitiremos en flujo. La primera línea es el encabezado de la instantánea, y cada línea después de esa contiene un objeto JSON.

Comprobemos el encabezado, siempre es una buena idea:

```py
# Verificar el encabezado de la instantánea:
header = response.readline().decode('utf-8').strip()

if header != '### Decentraland json snapshot':
    print("Encabezado de instantánea inválido: " + header)
    sys.exit(1)
```

Ahora podemos procesar todas las entities en la instantánea, leyendo la respuesta línea por línea. Para nuestros modestos propósitos, *procesar* significa imprimir el tipo y el ID de la entity:

```py
# Leer y decodificar todos los ítems, un JSON por línea:
for line in response:
    item = json.loads(line)
    print(item['entityType'], item['entityId'])
```

Este bucle comenzará a transmitir, analizar y imprimir líneas como estas hasta terminar con la instantánea:

```
profile bafkreic36qmzyprs6whkpuxbeiif4no6kvdrr2tfpichbx2fzfz5py6eyv
scene bafkreibr5xfujqrp5q3o4s73vm2yljlcp7cucqgugssnarsuclxv4emlmy
profile bafkreid7khr5wnkialba44rsslffi633rh3lvctad5oa5vjoe6wa7s4c5a
wearable bafkreihlqcb7jgubomyidikpwpqhgzbagltk5m4rgbjdvzydxmoka7bg4i
```

¡Salud! Hemos usado el sistema de snapshots para explorar parte del contenido disponible en Decentraland.

Recuerda que puedes encontrar el [script completo](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) en GitHub, junto con un [ejemplo más avanzado](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/contenido/practica/python-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
