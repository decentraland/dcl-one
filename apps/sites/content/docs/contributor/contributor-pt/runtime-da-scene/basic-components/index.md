# Basic Components

O World Explorer suporta uma lista de components básicos que compartilham sua definição com scenes, oferecendo suporte integrado para posicionamento, animação, media, consultas de estado do world

Há cinco tipos de components:

1. [Components de objeto](#object) adicionam propriedades visuais e físicas a Entities.
2. [Components de jogo](#game) podem solicitar actions e receber informações do game engine.
3. [Components de mídia](#media) podem exibir imagens e tocar sounds.
4. [Components de área](#area-components) alteram o comportamento de Entities em zonas específicas.
5. [Components de UI](#ui) permitem que scenes renderizem interfaces flutuantes.

Exceto por [`Transform`](#transform), que é especificado em detalhe abaixo, o estado de todos os components é serializado usando protocol buffers. Você pode seguir o link em cada título para ver a definição completa.

### Components de objeto <a href="#object" id="object"></a>

Os components mais comuns são aqueles que anexem propriedades visuais e físicas a uma Entity. Eles podem ser usados para posicionar, redimensionar, pintar e adicionar colisão a objects in-game.

***

#### `Transform` [↗ spec](https://adr.decentraland.org/adr/ADR-153) <a href="#transform" id="transform"></a>

Adiciona posição, rotação e escala a uma entity.

Este component é, de longe, o mais usado e frequentemente atualizado ao longo da vida útil de uma scene. Por isso, `Transform` não é serializado usando protocol buffers e, em vez disso, é compactado em uma estrutura personalizada.

`Transform` tem tamanho de `44` bytes, com este layout exato:

```
.-----------------------.-------------------------------.-----------------------.-----------------.
|   x   |   y   |   z   |   x   |   y   |   z   |   w   |   x   |   y   |   z   | parent (uint32) |
'-----------------------'-------------------------------'-----------------------'-----------------'
╵  posição (3x float)  ╵      rotação (4x float)      ╵   escala (3x float)    ╵

```

Esta abordagem permite que o runtime (especialmente em ambientes de código de baixo nível) evite etapas intermediárias de serialização e desserialização. A memória onde um `Transform` reside pode ser copiada, compartilhada e apontada diretamente.

As coordenadas podem ser fracionárias e, por isso, usam um número de ponto flutuante de 4 bytes.

O `parent` campo indica que esta Entity deve ser posicionada em relação a outra, identificada pelo seu ID.

Como o valor nulo para `parent` também é o [`RootEntity`](/docs/contributor/contributor-pt/runtime-da-scene/basic-entities/#rootentity) ID (`0`), qualquer `Transform` é relativo a ele por padrão.

Por protocolo, `Transform` deve ser serializado em ordem de bytes big-endian.

***

#### `MeshRenderer` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_renderer.proto#L7) <a href="#meshrenderer" id="meshrenderer"></a>

Fornece comportamento básico de rendering para uma Entity.

Pode ser definido para renderizar um plano, uma esfera, um cubo ou cilindro. Para renderizar formas 3D arbitrárias, use [`GltfContainer`](#gltfcontainer).

***

#### `MeshCollider` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/mesh_collider.proto#L15) <a href="#meshcollider" id="meshcollider"></a>

Fornece comportamento básico de colisão e detecção de mouse pointer para uma Entity.

Pode ser definido para se comportar como um plano, uma esfera, um cubo ou cilindro. Para ter uma collision mesh personalizada, use [`GltfContainer`](#gltfcontainer).

***

#### `Material` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/material.proto#L19) <a href="#material" id="material"></a>

Define as propriedades de texture, iluminação, cor e transparência de uma Entity que também possui o [`MeshRenderer`](#meshrenderer) component.

É uma estrutura complexa projetada para cobrir todos os casos comuns, mas cada field tem um valor padrão razoável e pode ser deixado não especificado.

Para personalizar completamente as propriedades visuais de uma Entity, use [`GltfContainer`](#gltfcontainer).

***

#### `GltfContainer` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/gltf_container.proto#L7) <a href="#gltfcontainer" id="gltfcontainer"></a>

Anexa um modelo 3D a esta Entity, dado o [caminho do file](/docs/contributor/contributor-pt/conteudo/entities/#files) de um `.gltf` asset no manifest da scene.

Como o modelo tem seus próprios meshes e materials, este component substitui qualquer comportamento de [`MeshRenderer`](#meshrenderer), [`MeshCollider`](#meshcollider) e [`Material`](#material).

***

#### `Billboard` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/billboard.proto#L17) <a href="#billboard" id="billboard"></a>

Faz uma Entity reorientar automaticamente sua `Transform` para ficar de frente para a câmera, ou opcionalmente para outra Entity referenciada via `target_entity`. Como o nome indica, é usado para exibir billboards in-game e frequentemente combinado com [`TextShape`](#textshape).

Afeta todas as direções por padrão, mas pode ser definido para girar apenas em um eixo. Se a target Entity referenciada não existir, a reorientação fica suspensa até que ela exista.

***

#### `Animator` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/animator.proto#L7) <a href="#animator" id="animator"></a>

Define uma ou mais animations que podem ser aplicadas simultaneamente a uma entity.

As scenes podem definir o estado do component para personalizar, reproduzir e fazer transição entre animations, além de lê-lo para verificar quais animations estão disponíveis ou estão sendo reproduzidas no momento.

***

#### `AvatarAttach` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_attach.proto#L16) <a href="#avatarattach" id="avatarattach"></a>

Indica que a posição de uma Entity deve seguir um ponto de ancoragem específico no body de um avatar.

Pode afetar qualquer avatar, não apenas o do jogador, definindo um user ID.

***

#### `TextShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/text_shape.proto#L11) <a href="#textshape" id="textshape"></a>

Renderiza texto na posição de uma Entity, dada por seu `Transform`.

É altamente configurável, suportando parâmetros como size, margin, padding, color, shadow e mais.

***

#### `Visibility` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/visibility_component.proto#L7) <a href="#visibility" id="visibility"></a>

Define se uma Entity está visível (o padrão) ou invisível.

Objetos invisíveis ainda existem e exibirão comportamento de quaisquer outros components anexados.

***

#### `AvatarShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_shape.proto#L8) <a href="#avatarshape" id="avatarshape"></a>

Contém informações sobre o avatar do jogador, incluindo sua body shape, cores, wearables e estado transitório.

Este component é anexado a entities no [avatar scene](/docs/contributor/contributor-pt/runtime-da-scene/execution/#avatarscene).

### Components de jogo <a href="#game" id="game"></a>

Alguns components básicos podem ser usados pela scene e pelo runtime para trocar informações. Eles aproveitam o [mecanismo de sincronização ECS](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/engine-api/#synchronization) para garantir consistência e ordenação para quaisquer alterações de state ou events.

***

#### `CameraMode` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode.proto#L8) <a href="#cameramode" id="cameramode"></a>

Pode ser usado para determinar se o jogador tem uma visão em 1ª pessoa ou 3ª pessoa.

***

#### `PointerLock` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_lock.proto#L7) <a href="#pointerlock" id="pointerlock"></a>

Pode ser usado para determinar se o mouse pointer está seguindo automaticamente o ponto de foco da câmera (locked) ou pode se mover livremente na tela (unlocked).

Ele está anexado ao [`CameraEntity`](/docs/contributor/contributor-pt/runtime-da-scene/basic-entities/#cameraentity), e seu estado pode ser lido (mas não escrito) a partir da scene.

***

#### `PointerEvents` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events.proto#L17) <a href="#pointerevents" id="pointerevents"></a>

Mostra feedback visual quando o pointer clica ou passa o mouse sobre uma Entity.

***

#### `PointerEventsResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/pointer_events_result.proto#L13) <a href="#pointereventsresult" id="pointereventsresult"></a>

Contém informações sobre input recente do jogador, incluindo keys e pointer actions.

Ele está anexado ao `RootEntity`, e atualizado pelo runtime com quaisquer novos events a cada frame.

***

#### `Raycast` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast.proto#L15) <a href="#raycast" id="raycast"></a>

Pode ser anexado a uma Entity para solicitar um raycast do game engine. O `RaycastResult` component será anexado posteriormente à mesma entity.

A origem, direção e comprimento máximo do ray podem ser configurados.

***

#### `RaycastResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/raycast_result.proto#L9) <a href="#raycastresult" id="raycastresult"></a>

Anexado pelo runtime a Entities que têm um [`Raycast`](#raycast) component aguardando results.

Contém informações sobre o ray original e identifica quaisquer Entities que foram atingidas.

### Components de mídia <a href="#media" id="media"></a>

As scenes podem anexar components especiais para exibir imagens, mostrar video ou tocar sounds.

***

#### `AudioSource` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_source.proto#L7) <a href="#audiosource" id="audiosource"></a>

Toca um áudio incluído com a scene, dado o [caminho do file no seu manifest](/docs/contributor/contributor-pt/conteudo/entities/#files).

O som se origina da posição da entity associada. Seu pitch, volume e comportamento de looping podem ser definidos, e o estado do audio player pode ser lido.


> [!NOTE]
> Em preparação para futuras atualizações, o field em `AudioSource` é chamado de `audio_clip_url`, mas na versão atual do protocol ele é, na verdade, o caminho definido no manifest.


***

#### `AudioStream` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/audio_stream.proto#L7) <a href="#audiostream" id="audiostream"></a>

Similar a [AudioSource](#audiosource), mas o áudio é transmitido em tempo real a partir de uma URL externa.

Apesar de estar anexado a uma entity específica, o som não é afetado por sua posição. Seu volume pode ser definido, e o estado do audio player pode ser lido.

***

#### `NftShape` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/nft_shape.proto#L36) <a href="#nftshape" id="nftshape"></a>

Exibe um NFT associado a um asset de imagem ou video.

Renderiza um canvas 2D com uma moldura decorativa configurável.

### Components de área

Esses components permitem que scenes modifiquem o comportamento padrão de Entities dentro de limites especificados.

***

#### `AvatarModifierArea` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/avatar_modifier_area.proto#L15) <a href="#avatarmodifierarea" id="avatarmodifierarea"></a>

Altera o comportamento de avatars dentro de um espaço centrado em torno de uma Entity.

É definido com um vetor de tamanho 3D e pode afetar se os avatars estão visíveis ou clicáveis. Avatars específicos podem ser excluídos desse efeito dado o ID do usuário.

***

#### `CameraModeArea` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/camera_mode_area.proto#L11) <a href="#cameramodearea" id="cameramodearea"></a>

Altera o modo de câmera (1ª pessoa ou 3ª pessoa) dentro de um espaço centrado em torno de uma Entity.

É definido com um vetor de tamanho 3D e um modo de câmera desejado.

### Components de UI <a href="#ui" id="ui"></a>

Os components a seguir são usados para criar interfaces gráficas que flutuam sobre o game world.

Geralmente são anexados a conjuntos de Entities que possuem hierarquia [`UiTransform`](#uitransform) components, relacionados entre si por meio do `parent` atributo.

Por exemplo, uma janela flutuante de opções poderia ser uma Entity com um [`UiTransform`](#uitransform) e um [`UiBackground`](#uibackground) component, além de uma Entity com uma child [`UiTransform`](#uitransform) e um [`UiDropdown`](#uidropdown).

***

#### `UiTransform` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_transform.proto#L77) <a href="#uitransform" id="uitransform"></a>

Descreve o tamanho, o posicionamento, a margin e o padding de um component de UI.

É baseado no modelo flexbox e altamente personalizável.

***

#### `UiBackground` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_background.proto#L11) <a href="#uibackground" id="uibackground"></a>

Descreve uma cor ou texture a ser usada como background em uma UI Entity.

***

#### `UiDropdown` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown.proto#L11) <a href="#uidropdown" id="uidropdown"></a>

Define uma lista de opções mutuamente exclusivas a serem exibidas em um widget dropdown.

***

#### `UiDropdownResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_dropdown_result.proto#L9) <a href="#uidropdownresult" id="uidropdownresult"></a>

Contém o valor selecionado de um [`UiDropdown`](#uidropdown), definido pelo runtime e lido pela scene.

***

#### `UiInput` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input.proto#L11) <a href="#uiinput" id="uiinput"></a>

Define um widget de entrada de texto, com alguma margem para personalização.

***

#### `UiInputResult` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_input_result.proto#L9) <a href="#uiinputresult" id="uiinputresult"></a>

Contém o valor de texto de um [`UiInput`](#uiinput), definido pelo runtime e lido pela scene.

***

#### `UiText` [↗ source](https://github.com/decentraland/protocol/blob/ccb88d679f20c0e22840c324879d7b2535f6c9a6/proto/decentraland/sdk/components/ui_text.proto#L11) <a href="#uitext" id="uitext"></a>

Define uma visualização de texto simples, com alguma margem para personalização.

***


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/basic-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
