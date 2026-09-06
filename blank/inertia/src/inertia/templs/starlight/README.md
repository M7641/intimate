# Odyssey

[Starlight](https://starlight.astro.build/) template.

I have added a handful of custom components which is why I don't use the default template.

## Pre-requisites

You will need Node in order to run the frontend. My tool of choice is to use [Mise-en-Place](https://mise.jdx.dev/getting-started.html) for managing Node versions.

```bash
curl https://mise.run | sh
eval "$(mise activate bash)"
mise use --global node@latest bun@latest
```

Then, in the folder of the app:

```bash
bun install
bun run dev
```

Once confirmed to be working, you can add either `.md` or `.mdx` files to the `content/docs` folder. To add these new pages to the sidebar, edit the `astro.config.mjs` file.
