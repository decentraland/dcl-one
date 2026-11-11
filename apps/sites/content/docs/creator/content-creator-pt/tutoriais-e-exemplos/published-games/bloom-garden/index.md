> Um jardim cooperativo com estado persistente das plantas, estimativa de deslocamento do relógio do servidor e animações de pétalas em pool.

# Bloom Garden

![Pré-visualização de Bloom Garden](/docs/_remote/3a9e2127ba7a6f3cdcd435b3.webp)

Uma experiência de jardim cooperativo em que os jogadores trabalham juntos para manter um jardim vivo e despertar o Big Bloom.

O seu Multiplayer Server usa Storage integrado para o estado das plantas, uma classificação semanal e limites de rega por jogador.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=bloomgarden.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · TheLivingGarden

[Ver fonte upstream](https://github.com/TheLivingGarden/TheLivingGarden/tree/main-main)

**Licença:** [MIT](https://github.com/TheLivingGarden/TheLivingGarden/blob/main/LICENSE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Estimar o desfasamento entre os relógios do Server e do Client

O auxiliar de relógio mantém cinco amostras de desfasamento, rejeita desvios grandes e converte timestamps do Server para a hora local. O Client de rega alimenta-o com timestamps recebidos através das definições de mensagem da Scene.

**Contexto do projeto:** Isto ilustra uma estimativa de temporização leve; o tempo de trânsito da rede está incluído nas amostras.

**Referências de origem:**

* [O auxiliar de relógio](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/shared/clockSync.ts#L3-L55)
* [O client de rega](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/wateringSystem.ts#L1144-L1146)

### Deixe os efeitos temporários terminarem naturalmente

A configuração e os controlos das pétalas criam um pool reutilizável a partir do modelo Petal nomeado da Scene e, em seguida, iniciam ou estabilizam o efeito. As fases de atualização fazem as pétalas crescerem, deixam-nas cair e repousar, e encolhem-nas antes de as reciclar. Estude isto quando um efeito deve parar de gerar objetos enquanto os objetos existentes concluem a sua animação.

**Contexto do projeto:** A colocação depende de BLOOM\_CENTER e das constantes de temporização do módulo.

**Referências de origem:**

* [A configuração e os controlos das pétalas](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L119-L173)
* [As fases de atualização](https://github.com/TheLivingGarden/TheLivingGarden/blob/b2c9a90791b1eed5c92a4ffadf6ac480d91510ea/src/petalSystem.ts#L207-L310)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/bloom-garden.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
