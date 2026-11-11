> Funcionalidades disponíveis no cliente de desktop do Decentraland que ainda não são suportadas na aplicação móvel, e inconsistências conhecidas entre plataformas.

# Funcionalidades em falta


> [!NOTE]
> Esta página acompanha a lacuna de funcionalidades entre o cliente de desktop (Unity) da Decentraland e a app móvel. É baseada no [rastreador de paridade de funcionalidades do godot-explorer](https://github.com/decentraland/godot-explorer/issues/2402) e é atualizada regularmente. As ETAs são estimativas e podem mudar.
> 
> Última revisão: **Outubro de 2026** (app móvel v1.15.0).


## Lançado recentemente

Estas já não são lacunas a partir da app móvel **v1.15.0**, que está a ser disponibilizada nas lojas de apps em outubro de 2026:

* [`stopEmote` ação restrita](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/#animate-only-the-upper-body). As emotes da parte superior do corpo iniciadas por uma scene também pausam quando o player abandona a scene, retomam ao regressar e param quando a scene é descarregada — v1.15.0
* [`Tween` `MoveRotateScale` mode](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/#simultaneous-tweens) — v1.15.0
* As posições dos outros players são comunicadas às scenes em relação à scene, tal como a do player local (`getPlayer({ userId }).position`, `onEnterScene`, items anexados a outros players) — v1.15.0
* Movimento do player, primeira passagem de paridade com Unity: tamanho da capsule, altura do passo, arco do salto e gravidade, velocidade de corrida e controlo no ar — v1.15.0
* Os raycasts para baixo a partir do player reportam o correto `hitDistance` em geometria inclinada — v1.15.0
* As emotes da parte superior do corpo são vistas como da parte superior do corpo por outros players móveis — v1.15.0

## Funcionalidades do SDK em falta no mobile

* [Fontes personalizadas (`fontSrc` em `TextShape`, `Label`, `Button`, `Input` e `Dropdown`)](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#custom-fonts) — a app móvel ignora `fontSrc` e renderiza o fallback `font` — Sem ETA
* [`AvatarNametag` (etiqueta criada na scene acima do nome de um player)](https://github.com/decentraland/godot-explorer/issues/2811) — Sem ETA
* [Sequências de Tween (`TweenSequence`)](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/#tween-sequences) — não implementado; os tweens em sequência não animam — Sem ETA
* [SDK7 UiBackground faz nine-slice dos tiles em vez de esticar](https://github.com/decentraland/godot-explorer/issues/2060) — Sem ETA
* Compressão de Mesh Draco (`KHR_draco_mesh_compression`) — a app móvel não tem um descodificador Draco em runtime. Os models comprimidos com Draco `.gltf`/`.glb` carregam apenas a partir dos assets otimizados da scene, onde o asset server já os descomprimiu. Quando uma scene não tem assets otimizados, como na preview local, não conseguem carregar sem erro. Exporte os seus models sem compressão Draco para garantir. Um diálogo de erro para este caso está a ser acompanhado em [#2432](https://github.com/decentraland/godot-explorer/issues/2432) — Sem ETA
* Smart Items — não oficialmente suportados no mobile

## Funcionalidades do cliente de desktop não presentes no mobile

* [Chat de voz por proximidade](https://github.com/decentraland/godot-explorer/issues/888) — 1.º trimestre de 2027
* [Apontar no World](https://github.com/decentraland/godot-explorer/issues/1736) — Sem ETA
* [Alteração da cor da nameplate](https://github.com/decentraland/godot-explorer/issues/1684) — Sem ETA
* [Comunidades](https://github.com/decentraland/godot-explorer/issues/656) — 1.º trimestre de 2027
* [Galeria de fotos](https://github.com/decentraland/godot-explorer/issues/680) — dezembro de 2026
* [Streams da comunidade](https://github.com/decentraland/godot-explorer/issues/676) — 1.º trimestre de 2027
* [Badges de perfil](https://github.com/decentraland/godot-explorer/issues/678) — dezembro de 2026
* [Missões diárias](https://github.com/decentraland/godot-explorer/issues/682) — Sem ETA
* [Reações no chat](https://github.com/decentraland/godot-explorer/issues/1824) — 1.º trimestre de 2027
* [Tradução automática do chat](https://github.com/decentraland/godot-explorer/issues/2260) — 1.º trimestre de 2027
* [Chat: mensagens diretas](https://github.com/decentraland/godot-explorer/issues/1120) — 1.º trimestre de 2027
* [Suporte a DCL Cast no mobile](https://github.com/decentraland/godot-explorer/issues/1881) — Sem ETA
* [URLs de vídeos do YouTube / Google Drive não suportados](https://github.com/decentraland/godot-explorer/issues/2081) — Restrito no mobile por conformidade com as lojas
* [Slots de outfit na backpack](https://github.com/decentraland/godot-explorer/issues/1625) — dezembro de 2026
* [Oferecer wearables a outros players](https://github.com/decentraland/godot-explorer/issues/2957) — dezembro de 2026
* [Aviso "Session ended" quando a mesma conta inicia sessão noutro dispositivo](https://github.com/decentraland/godot-explorer/issues/2880) — Sem ETA
* [Categorias Discover (navegar por experiências por categoria)](https://github.com/decentraland/godot-explorer/issues/3037) — Sem ETA

## Inconsistências entre plataformas

* **Movimento e colisões do player vs Unity**
  * [Inclinações, bordas, paredes, aterragem e plataformas móveis](https://github.com/decentraland/godot-explorer/issues/2852) e [comportamento de glide](https://github.com/decentraland/godot-explorer/issues/2854) — correção em revisão, Sem ETA
  * [Os colliders da scene com menos de 1 cm de espessura só colidem por cima, e os bloqueadores invisíveis e algumas collision layers do SDK não bloqueiam o player](https://github.com/decentraland/godot-explorer/issues/2853) — Sem ETA
  * Animações de spawn, aterragem e salto, blending de movimento ([#1553](https://github.com/decentraland/godot-explorer/issues/1553), [#1554](https://github.com/decentraland/godot-explorer/issues/1554), [#2855](https://github.com/decentraland/godot-explorer/issues/2855), [#2856](https://github.com/decentraland/godot-explorer/issues/2856)) — correção em revisão, Sem ETA
* [Algumas emotes de outros clientes não reproduzem no mobile: emotes acionadas pela scene de players Bevy Web e emotes de players Unity que chegam via LiveKit](https://github.com/decentraland/godot-explorer/issues/2986) — correção em revisão, Sem ETA
* [Numa preview móvel do Creator Hub, o avatar do player móvel não é visível para testers no desktop](https://github.com/decentraland/godot-explorer/issues/2773) — correção em andamento, Sem ETA
* [Os dentes do avatar são renderizados a escuro/cinzento em vez de branco](https://github.com/decentraland/godot-explorer/issues/1994) — Sem ETA

## Restrições de input/plataforma

* **Input apenas por toque** — sem estados de hover do rato, atalhos de teclado ou clique com o botão direito.
* **Suporte limitado a gestos** — pinch-to-zoom (alternar entre a camera em primeira e terceira pessoa) foi adicionado na v1.14.0; atualmente não há outros gestos planeados.

## Reportar uma funcionalidade em falta

Se encontrar uma limitação que não esteja listada aqui, por favor [reporte-a](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/report-bug/) para que possamos documentá-la e priorizá-la.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/cliente-movel/missing-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
