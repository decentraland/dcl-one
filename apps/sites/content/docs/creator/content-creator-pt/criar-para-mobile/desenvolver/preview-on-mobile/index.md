> Pré-visualize sua scene em um dispositivo móvel real a partir do Creator Hub ou da CLI.

# Pré-visualização no Celular

Pode pré-visualizar a sua cena diretamente na aplicação móvel da Decentraland a partir do Creator Hub ou da linha de comando. Esta é a única forma fiável de confirmar que a sua UI, o tratamento de input e o desempenho se aguentam num dispositivo real.

## Pré-requisitos

* A aplicação móvel da Decentraland instalada no seu telefone — consulte os [links de download](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/#get-the-mobile-app).
* O telefone e a sua máquina de desenvolvimento têm de estar na **mesma rede local** (Wi‑Fi). A pré-visualização é servida a partir do seu computador; o código QR aponta para um URL da LAN.

## Opção A — A partir do Creator Hub

1. Abra a sua cena no Creator Hub.
2. Clique na seta ao lado do **Play** botão para abrir **Play Options**, e depois escolha **Mostrar código QR para telemóvel**.
3. Leia o código QR apresentado com a câmara do seu telefone. A ligação abre a aplicação móvel da Decentraland e carrega a pré-visualização da sua cena. Clique na área escura em redor do pop-up para o fechar.

<figure><img src="/docs/_remote/c6aa2bdbe9bdd432e1e7e96d.webp" alt="Creator Hub Play Options menu with the Show QR Code for Mobile option"><figcaption><p>A opção "Show QR Code for Mobile" no menu Play Options do Creator Hub.</p></figcaption></figure>

## Opção B — A partir da linha de comando

A partir da raiz do projeto da sua cena, execute:

```bash
npm run start -- --mobile
```

O CLI imprime um código QR no terminal que aponta para o URL da LAN da sua cena. Leia-o com o telefone para carregar a cena na aplicação móvel da Decentraland.

<figure><img src="/docs/_remote/ce46ee596043fea52191ae3d.webp" alt="QR code printed by the Decentraland CLI for mobile preview"><figcaption><p>Um código QR impresso por <code>npm run start -- --mobile</code>. Leia-o com o telefone para abrir a cena na aplicação móvel da Decentraland.</p></figcaption></figure>


> [!NOTE]
> **💡 Dica**: Quando passa `--mobile`, o Explorer de desktop também não é iniciado. Se quiser testar em ambos ao mesmo tempo, execute `npm run start` num terminal para desktop e `npm run start -- --mobile` noutro para mobile.


## Recarregamento automático

Tal como na pré-visualização no desktop, a pré-visualização no mobile recarrega automaticamente quando altera os ficheiros da cena. Não precisa de voltar a ler o código QR após cada edição.

## Resolução de problemas

* **O código QR não abre a aplicação** — certifique-se de que a aplicação Decentraland está instalada e de que a abriu pelo menos uma vez no dispositivo.
* **O telefone não consegue chegar ao servidor de pré-visualização** — confirme que o telefone e a máquina de desenvolvimento estão na mesma rede Wi‑Fi. Algumas redes empresariais isolam os clientes; nesse caso, use um hotspot pessoal ou uma rede diferente.
* **A cena carrega no telefone, mas parece diferente do desktop** — isso é esperado. Use isto como o momento para validar a sua [área segura](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/), [dimensão da UI](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/ui-best-practices/#sizing), e [associações de input](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/).

## Relacionado

* [Pré-visualize a sua cena](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/)
* [Usando a CLI](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/)
* [Tenha destaque no Discover mobile](/docs/creator/content-creator-pt/criar-para-mobile/publicar/get-featured/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
