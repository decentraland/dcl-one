> >-

# Game Objects

As your scene becomes more complicated, it's useful to put some of the logic into separate game object files. By doing this, you can keep the main code for the scene clean and simple to read, while encapsulating reusable parts that control several entities in the scene.

A Game Object holds all the properties and methods for a type of entity you might find in your scene, for example a door or a button or a monster. Thanks to this abstraction, the files with your scene's main logic can instance full game objects through just one line of code. You can also call the methods on these objects with the same ease.

We recommend keeping game object definitions each in a separate file.

## The Type Object Pattern

If your scene uses various types of game objects that have much in common, you might want to take this refactor even further, and define a _base_ game object type with all the things that these have in common. You can then create sub classes that inherit from this base class.

You can read more about this pattern in [Game Programming Patterns](http://www.gameprogrammingpatterns.com/type-object.html).
