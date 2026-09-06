import ThemeToggle from "@/components/themeToggle.tsx";
import Logo from "@/assets/Logo.tsx";

export default function Header({
  title = "App",
  subtitle,
}: {
  title?: string;
  subtitle?: string;
}) {
  return (
    <header className="p-4 border-b flex items-center gap-4">
      <Logo className="w-10 h-10" />
      <div>
        <p className="text-2xl font-bold">{title}</p>
        {subtitle && <p className="text-[14px]">{subtitle}</p>}
      </div>
      <div className="ml-auto">
        <ThemeToggle />
      </div>
    </header>
  );
}
