# Models

Each kit sits in its own folder with its licence and its `Textures/`
folder, because the `.glb` files reference `Textures/colormap.png` by
relative path.

| Folder                       | Source                              | Licence |
| ---------------------------- | ----------------------------------- | ------- |
| `kenney/car-kit`             | kenney.nl/assets/car-kit (3.1)      | CC0     |
| `kenney/city-kit-suburban`   | kenney.nl/assets/city-kit-suburban (2.0) | CC0 |

Only a subset of each kit is copied in. Fetch the full kit from the source
to add more.

Kenney's models are toy-scaled: a car is about 2.5 units long and a house
about 1.3 units wide. The scene scales them up on spawn. The car kit's
vehicles face `+Z`, the opposite of the glTF convention, so the body is
turned half a turn when it is parented to the physics body.
