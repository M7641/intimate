# Cartridge

A module manager for DS projects.

# Technical issues to resolve:

1. Is it worth having a global config that the package creates to keep track of which modules are included so far? Thinking it's a dict that we add and remove stuff from. Might introduce strange behaviours if it's not very deeply connected with the code base however. Not sure I would want to try maintain a state like that. On the other hand, what will we have to interface with?

It might be possilbe to install the packages in the add command, however that feels very wrong. Can you link multiple requirements files together? That could be another idea, each module has it's own req files.

It might be worth trying to come up with a multi module approach. An add command would just add another package and then we would have an m install which would install that added module to the overall repo which you can then import and use as normal. I don't mind that idea, would lay the groundwork for a full seperation in the future. It does get very noisy.

Does python have a similar feature to Cargo where you can specify features? I think it does, with the `[]` syntax. I can imagine `pip install m["module1", "module2"]`.

Worth adding I think cargo has inspired this idea by accident. As Rust has repos where there are multiple directories acting as stand alone packages. Rust makes that super clean, I don't think Python will. https://github.com/launchbadge/sqlx/tree/main/sqlx-macros-core

This must be a pattern someone has done already in the Python world. Indeed, some ideas are:

1. https://github.com/effigies/hatch-monorepo/tree/main from https://discuss.python.org/t/multiple-packages-from-same-src-using-pyproject-toml/30998/12
2. https://github.com/Azure/azure-sdk-for-python/tree/main/sdk from https://discuss.python.org/t/how-to-best-structure-a-large-project-into-multiple-installable-packages/5404

## Initial idea of desired interface:

```bash

m add <module> --options

# eg.
m add elasticiy --type=loglog

# Install all the modules
m install

# deploy all modules if no module is specified
m deploy --module <module>
```

## How would we do versioning?

Versioning will line up with the version of the repo. If this approach even works, then it's natural we would start to split them out into their own repos anyway and then we can version them seperately. This repo could then become a wrapper that loads in modules from github so you don't have to clone them.

Why would you not want to install each one? That could work as well, but the DS then loses total control of the code which is why this even exists in the first place. Hence I hope this is always a step that gives you the most up to date code that works and will deliver with no fuss. It's a template to start work from in the worst case.

Upadting is something I would have to think about carefully as I don't want to be deleting any code. Hence, I think I want to stick with just `add` and the user has to manually move to source code to get a new added version.

## How would we do testing?

Will have to see how fleixble pytest will be when it comes to what directory the files are in.

My desire would be:

```bash

m test --module <module>

```

The guiding pricniple for tests is to serve as the desired interface for the code. For example, if a change impacts how the interaface works then that should fail a test.

## How would we do deployment?

There will be deploy scripts somewhere which will use the `nimbus-sdk` and all the config for those will be in the scripts them selves so there won't be any yaml files or anything like that, so when I settle on a pattern that will just fall into place. The deploy command will just crawl through each one with multiprocess of course.

The home of the Dockerfiles is a question. Also, I think this becomes easy when we know what kind of structure will be needed to compose these modules together into one repo. Ideally it will be in the root repos of each module.

M it self can always include the uilts and the like to actually do that deploy script. We don't have to have it all in the open. That could be a principle I want to follow though, that being after the add stage you should be able to reomve the m package and still have a working repo. That's a nice idea. Question is how nice of an idea is that compared to the redudancy that would require?

Also, we will be following `Dockerfile.<module>` naming. There will be redudancy in this approach, but it will be very clear what is going on which is more important users can then alter to suit them. I could code it to swap out the final lines so I could avoid the redudancy here.

## How would we do documentation?

The main things that need to be comunicated on the surface is what the epected inputs are.

Otherwise, the code should be readable without documentation. Documentation will be done in the code itself when it's valuable to do so. I expect DS to be able to read code.

## How would we do logging?

We don't. The code should return useful errors.

## How would we run the code?

Each module would have a `main.py` file that would be run which should be a viable pattern of how the code should be run. An optional step is to have a

```bash

m run --module <module> --args <args>

```

as well.

## Principles

1. DS first
2. Total control for the user
3. Minimalism
4. Composition
5. Power
6. Solve problems when they actually occur and not problems that might occur.

## Aim

Rapidly deploy DS code to swap out blocks within apps with no fuss.

## Descision Making

Done by Informed Captain / Benevolent Dictator approach. Dissent on code choices is expected and commitement to the final decision is also expected.


## Inspiration sources

1. https://ui.shadcn.com/docs - inspired the apporach that library can inject code into a repo and then the user can modify it.
2. poetry deomstrating progromatic approach to package management. Although uv has a ligher touch, but kept that feature.
3. templates, but in the oposite sense. Rather than having a template where you have to cut it down to size, this is a template builder where you work up to what you see fit for the project.

## Future work

1. Ability to add a basic dash app.
2. Remix app.
3. Rust backend.
4. Empty workflow.

Also, I do think there is value in the idea of just copying in mini projects that you install as if they were all stand alone packages. They can all have a default interface that is `module run --args`. Each would have a cli and it would just be the unit it represents. You would then have to go through each one and install it and it is a bit cumbersome compared to what this is turning into. However, that redudancy is probably a good thing for the average user. Each cube is stand alone in the truest sense and you can get a really nice seperation of ideas. It also enables multi language as well given each one is so distinct. Furthermore, the "meta" package in this case and just be purely focued on hygeine and the highest level of tooling.

Idea as dir structure:

```bash
modules:
    module1
        pyproject.toml
        src
            main.py
            cli.py
            utils.py
            ...
        Dockerfile.module1
    module2
    module3
    ...
    moduleN
src
    main.py
    cli.py
    utils.py
    ...
pyproject.toml
README.md
MANIFEST.in
.pre-commit-config.yaml
```

Extened idea on the already extended idea, the meta package can then also have the "recipie book" layer that would know what combinations of moudles are in markdown for example.

### Tmp runing python 3.11

```bash
uv python install 3.11
uv venv <name> --python 3.11
```
