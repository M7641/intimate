A definition of a parser just read: A parser is a function that consumes less structured input and produces more structured output.

It's a partial function in which values in the domain lack a corresponding target value. This means they inherently can always fail. This is a mathematical concept originally. On the computing side, it's usually bytes being parsed into other structured text. 

For example, a byte stream is being parsed into a JSON file. The possible domain is all byte combinations, but only a small subset can be mapped to a valid JSON file. 

Therefore, if you parse all your inputs at the walls of your program, you know within your program what set of values you can work with. 

Interestingly, this does not work well in Python. Python does not have static typing, so you can quickly lose track of what an object is. Rust or Haskell just won't let you compile the code unless you've been totally consistent with a data object throughout the entire lifetime of the code.

How would this idea compare with just validating data? As a critique, we have some ideas raised:

Just validating data leads to shotgun parsing. A common case: a date field validated repeatedly to ensure it is between 1 and 7. Which was a lot of wasted checking. If it was useful, that implies you had corrupt data in your application, so if this approach was valuable, you are already in a spot of bother. In other kinds of programs, if this invalid input actually got saved or used, then you've poisoned your system, and you might not be able to get back easily to a clean state. 

A key takeaway is that validations don't return anything, which makes them effectively optional and easy to ignore or remove. They sit atop a solution, adding complexity without being integral to it. Whereas, if you parse, it becomes an integral part of the solution and creates a black-and-white wall around the application. 

You make data structures that won't work illegally from the outset. 

You specify what works, not what will not work. 

If parts of the application require more precise data models, you can parse into more precise data structures, as if it were an application within an application. 

Other points:
1. Let your datatypes inform your code, don't let your code control your datatypes. An example is throwing a bool into a data structure just because a function needs it. Rather, find a way to make your function work with the real data. 
2. Functions that don't return anything are inherently strange. What are they doing? Are there better ways than just creating a route in the code to nowhere?
3. You can parse some data to determine how to parse the rest.
4. Avoid denormalised representations. Strive for a single source of truth rather than duplicating data across multiple data objects. 
5. Make validators a fake parser. If you require a range of integers, there's no natural parser for this. Rather, you can wrap the integer parsing in a step process that validates the range whilst parsing. This is at least encapsulated. 

Type-driven design is a phrase that encapsulates some of these ideas. Pydantic is an attempt to add this to Python, but the language does not naturally have it. 