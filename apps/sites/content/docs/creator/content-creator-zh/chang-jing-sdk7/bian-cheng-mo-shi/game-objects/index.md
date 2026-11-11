> 使用游戏对象模式让代码更易读，也更易扩展。

# 游戏对象

随着场景变得越来越复杂，把部分逻辑放到独立的游戏对象文件中会很有用。这样做可以让场景的主代码保持简洁、易读，同时将控制场景中多个实体的可复用部分封装起来。

游戏对象保存了你在场景中可能遇到的某类实体的所有属性和方法，例如门、按钮或怪物。得益于这种抽象，包含场景主要逻辑的文件只需一行代码就能实例化完整的游戏对象。你也可以同样轻松地调用这些对象的方法。

我们建议将每个游戏对象定义放在单独的文件中。

## 类型对象模式

如果你的场景使用了多种彼此有很多共同点的游戏对象类型，你可能会想进一步重构，并定义一个 *base* 包含这些对象共同特性的游戏对象类型。然后你可以创建继承自这个基类的子类。

你可以在以下内容中进一步了解这种模式： [《游戏编程模式》](http://www.gameprogrammingpatterns.com/type-object.html).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/game-objects.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
