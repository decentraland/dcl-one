> Gestiona quién puede operar una campaña de Decentraland Rewards asignando roles y permisos a direcciones de wallet.

# Usuarios y roles

Para realizar cualquier acción en una campaign dentro del servicio de Rewards, debes haber iniciado sesión con un wallet que tenga los permisos necesarios.

Cuando creas una campaign, se te asigna automáticamente como su owner y se te conceden todos los permisos. Como owner, puedes añadir a otros usuarios a la campaign y asignarles roles con permisos específicos. Sin embargo, cada campaign solo puede tener un owner —el creador—, que es el único usuario con autoridad para gestionar los fondos.

Para añadir un usuario a la campaign.

1. Haz clic en el **Roles de usuario** pestaña para ver la lista de usuarios que tienen acceso a la campaign.
2. Al final de esta lista, encontrarás un formulario para añadir un nuevo usuario. Nota: este formulario solo será visible si tienes los permisos necesarios para añadir usuarios.
3. Completa el formulario introduciendo la dirección wallet del usuario (o el nombre ENS) y seleccionando su rol.
4. Haz clic en el **Añadir usuario** botón para finalizar la adición.

![](/docs/_remote/f926b41052805cdf281f1c90.webp)

## Permisos para los roles

Estas son las acciones permitidas para cada rol:

| Permiso                                                  | visor | desarrollador | colaborador | propietario |
| -------------------------------------------------------- | :---: | :-----------: | :---------: | :---------: |
| Ver el nombre de la campaign, la network y el gas máximo |   ✅   |       ✅       |      ✅      |      ✅      |
| Editar el nombre de la campaign y el gas máximo          |   ❌   |       ❌       |      ✅      |      ✅      |
| Activar y desactivar la campaign                         |   ❌   |       ❌       |      ✅      |      ✅      |
| Ver suministro                                           |   ✅   |       ✅       |      ✅      |      ✅      |
| Añadir/eliminar Supply                                   |   ❌   |       ✅       |      ✅      |      ✅      |
| Ver dispensers                                           |   ✅   |       ✅       |      ✅      |      ✅      |
| Ver clave del dispenser                                  |   ❌   |       ✅       |      ✅      |      ✅      |
| Añadir/eliminar dispenser                                |   ❌   |       ✅       |      ✅      |      ✅      |
| Ver transacciones de la campaign                         |   ✅   |       ✅       |      ✅      |      ✅      |
| Ver usuarios                                             |   ✅   |       ✅       |      ✅      |      ✅      |
| Añadir/eliminar usuarios                                 |   ❌   |       ❌       |      ✅      |      ✅      |
| Eliminar usuarios propietarios                           |   ❌   |       ❌       |      ❌      |      ✅      |
| Gestionar fondos                                         |   ❌   |       ❌       |      ❌      |      ✅      |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/recompensas/users-and-roles.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
