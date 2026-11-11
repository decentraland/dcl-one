# Snapshots

Os content servers irão compilar periodicamente resumos das active entities que alojam, chamados *snapshots*. São regulares [files](/docs/contributor/contributor-pt/conteudo/file-system/) e podem ser descarregados usando o seu identificador.

Os snapshots são criados diariamente, semanalmente, mensalmente e anualmente. Cada um contém o conjunto de active entities que mudou desde o snapshot anterior para esse intervalo.

Os snapshots conterão versões conflitantes das mesmas entities (ou seja, diferentes [manifest files](/docs/contributor/contributor-pt/conteudo/entities/#properties) associados ao mesmo pointer) à medida que são atualizados. Ao analisá-los, os clients devem manter a versão no snapshot mais recente. Como os content servers podem eliminar inactive files, não é garantido que versões desatualizadas de entities estejam disponíveis para download.

Quando um novo snapshot *substitui* os mais antigos (por exemplo, um snapshot semanal que combina uma série de snapshots diários), os seus metadados indicam quais os ficheiros anteriores que são substituídos, para que os clients não precisem de os descarregar.

O conjunto completo de active entities pode ser descoberto combinando todos os snapshots disponíveis (mais sobre isto abaixo), mantendo a entity mais recente referenciada por cada [pointer](/docs/contributor/contributor-pt/conteudo/pointers/) descoberto ao longo do caminho.

Pode experimentar com snapshots usando código funcional na [secção de prática](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) .

### Descoberta de Snapshots <a href="#discover" id="discover"></a>

Para localizar o conjunto atual de snapshots, use o [`snapshots` endpoint](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots). A resposta contém um array de items com estes campos:

| Campo                     | Valor                                                                           |
| ------------------------- | ------------------------------------------------------------------------------- |
| `generationTimestamp`     | O timestamp UTC Unix quando este snapshot foi criado.                           |
| `hash`                    | O snapshot [file](/docs/contributor/contributor-pt/conteudo/file-system/).         |
| `numberOfEntities`        | O número de entradas no ficheiro do snapshot.                                   |
| `replacedSnapshotHashes`  | Um array com os `hash` de quaisquer snapshots substituídos por este.            |
| `timeRange.initTimestamp` | O timestamp UTC Unix (em milissegundos) para o início do intervalo do snapshot. |
| `timerange.endTimestamp`  | O timestamp UTC Unix (em milissegundos) para o fim do intervalo do snapshot.    |

Por exemplo:

```json
{
  "generationTimestamp": 1684979298844,
  "hash": "bafybeiflmm46nr4vv2h3wuzbx3pukcz7ju4fhbfzt6yxmoo533uktlgru4",
  "numberOfEntities": 12345,
  "replacedSnapshotHashes": [ "bafybeicw6x75ieaxfwynekbyhpcsgctpjkt6cb4j6oa7s57qjj6e4b5phd" ],
  "timeRange": {
     "initTimestamp": 1684281600000,
     "endTimestamp": 1684886400000
  }
}
```

### Descarregar Snapshots <a href="#download" id="download"></a>

Usando o `hash` campo de um snapshot, os clients podem descarregar os active entities associados criados ou atualizados nesse intervalo.

Os ficheiros do snapshot começam com esta linha exata:

```
### snapshot JSON da Decentraland
```

Depois disso, cada linha é um documento JSON que descreve um [entity](/docs/contributor/contributor-pt/conteudo/entities/) com os seguintes campos:

| Campo             | Valor                                                                                                                  |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `entityId`        | O identificador imutável para este [entity](/docs/contributor/contributor-pt/conteudo/entities/).                         |
| `entityType`      | Um de `scene`, `perfil`, `wearable`, `emote`, `store` ou `outfits`.                                                    |
| `pointers`        | Um array de [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) que resolvem (ou resolviam) para esta entity. |
| `entityTimestamp` | O timestamp UTC Unix (em milissegundos) quando esta entity foi carregada.                                              |
| `authChain`       | O [cadeia de autenticação](/docs/contributor/contributor-pt/conteudo/entities/#ownership) para esta entity.               |

Uma entrada típica parece assim:

```json
{
  "entityId": "bafkreigrvaqynmiglpvewwhn2yd63q5dvagrrt5jbhimzvbrn5kimj5zne",
  "entityType": "wearable",
  "pointers": ["urn:decentraland:matic:collections-v2:0x11a6879861f36cbad632a4e7226816a16139fb33:0"],
  "entityTimestamp": 1671117456129,
  "authChain": [
    // ... authentication chain payloads and signatures
  ]
}
```


> [!NOTE]
> Se pretender analisar uma linha de snapshot linha a linha, lembre-se de saltar (ou, melhor ainda, validar) a primeira com o cabeçalho, e esteja preparado para lidar com uma linha vazia no fim do ficheiro.


#### Iniciar um índice de Entities <a href="#index-start" id="index-start"></a>

Os clients que querem indexar o conjunto completo de active entities devem processar todos os snapshots atualmente disponíveis e manter o mais recente [entity](/docs/contributor/contributor-pt/conteudo/entities/) para cada [pointer](/docs/contributor/contributor-pt/conteudo/pointers/).

A estratégia mais simples é processar os snapshots em ordem cronológica inversa (isto é, do mais recente para o mais antigo), ignorando pointers que já tenham sido descobertos, para manter a referência para a entity mais recente.

Em pseudo-código:

```py
# Descarregue o conjunto atual de snapshots e ordene-os do mais recente para o mais antigo:
snapshots = get_snapshots()
snapshots.sort('timeRange.initTimestamp', DESCENDING)

seen_pointers = set()

# Processe os snapshots, mantendo a entity mais recente para cada pointer:
for snapshot in snapshots:
    items = get_snapshot_items(snapshot) 

    for item in items:
        if any(pointer in seen_pointers for pointer in item.pointers):
            discard(item)
        else:
            keep(item)
            seen_pointers.update(item.pointers)
```

Como entities individuais podem ser referenciadas por múltiplos pointers (como é comum no caso de [que as cenas](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/)), todos eles devem ser verificados antes de optar por manter ou descartar o item.


> [!NOTE]
> Os ficheiros de snapshot para os intervalos de tempo mais longos podem ser muito grandes. Para fins de desenvolvimento e experimentação que não exijam indexar todo o conjunto de entities, recomenda-se usar os snapshots mais pequenos. O conjunto resultante de entities será incompleto, mas válido.


#### Atualizar um índice de Entities <a href="#index-update" id="index-update"></a>

Os clients que mantêm um índice de entities atualizado podem fazer chamadas periódicas ao [`snapshots`](https://decentraland.github.io/catalyst-api-specs/#tag/Content-Server/operation/getSnapshots) endpoint, e determinar se devem descarregar cada ficheiro tendo em conta:

* O snapshot identificado por `hash` já foi descarregado?
* Está `hash` na `replacedSnapshotHashes` lista de outro snapshot que já foi descarregado?
* É o `timeRange` relevante para os fins atuais?

Se tiverem de ser processados novos snapshots, pode usar-se a mesma estratégia acima para atualizar um conjunto de dados existente.

### Exemplos

Na [secção de prática](https://github.com/decentraland/docs/tree/main/contributor/content/practice/README.md) secção, encontrará exemplos de código que funcionam com o sistema de snapshots.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/snapshots.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
