# Exemplos em Python

Esta prática mostra como escrever um programa simples que descarrega e analisa algum conteúdo usando o [snapshots](/docs/contributor/contributor-pt/conteudo/snapshots/) disponibilizado pelos servidores de conteúdo.


> [!NOTE]
> Pode encontrar o [script completo](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) no GitHub, juntamente com um [exemplo mais avançado](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


Vamos usar o servidor da Fundação Decentraland em `peer.decentraland.org`, e Python 3 como a nossa linguagem de preferência.

Isto é o que vamos fazer:

1. Consultar o estado do servidor de conteúdo.
2. Selecionar e descarregar um [snapshot](/docs/contributor/contributor-pt/conteudo/snapshots/) com uma lista de entities.
3. Imprimir o tipo e o ID de todas as entities referenciadas.

Vamos começar o nosso script com alguns preparativos. Vamos usar apenas módulos da biblioteca padrão, mas, em código real, provavelmente vai querer um cliente HTTP mais confortável (como a [requests](https://github.com/psf/requests) biblioteca).

```py
# Fazer uma requisição HTTP GET e devolver uma resposta HTTP semelhante a um ficheiro.
def fetch(path):
    url = f"https://peer.decentraland.org/{path}"
    headers = { "User-Agent": "urllib" } # importante em alguns servidores (se vazio, 403 Forbidden)

    request = urllib.request.Request(url, headers=headers)
    response = urllib.request.urlopen(request)

    return response
```

A nossa função auxiliar simples faz uma requisição HTTP `GET` e devolve o objeto de resposta semelhante a um ficheiro. Nada de especial. Vamos usá-la para aceder ao `/about` endpoint e verificar o estado do servidor:

```py
# Verificar o estado do servidor:
about = json.load(fetch('about'))

if not about['healthy']:
    print("O servidor não está saudável!")
    sys.exit(1)
```

Se passarmos deste ponto, o servidor está em funcionamento (obtivemos uma `200` response) e indica que está operacional. Podemos solicitar o conjunto atual de snapshots (que vem em formato de array JSON):

```py
# Obter a lista de snapshots:
all_snapshots = json.load(fetch('content/snapshots'))
```

Os ficheiros de snapshot (especialmente para os intervalos de tempo mais longos) podem ser muito grandes. Para uma experiência rápida, vamos apanhar o menor da lista por `numberOfEntities`:

```py
# Escolher o snapshot mais pequeno, em termos de entities incluídas:
snapshot = min(all_snapshots, key=lambda s: s['numberOfEntities'])
```

Para descarregar o conteúdo, precisamos do `hash` campo de `snapshot`. Obtemos o URL do ficheiro ao anexá-lo à raiz do conteúdo:

```py
# Pedir o ficheiro à API de conteúdo:
response = fetch('content/contents/' + snapshot['hash'])
```

O ficheiro que selecionámos é suficientemente pequeno para guardar em memória, mas vamos fingir que não sabemos isso e fazer streaming dele. A primeira linha é o cabeçalho do snapshot, e cada linha seguinte contém um objeto JSON.

Vamos verificar o cabeçalho, que é sempre uma boa ideia:

```py
# Verificar o cabeçalho do snapshot:
header = response.readline().decode('utf-8').strip()

if header != '### Decentraland json snapshot':
    print("Cabeçalho de snapshot inválido: " + header)
    sys.exit(1)
```

Agora podemos processar todas as entities no snapshot, lendo a resposta linha por linha. Para os nossos modestos propósitos, *processar* significa imprimir o tipo e o ID da entity:

```py
# Ler e descodificar todos os itens, um JSON por linha:
for line in response:
    item = json.loads(line)
    print(item['entityType'], item['entityId'])
```

Este ciclo começará a fazer streaming, a analisar e a imprimir linhas como estas até terminar com o snapshot:

```
profile bafkreic36qmzyprs6whkpuxbeiif4no6kvdrr2tfpichbx2fzfz5py6eyv
scene bafkreibr5xfujqrp5q3o4s73vm2yljlcp7cucqgugssnarsuclxv4emlmy
profile bafkreid7khr5wnkialba44rsslffi633rh3lvctad5oa5vjoe6wa7s4c5a
wearable bafkreihlqcb7jgubomyidikpwpqhgzbagltk5m4rgbjdvzydxmoka7bg4i
```

Saúde! Usámos o sistema de snapshot para explorar algum do conteúdo disponível em Decentraland.

Lembre-se de que pode encontrar o [script completo](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots_mini.py) no GitHub, juntamente com um [exemplo mais avançado](https://github.com/decentraland/documentation/blob/main/content/contributor/content/practice/snapshots.py).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/pratica/python-examples.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
