import { Show } from "solid-js";

import ThemeToggle from "@/components/themeToggle";
import SolidLogo from "@/components/SolidLogo";

export default function Header(props: { title?: string; subtitle?: string }) {
  return (
    <header class="p-4 border-b flex items-center gap-4">
      <SolidLogo class="w-10 h-10" />
      <div>
        <p class="text-2xl font-bold">{props.title ?? "App"}</p>
        <Show when={props.subtitle}>
          <p class="text-[14px]">{props.subtitle}</p>
        </Show>
      </div>
      <div class="ml-auto">
        <ThemeToggle />
      </div>
    </header>
  );
}
