import { Moon, Sun } from "lucide-react";
import { BsMotherboard } from "react-icons/bs";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useTheme } from "@/components/themeProvider.tsx";

export default function ThemeToggle() {
  const { theme, setTheme } = useTheme();

  let icon;
  if (theme === "dark") {
    icon = <Moon className="h-[1.2rem] w-[1.2rem] transition-all" />;
  } else if (theme === "cyber") {
    icon = <BsMotherboard className="h-[1.2rem] w-[1.2rem] transition-all" />;
  } else {
    icon = <Sun className="h-[1.2rem] w-[1.2rem] transition-all" />;
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="icon">
          {icon}
          <span className="sr-only">Toggle theme</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={() => setTheme("light")}>
          Light
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => setTheme("dark")}>
          Dark
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => setTheme("cyber")}>
          Cyber
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
