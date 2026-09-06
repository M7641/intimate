import { Show } from "solid-js";
import { Moon, Sun, Zap } from "lucide-solid";

import { Button } from "@/components/ui/button";
import { useTheme } from "@/components/themeProvider";

const order = ["light", "dark", "cyber"] as const;

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  const cycle = () => {
    const next = order[(order.indexOf(theme()) + 1) % order.length];
    setTheme(next);
  };

  return (
    <Button
      variant="outline"
      size="icon"
      onClick={cycle}
      aria-label={`Switch theme (current: ${theme()})`}
    >
      <Show when={theme() === "light"}>
        <Sun />
      </Show>
      <Show when={theme() === "dark"}>
        <Moon />
      </Show>
      <Show when={theme() === "cyber"}>
        <Zap />
      </Show>
    </Button>
  );
}
