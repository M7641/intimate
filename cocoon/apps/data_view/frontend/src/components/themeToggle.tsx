import { Moon, Sparkles, Sun } from "lucide-react";
import { BsMotherboard } from "react-icons/bs";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useTheme } from "@/components/themeProvider";

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  let icon;
  if (theme === "dark") {
    icon = <Moon className="h-[1.2rem] w-[1.2rem] transition-all" />;
  } else if (theme === "cyber") {
    icon = <BsMotherboard className="h-[1.2rem] w-[1.2rem] transition-all" />;
  } else if (theme === "celestial") {
    icon = <Sparkles className="h-[1.2rem] w-[1.2rem] transition-all" />;
  } else {
    icon = <Sun className="h-[1.2rem] w-[1.2rem] transition-all" />;
  }

  return (
    // Non-modal: a modal menu marks the rest of the page (incl. #root, which
    // holds the still-focused trigger) aria-hidden, which the browser warns
    // about. A theme picker has no need to trap focus.
    <DropdownMenu modal={false}>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="icon">
          {icon}
          <span className="sr-only">Toggle theme</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => setTheme("light")}>
          Light
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => setTheme("dark")}>
          Dark
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => setTheme("cyber")}>
          Cyber
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => setTheme("celestial")}>
          Celestial
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
