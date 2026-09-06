// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import catppuccin from "@catppuccin/starlight";
import starlightLinksValidator from "starlight-links-validator";
import starlightImageZoom from "starlight-image-zoom";
import remarkMath from "remark-math";
import rehypeMathjax from "rehype-mathjax";

import os from "os";

const TENANT = process.env.TENANT || "";

// Get workspace ID from hostname
const hostname = os.hostname() || "";
const workspaceId = hostname.includes("workspace-")
  ? hostname.split("workspace-")[1].slice(0, -2)
  : "";
const userName = process.env.USER || "";

// https://astro.build/config
export default defineConfig({
  markdown: {
    remarkPlugins: [remarkMath],
    rehypePlugins: [rehypeMathjax],
  },
  integrations: [
    starlight({
      title: "{{ project_title }}",
      components: {
        Footer: "./src/components/Footer.astro",
        MarkdownContent: "./src/components/MarkdownContent.astro",
        ThemeProvider: "./src/components/ThemeProvider.astro",
        ThemeSelect: "./src/components/ThemeSelect.astro",
      },
      plugins: [catppuccin(), starlightLinksValidator(), starlightImageZoom()],
      customCss: ["./src/styles/custom.css", "./src/styles/cyber.css"],
      favicon: "/logo-dark.svg",
      sidebar: [
        { label: "Introduction", slug: "index" },
        {
          label: "Guides",
          items: [
            // Each item here is one entry in the navigation menu.
            { label: "Example Guide", slug: "guides/example" },
          ],
        },
        {
          label: "Reference",
          autogenerate: { directory: "reference" },
        },
      ],
    }),
  ],
  server: {
    host: "localhost",
    port: 5173,
    allowedHosts: [`${TENANT}-${workspaceId}.nimbus.example`],
  },
  base: hostname.includes("workspace-")
    ? `/user/${userName}/proxy/absolute/5173`
    : "/",
});
