> Combate cooperativo de naves espaciais com estado do jogo pertencente ao servidor, progressão persistente, câmeras de torre e marcadores de objetivo na borda da tela.

# Astro Crew

![prévia de Astro Crew](/docs/_remote/31ce14ffd2ff2d42c05fb6b7.webp)

Uma cena cooperativa em que a tripulação compartilha uma única nave. Em cada parada, eles atiram em asteroides e discos voadores que se aproximam, reparam rupturas no casco e aumentam os níveis de atirador e engenharia. O combate roda em um server multijogador autoritativo.

* [Jogue no Decentraland](https://decentraland.org/play/?realm=astrocrew.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · DansStuff

[Ver fonte upstream](https://github.com/DansStuff/AstroCrew)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* **Multijogador autoritativo** para todo o combate. Os clients enviam requisições para mirar, reparar e sobrecarregar. O server é o dono do casco, das rupturas e dos spawns, e sincronizados `GameState` e `PlayerStats` aceitam apenas escritas do server multijogador.
* **Armazenamento** para os níveis de atirador e engenharia de cada jogador, e um placar semanal de missões.
* **Câmeras virtuais** nas três torres, mescladas com uma transição temporizada. Os impactos sacodem essas câmeras. Pegar uma arma congela o avatar com **InputModifier** e oculta o joystick móvel e a mira com **TouchScreenControls**.
* **PrimaryPointerInfo** aponta cada disparo a partir do raio mundial do ponteiro. **PointerLock** permanece liberado para que **PointerEvents** ainda possam controlar os consoles, as rupturas para reparar mantendo pressionado e a mesa de missões.
* **GltfNodeModifiers** recolorir as meshes emissivas da nave: verde durante a navegação, vermelho durante um encontro e um segundo conjunto enquanto as armas estão sobrecarregadas.
* **ReactEcs** HUD para o casco, os níveis de habilidade e o placar do fim da rodada. Os ícones de ruptura fora da tela ficam na borda da tela, posicionados com **UiCanvasInformation**.
* **Billboards**, **TextShape**e **texturas do avatar** em cada alvo: uma mira voltada para a câmera, um rótulo "Alvo travado" e retratos de quem estiver atirando nele.
* **Animator** reproduz em loop o clipe de giro do disco voador. **AudioSource** toca stingers e loops, com um conjunto de one-shots de laser para que um disparo possa reiniciar enquanto o último clipe ainda está a tocar. Os clipes são pré-carregados com **AssetLoad**.
* No mundo **TextShape** linhas para o placar semanal. Clicar numa linha abre os resultados dessa missão.
* **AvatarModifierArea** desativa o prompt do passaporte em toda a cena.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Direcionar os jogadores para objetivos fora da tela

edgePositionForBreach projeta uma direção de ruptura nos eixos horizontais da câmera, encontra uma borda de um retângulo e limita o ícone dentro da tela usando a proporção do canvas. BreachIcon conecta essa posição a um marcador pulsante do ReactEcs.

**Contexto do projeto:** Lê-o com as posições de ruptura da cena, o estado de ruptura ativa e as constantes de UI.

**Referências de origem:**

* [edgePositionForBreach](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L27-L76)
* [BreachIcon](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/ui/breachIcons.tsx#L78-L107)

### Adicionar tremor de câmera que retorna à pose original

shakeEntity registra um transform base e combina tremores sobrepostos usando a intensidade e a duração restante. CameraShakeSystem aplica offsets decrescentes de posição e rotação, depois restaura a pose base e remove o componente.

**Contexto do projeto:** O efeito usa as constantes de amplitude do projeto e deve ser registrado como um sistema ECS.

**Referências de origem:**

* [shakeEntity](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L13-L47)
* [CameraShakeSystem](https://github.com/DansStuff/AstroCrew/blob/dbe11b05be2a753b27b6d5dd866d0554ff73344f/src/effects/cameraShake.ts#L87-L121)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/astro-crew.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
