> Configuración y rendimiento en Decentraland

# Configuración y rendimiento

Cuando estés en el mundo, busca el ícono de engranaje en el menú lateral para acceder a la configuración de Graphic, Sound, Control y Chat.

## Optimización del rendimiento de Decentraland en tu dispositivo

Renderizar el mundo abierto de Decentraland requiere cierto nivel de potencia técnica. Si tu computadora no cumple con los requisitos mínimos que se indican a continuación, podrías experimentar un rendimiento deficiente y tiempos de carga largos. En este escenario, puedes intentar optimizar el rendimiento de Decentraland reduciendo tu configuración de Graphic en el mundo.

### **Windows**

|         | Requisitos mínimos                                                           | Configuración recomendada                        |
| ------- | ---------------------------------------------------------------------------- | ------------------------------------------------ |
| OS      | Windows 10 de 64 bits                                                        | Windows 11 de 64 bits                            |
| CPU     | Intel i5 de 7.ª generación o AMD Ryzen 5 Series                              | Intel i7 de 12.ª generación o AMD Ryzen 7 Series |
| GPU     | Nvidia RTX 20 Series o AMD Radeon RX 5000 Series (compatible con DirectX 12) | Nvidia RTX 30 Series o AMD Radeon RX 6000 Series |
| VRAM    | 6 GB                                                                         | 12 GB                                            |
| RAM     | 16GB                                                                         | 32 GB                                            |
| Storage | 8GB HDD                                                                      | 20GB SDD                                         |

### **Mac**

|         | Requisitos mínimos                                            | Configuración recomendada         |
| ------- | ------------------------------------------------------------- | --------------------------------- |
| OS      | macOS 11 Big Sur                                              | macOS 12 Monterey                 |
| CPU     | Apple M1                                                      | Apple M1 Pro/M2                   |
| GPU     | GPU integrada Apple M1 (se requiere compatibilidad con Metal) | GPU integrada Apple M1 Pro/M2 Pro |
| VRAM    | 6 GB                                                          | 12 GB                             |
| RAM     | 16GB                                                          | 32 GB                             |
| Storage | 8GB HDD                                                       | 20GB SDD                          |

### Ajustando la configuración de Graphic

Si tu computadora no cumple con todos los requisitos mínimos de hardware de Decentraland, puedes intentar optimizar tu experiencia reduciendo tu configuración de graphic en el mundo. Aquí están los ajustes para obtener el mejor rendimiento:

![](/docs/_remote/154c3847b6bd7b0396f9089c.webp)

## Configuración de Sound

Modifica el volumen de sonidos individuales para personalizar tu experiencia en Decentraland:

* **Maestro:** Afecta a todos los sonidos
* **UI SFX:** Afecta a la música que suena cuando inicias sesión en Decentraland
* **Música y SFX en el mundo:** Afecta a la música, el audio de streams y cualquier otro efecto de sonido generado por una Scene
* **Avatar & Emote SFX:** Afecta a los sonidos de Avatar y Emote con efectos de sonido

![](/docs/_remote/2b2767783d43b899be6c38c3.webp)

## Configuración de Control

Aquí puedes modificar la sensibilidad de tu mouse dentro del mundo.

![](/docs/_remote/821016ae97631107c7bb7d45.webp)

## Configuración de Chat

Desde la configuración principal, puedes modificar las preferencias de notificaciones y de burbujas de chat para todos tus chats simultáneamente, así como los permisos de tus DM. Consulta la [**Chat**](/docs/player/player-es/en-el-mundo/friends-and-chatting/#chat-settings) sección Chat para ver el desglose completo.

![](/docs/_remote/d045987d8716c9125d8f9712.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-es/en-el-mundo/settings-and-performance.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
