> Acompanhe visitas de jogadores, engajamento e retenção para suas scenes publicadas

# Analytics da Scene

Como criador, é muito valioso acompanhar as visitas dos jogadores e a forma como os jogadores interagem com a sua cena. Em vez de assumir, pode obter dados reais e tomar decisões informadas enquanto itera nos seus designs.

O [Creator Hub](https://decentraland.org/create) inclui uma secção **Analytics** com métricas detalhadas para cada cena que publica ou sobre a qual tem permissões. Não precisa de adicionar nada à sua cena; estes dados são recolhidos automaticamente para todas as cenas publicadas.


> [!NOTE]
> **📔 Nota**: Os dados de Analytics são atualizados uma vez por dia. Apenas os proprietários e operadores de uma cena podem ver os seus dados.


## Ver as análises da sua cena

1. Abra o Creator Hub e inicie sessão com a sua conta.
2. Selecione o separador **Analytics** na barra de navegação superior. Verá uma lista de todas as cenas publicadas que possui ou opera, com métricas principais para cada uma.
3. Clique numa cena para abrir a respetiva página de detalhes, com o conjunto completo de métricas organizado por separadores.

Os dados são acompanhados por cena. Se um World contiver várias cenas, cada cena aparece como uma entrada separada na lista, com as suas próprias métricas.

## A lista de cenas

A lista mostra uma linha por cena, com os seguintes campos:

* **Visitas únicas**: Utilizadores distintos que visitaram a sua cena, contados uma única vez cada.
* **Novos utilizadores**: Utilizadores que visitaram a sua cena pela primeira vez.
* **Retenção D7**: A percentagem de novos utilizadores que voltaram a visitar a sua cena pelo menos uma vez nos 7 dias após a sua primeira visita.
* **Receita**: MANA ganho na sua cena.
* **Tempo mediano de jogo**: Tempo mediano que cada utilizador passa na sua cena por dia: metade das visitas diárias é mais longa, metade mais curta.

Pode ordenar a lista ou pesquisar por nome, e fixar no topo as cenas que mais lhe interessam.

## A página de detalhes

Clique em qualquer cena da lista para abrir a respetiva página de detalhes. Ao lado das métricas, encontrará um cartão com informações gerais sobre a cena: a sua taxa de gostos, se o acesso é público ou privado, o World em que foi publicada, quem a publicou por último e quando foi atualizada pela última vez. Também pode copiar o URL da cena, entrar na cena ou criar um evento para ela diretamente a partir desta página.

Use o seletor **Intervalo de datas** para calcular as métricas dos últimos 7, 30 ou 60 dias, e o botão **Exportar Analytics** para descarregar os dados como um .csv.

Os gráficos semana a semana, como as coortes de retenção, os utilizadores ativos semanais e os visitantes socialmente envolvidos, abrangem sempre as últimas 16 semanas completas, independentemente do intervalo de datas que escolher.

Uma métrica que ainda não tem dados mostra um traço (`-`) em vez de um número.

As métricas estão organizadas em quatro separadores.

### Visão geral

Um resumo do crescimento, envolvimento e retenção de utilizadores da sua cena.

* **Visitas totais**: Cada visita à sua cena, incluindo visitas repetidas do mesmo utilizador.
* **Visitas únicas**: Utilizadores distintos que visitaram a sua cena, contados uma única vez cada.
* **Novos utilizadores**: Utilizadores que visitaram a sua cena pela primeira vez.
* **Utilizadores simultâneos**: O maior número de utilizadores na sua cena ao mesmo tempo.
* **Receita**: MANA ganho na sua cena.
* **Retenção ao 7.º dia**: A percentagem de novos utilizadores que voltaram a visitar a sua cena pelo menos uma vez nos 7 dias após a sua primeira visita.
* **Tempo mediano de jogo**: Tempo mediano que cada utilizador passa na sua cena por dia: metade das visitas diárias é mais longa, metade mais curta.
* **Tempo AFK**: Tempo médio que os utilizadores passam na sua cena sem interagir.
* **Desktop / Mobile**: Utilizadores que visitaram a sua cena a partir de um cliente desktop ou mobile.

### Visitas

Quantos utilizadores visitaram a sua cena e quantos eram visitantes novos ou recorrentes.

* **Visitas únicas nos últimos 60 dias**: Visitantes únicos, divididos por plataforma (todas as plataformas, desktop e mobile). Os utilizadores são deduplicados entre plataformas: alguém que joga em desktop e mobile é contado uma vez no total de todas as plataformas e uma vez no total de cada plataforma.
* **Utilizadores ativos semanais**: Utilizadores ativos únicos durante cada semana.
* **Fluxo semanal de utilizadores**: Como os seus utilizadores semanais se dividem entre:
  * **Novos utilizadores**: Utilizadores que visitaram a sua cena pela primeira vez.
  * **Utilizadores que regressaram**: Utilizadores que visitaram na semana passada e regressaram esta semana.
  * **Utilizadores reativados**: Utilizadores que tinham deixado de visitar e regressaram esta semana.

### Envolvimento

Como os utilizadores interagem com a sua cena depois de lá entrarem.

* **Tempo médio diário de jogo**: Tempo médio que cada utilizador passa na sua cena numa única visita, com um gráfico semana a semana e a variação em relação à semana anterior.
* **Tempo médio semanal de jogo**: Tempo médio que cada utilizador passa na sua cena por semana.
* **Interações sociais**: Três sinais sociais, apresentados tanto como totais semanais como como a proporção dos visitantes dessa semana que realizou cada ação:
  * **Mensagens enviadas**: Mensagens de chat enviadas pelos visitantes na sua cena.
  * **Emotes executados**: Emotes executados pelos visitantes na sua cena.
  * **Novas amizades**: Pedidos de amizade aceites entre visitantes na sua cena.

### Retenção

A percentagem de utilizadores que regressam após a sua primeira sessão.

* **Plataformas**: A sua média móvel de retenção de 60 dias, comparada entre desktop e mobile.
* **Retenção ao 7.º dia por semana da coorte**: A percentagem de utilizadores que regressaram pelo menos uma vez nos 7 dias após a sua primeira visita, agrupados pela semana em que entraram pela primeira vez na sua cena. O gráfico desenha uma linha de referência de 20% como valor de referência saudável.
* **Taxa semanal de churn**: A percentagem de utilizadores ativos da semana anterior que não regressaram esta semana.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/outros/scene-analytics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
