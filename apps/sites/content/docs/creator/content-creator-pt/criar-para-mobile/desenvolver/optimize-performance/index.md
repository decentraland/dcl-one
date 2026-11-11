> Limites de cenas para mobile e metas de desempenho para cenas do Decentraland, e como as pré-visualizar e medir a partir do Creator Hub.

# Desempenho

Os dispositivos móveis têm restrições de recursos mais apertadas do que desktop ou web. Entender os limites da scene mobile e como visualizá-los ajuda você a publicar uma scene que funcione sem problemas para o maior público possível.

## Limites da Scene Mobile

A tabela abaixo mostra os limites aplicados ao executar em mobile. Alcançar um **limite suave** aciona um aviso no painel de performance; alcançar um **limite rígido** impede o carregamento da scene.

| Métrica                            | Limite Suave | Limite Rígido |
| ---------------------------------- | ------------ | ------------- |
| Triângulos                         | 1,000,000    | 1,200,000     |
| Entities                           | 4,800        | 6,000         |
| Meshes (corpos)                    | 2,400        | 3,000         |
| Geometrias                         | 1,000        | 2,000         |
| Materiais                          | 400          | 500           |
| Texturas                           | 400          | 500           |
| Colliders                          | 1,200        | 1,500         |
| Emissores de VFX                   | 8            | 10            |
| Partículas em tempo real           | 25,000       | 50,000        |
| Fontes de luz                      | 8            | 16            |
| Luzes de sombra                    | 2            | 4             |
| Tamanho do conteúdo                | 120 MB       | 150 MB        |
| Conteúdo externo                   | 40 MB        | 50 MB         |
| Memória (RSS do processo)          | 1.638 MB     | 2.048 MB      |
| Chamadas de desenho                | 1,000        | 2,000         |
| Performance (quanto maior, melhor) | 90%          | 85%           |

## Luzes e partículas

As linhas de luz contam **criado** conteúdo da scene, não o que o engine acaba executando. Cada perfil gráfico ativa apenas um orçamento das fontes de luz mais próximas (0 em Very Low até 8 em High) e pode desativar as sombras completamente — então uma scene que cria 20 luzes ainda paga o custo de scene delas, embora no máximo 8 emitam luz ao mesmo tempo. Crie até o limite, não dependa do orçamento em tempo de execução.

As partículas também têm limite no runtime: 5.000 partículas por emissor e 50.000 por scene. Tudo o que for criado acima desses limites é limitado automaticamente, então excedê-los só desperdiça orçamento da scene.

## Preview de Performance Mobile

Você pode verificar a performance mobile da sua scene diretamente no **Creator Hub** sem fazer deploy em um dispositivo físico.

1. Abra sua scene no **Creator Hub**.
2. Selecione Preview > Mostrar QR Code for Mobile
3. Abra o **Scene Limits Preview** no ícone no canto superior direito do seu telefone (monitor com estatísticas em vermelho).

![Painel de limites da scene mobile no Creator Hub](/docs/_remote/86e4b4fb53b120e0cdcd5a75.webp)


> [!WARNING]
> **Mantenha a Performance acima de 90% em High Graphics**
> 
> O **Performance** valor é uma porcentagem do orçamento de FPS definido pelo perfil gráfico ativo. Um valor de **100%** significa que a scene está atingindo a meta total de FPS.
> 
> Sempre teste em um dispositivo intermediário como o **Samsung Galaxy A54** e mire em um **score de Performance acima de 90% no perfil gráfico High**. Isso garante uma experiência suave para a maioria dos jogadores mobile. Veja [Hardware Requirements](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/hardware-requirements/) para dispositivos de referência.
> 
> Você pode alterar o perfil gráfico em **In-Game Menu → Settings → Graphics → defina Dynamic Graphics como Off → troque Profiles**.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/optimize-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
