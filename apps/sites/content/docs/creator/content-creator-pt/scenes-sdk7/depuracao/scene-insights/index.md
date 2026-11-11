> Pesquise logs persistentes do Multiplayer Server, acompanhe logs em tempo real e inspecione métricas de desempenho da scene.

# Scene Insights

Scene Insights ajuda você a investigar uma [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) scene implantada. Pesquise logs históricos do servidor, acompanhe novas mensagens ao vivo e obtenha CPU, memória e outras métricas no seu terminal ou assistente de IA.

**Logs e métricas são mantidos por 7 dias**, entre reinicializações do servidor e novos deploys da scene. Isso cobre a execução do lado do servidor; use [Debug in Production](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-prod/) para problemas do lado do cliente e de rendering.

## Atualize Seu Projeto

Execute estes comandos a partir do seu projeto de scene para instalar a versão mais recente do `auth-server` SDK e runtime:

```bash
npm install @dcl/sdk@auth-server @dcl/js-runtime@auth-server
```

Se o seu projeto também listar `@dcl/sdk-commands` diretamente em `package.json`, atualize-o também para `@auth-server` . Confirme que o CLI instalado inclui `login`, `logs`, `tail`, `metrics` e `executions`:

```bash
npx sdk-commands scene-insights --help
```

Para depuração assistida por IA, instale ou atualize também os [SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/):

```bash
npx skills add decentraland/sdk-skills --all
```

Você pode pedir ao seu companheiro de IA para fazer a configuração:

> Atualize esta scene para a versão mais recente do auth-server SDK e runtime, e atualize os Decentraland SDK Skills. Ajude-me a entrar no Scene Insights e, depois, investigue as últimas duas horas de erros do servidor e o uso de CPU e memória.

Ler telemetria de uma scene já implantada não requer um redeploy. Mudanças de permissão exigem.

## Quem Pode Ler os Dados?

Entre com uma wallet que atenda a uma destas condições:

* Ela aparece no nível raiz da scene implantada no `logsPermissions` array.
* Ela é proprietária do World.
* Ela é uma colaboradora de implantação do World cuja permissão cobre todo o World ou cada parcela implantada da scene.
* Para Genesis City, ela é proprietária da **parcela base** da scene, ou tem permissão delegada de gerenciamento de LAND nessa parcela: `operator`, `updateOperator`, `updateManager`, ou `approvedForAll`.

Proprietários e gestores elegíveis não precisam de uma `logsPermissions` entrada ou um redeploy da scene. Para conceder acesso explicitamente a outra wallet, adicione-a em `scene.json` e faça o redeploy:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

Para Genesis City, a permissão é verificada em relação à parcela base da scene implantada, mesmo que você consulte outra parcela dentro da scene. Propriedade ou direitos de gerenciamento em uma parcela diferente, por si só, não concedem acesso à scene.

As mesmas permissões cobrem logs históricos, logs ao vivo, métricas e histórico de execução. O acesso segue **as permissões atuais**: qualquer pessoa atualmente autorizada pode ler todo o histórico retido sob a mesma chave da scene por até 7 dias, incluindo logs escritos enquanto outra pessoa implantava a scene. Isso inclui novos proprietários, operators, tenants elegíveis e wallets recém-adicionadas a `logsPermissions`. Considere esse histórico antes de conceder acesso.

Mudanças de permissão não são imediatas. Por padrão, os caches de permissão duram 60 segundos e streams ao vivo abertos revalidam o acesso a cada 5 minutos. Aguarde cerca de 6 minutos para que um stream aberto observe a revogação. Esses são tempos padrão, não um prazo rígido: configuração e períodos de graça limitados durante falhas temporárias de capacidade podem estender o atraso. Fazer login não concede permissão para ler outras scenes.

## Entrar

```bash
npx sdk-commands scene-insights login
```

Aprove o login no seu navegador com uma wallet normal ou uma smart contract wallet. Isso cria uma sessão do Insights limitada a ler logs, métricas e executions. Ela dura até **12 horas**, limitada pela expiração da sessão pai. Comandos subsequentes a reutilizam sem outra aprovação.

Verifique o endereço conectado e a expiração com:

```bash
npx sdk-commands scene-insights session --json
```

Quando a sessão expirar, execute `login` novamente. Se um assistente de IA estiver investigando para você, aprove o login da wallet você mesmo. Não compartilhe chaves privadas nem o arquivo de sessão em cache com ele.

### Mantenha o Arquivo de Sessão Privado


> [!WARNING]
> **Nunca faça commit, deploy ou compartilhe o seu arquivo de sessão de login.** Por padrão, ele é salvo em `~/.dcl/scene-insights-session.json`. Ele contém uma chave privada temporária de assinatura e a delegação de leitura da sua wallet. Qualquer pessoa com uma cópia pode usar essas credenciais para ler logs, métricas e o histórico de execução das scenes às quais sua wallet pode acessar até a sessão expirar. Não o envie para colegas de equipe, assistentes de IA, chats de suporte ou relatórios públicos de issues, nem o inclua em arquivos compactados do projeto.


O SDK o protege automaticamente:

* Ele salva o arquivo no diretório home por padrão, fora do projeto da scene. Se você definir `DCL_SCENE_INSIGHTS_SESSION_DIR`, ele usa o `.dcl` desse diretório em vez disso.
* Ele adiciona `*` ao `.gitignore` da pasta de sessão e recusa salvar ou usar um arquivo de sessão que o Git já rastreia.
* Ele grava o arquivo de sessão com permissões somente do proprietário (`0600`) e cria sua pasta com `0700` permissões em sistemas que as suportam.
* O coletor padrão de arquivos de deploy do SDK exclui arquivos e diretórios prefixados com ponto, incluindo `.dcl`, independentemente de `.dclignore`.

Tenha estas verificações em mente:

* Revise os arquivos staged antes de fazer commit. Não force a adição do arquivo de sessão; `.gitignore` não protege arquivos já rastreados pelo Git.
* Se você personalizar o local da sessão, mantenha-o fora do seu projeto de scene sempre que possível. Nunca copie ou renomeie a sessão para assets da scene ou outra pasta que possa ser implantada.
* Se você usar scripts personalizados de deploy, backup ou ZIP, exclua explicitamente o arquivo de sessão e `.dcl` credenciais; essas ferramentas podem não aplicar as exclusões do SDK.
* Compartilhe apenas a saída de status segura de `npx sdk-commands scene-insights session --json` quando necessário, nunca o próprio arquivo de sessão. Os colegas de equipe devem entrar com suas próprias wallets autorizadas.

`npx sdk-commands scene-insights logout` exclui a sessão salva localmente. Ele **não** revoga cópias já compartilhadas: essas credenciais podem continuar utilizáveis até expirar. Se o arquivo foi exposto, remova-o dos artefatos compartilhados e do histórico do Git, conforme aplicável; excluir o arquivo local ou entrar novamente não invalida a cópia exposta.

## Escolha uma Scene

A partir do projeto da scene, os comandos inferem o World a partir de `scene.json`do `worldConfiguration.name`, ou use `scene.base` para Genesis City. Você também pode escolher explicitamente:

```bash
# Um World
npx sdk-commands scene-insights logs --world myworld.dcl.eth

# Uma scene em um World com várias scenes
npx sdk-commands scene-insights logs --world myworld.dcl.eth --position 10,10

# Uma scene da Genesis City: use = quando as coordenadas forem negativas
npx sdk-commands scene-insights logs --position=-125,-96
```

Os comandos usam produção por padrão (`.org`). Para uma scene na rede de desenvolvimento, adicione `--target https://multiplayer-scenes-insights-service.decentraland.zone` a login, verificação de sessão e consultas. Usar o mesmo target em tudo também garante que as smart contract wallets sejam validadas na rede correta.

## Pesquisar Logs e Execuções

```bash
# Últimos 200 erros nas últimas 6 horas
npx sdk-commands scene-insights logs --from 6h --level error --limit 200

# Encontrar texto em uma janela de tempo fixa em UTC
npx sdk-commands scene-insights logs --from 2026-11-01T14:00:00Z --to 2026-11-01T14:30:00Z --query "player joined"

# Listar execuções do servidor no último dia
npx sdk-commands scene-insights executions --from 1d

# Logs de uma execução: substitua EXECUTION_ID por um ID de executions
npx sdk-commands scene-insights logs --execution EXECUTION_ID --from 1d
```

`logs` usa por padrão a última hora e as 100 linhas correspondentes mais recentes, exibidas cronologicamente. O CLI aceita `--limit` entre 1 e 10.000 e busca várias páginas da API conforme necessário. `--level` é uma severidade mínima (`warn` inclui erros); `--query` pesquisa texto literal com correspondência ASCII sem diferenciar maiúsculas de minúsculas. Use `--source scene,engine,stderr` para selecionar as origens dos logs, ou passe um subconjunto dessas fontes.

`executionId` identifica uma execução do servidor; `entityId` identifica a versão implantada da scene. Uma nova execution pode significar uma parada/início ociosa, um rollout ou uma falha. Verifique os logs ao redor antes de concluir que houve uma falha.

Adicione `--json` para saída legível por máquina. O CLI retorna logs históricos como um único documento JSON; o campo `more: true` significa que existem correspondências adicionais além do limite retornado. Restrinja a janela ou aumente o limite. Clientes diretos da API recebem até 1.000 linhas por página e usam um cursor para paginação.

## Acompanhar Logs ao Vivo

```bash
npx sdk-commands scene-insights tail
```

Pressione **Ctrl+C** para parar. Para uma captura com duração limitada, inclusive ao pedir a um assistente de IA para investigar:

```bash
npx sdk-commands scene-insights tail --for 30s --lines 20 --json
```

Isso reproduz as 20 linhas mais recentes e, depois, acompanha as novas por 30 segundos. A saída JSON contém uma entrada de log por linha. O CLI reconecta e retoma durante um tail em execução. Os logs normalmente chegam alguns segundos depois que a scene os escreve.

`tail` suporta filtros de nível e texto. Para janelas de tempo históricas ou filtros de execução/origem, use `logs`.

Se você usar `npm run server-logs`, verifique se o seu projeto contém: `package.json` "scripts": {

```json
{
  "server-logs": "sdk-commands scene-insights tail"
    Compilações completas do projeto com o SDK atualizado criam ou migram o script padrão gerado pelo SDK. Scripts personalizados são preservados; previews com
  }
}
```

\--skip-build `e compilações de arquivo único não o atualizam. Você pode editar o script diretamente sem recompilar só para inspecionar logs.` Inspecionar Métricas de Desempenho

## Primeiro liste as métricas que têm dados, depois consulte uma série:

npx sdk-commands scene-insights metrics --json

```bash
npx sdk-commands scene-insights metrics cpu.percent --from 2h --step 60 --json
npx sdk-commands scene-insights metrics mem.heap_used --from 2h --step 60 --json
--step
```

`define o tamanho do bucket em segundos. As métricas disponíveis dependem da versão do engine e da atividade da scene. A descoberta de métricas e` usam por padrão as últimas 24 horas. Use `executions` --execution EXECUTION\_ID `em uma série de métricas para isolar uma execução.` Métrica

| Métrica                                                 | O que ela informa                                                                                                                  |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `cpu.percent`                                           | CPU da thread de execução como porcentagem de um núcleo; valores sustentados próximos de 100% sugerem que essa thread está ocupada |
| `cpu.run_ms`                                            | Tempo de CPU gasto em `onStart`/`onUpdate`, não latência de relógio de parede                                                      |
| `mem.heap_used`, `mem.heap_limit`, `mem.external_bytes` | uso do heap JavaScript, limite do heap e memória externa do V8                                                                     |
| `logs.lines`, `logs.bytes`                              | Saída do console antes dos limites de taxa; útil para identificar logging excessivo                                                |
| `fetch.failed`, `storage.failed`                        | Erros de envio de requests para chamadas HTTP ou de storage de saída; correlacione com logs de erro                                |
| `fetch.status_4xx`, `fetch.status_5xx`                  | Respostas de erro HTTP; estas não são contadas como `fetch.failed`                                                                 |
| `logs.dropped`                                          | Linhas descartadas pelo orçamento de logs por tick do engine                                                                       |
| `comms.msgs_out`, `comms.bytes_out`                     | Mensagens e bytes enviados aos jogadores                                                                                           |

As métricas de CPU não medem o servidor inteiro, e as métricas de memória não representam a memória total do processo nem o limite do contêiner. Compare a mesma execution, janela de tempo e carga de trabalho antes de diagnosticar um vazamento ou problema de desempenho.

As gauges retornam `avg` e `max` por bucket. As sums retornam `sum` e uma `rate`por segundo. Um ponto `null` significa que não há amostra armazenada, não uso zero; scenes ociosas, atrasos e falhas de coleta podem causar lacunas.

## Retenção e Limites

| Limite                                 | Padrão                                                                                                                                                                                             |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Retenção de logs e métricas            | 7 dias                                                                                                                                                                                             |
| Faixa máxima de tempo da consulta      | 7 dias                                                                                                                                                                                             |
| Resultados de logs por consulta no CLI | 100 por padrão; até 10.000 via paginação                                                                                                                                                           |
| Série de métricas                      | Até 1.500 buckets; aumente `define o tamanho do bucket em segundos. As métricas disponíveis dependem da versão do engine e da atividade da scene. A descoberta de métricas e` para janelas maiores |
| Logging contínuo da scene              | 50 linhas/segundo e 64 KiB/segundo                                                                                                                                                                 |
| Alocação para picos de logging         | 500 linhas e 512 KiB                                                                                                                                                                               |
| Cota diária de logs da scene           | Limite flexível de 500.000 linhas ou 128 MiB por processo de servidor; redefine às 00:00 UTC                                                                                                       |

Os contadores de cota diária são mantidos em memória. A reinicialização do processo do servidor ou uma scene sendo movida para outra réplica também redefine seus contadores, então a cota diária é um limite aproximado ao longo dessas mudanças.

Limites de taxa e cotas diárias descartam logs excedentes; linhas descartadas não podem ser recuperadas depois. Mensagens de aviso do engine relatam descartes e redefinições de cota. Atingir a cota diária interrompe a persistência adicional de logs até a redefinição; a scene continua em execução. Reduza o logging repetido por frame para que mensagens úteis caibam no orçamento.

O engine também tem um orçamento separado de logs por tick (1.000 linhas por padrão); as linhas excedentes são descartadas antes da exportação e contabilizadas em `logs.dropped`.

Mensagens grandes podem ser truncadas (a API usa por padrão 8.192 caracteres por mensagem). A coleta é feita no melhor esforço: interrupções, falhas de exportação e limites de taxa podem deixar lacunas. Apenas a telemetria coletada está disponível; instalar o novo CLI não pode recuperar dados antigos ou descartados. Resultados vazios, por si só, não provam que uma scene esteja saudável.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
