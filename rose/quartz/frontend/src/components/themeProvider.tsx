import {
  createContext,
  createEffect,
  createSignal,
  useContext,
  type Accessor,
  type JSX,
} from "solid-js";

type Theme = "dark" | "light" | "cyber";

type ThemeProviderProps = {
  children: JSX.Element;
  defaultTheme?: Theme;
  storageKey?: string;
};

type ThemeProviderState = {
  theme: Accessor<Theme>;
  setTheme: (theme: Theme) => void;
};

const ThemeProviderContext = createContext<ThemeProviderState>();

export function ThemeProvider(props: ThemeProviderProps) {
  const storageKey = () => props.storageKey ?? "vite-ui-theme";

  const [theme, setThemeSignal] = createSignal<Theme>(
    (localStorage.getItem(storageKey()) as Theme) ??
      props.defaultTheme ??
      "light",
  );

  // Mirror the active theme onto the root element so the CSS variables apply.
  createEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark", "cyber");
    root.classList.add(theme());
  });

  const value: ThemeProviderState = {
    theme,
    setTheme: (next: Theme) => {
      localStorage.setItem(storageKey(), next);
      setThemeSignal(next);
    },
  };

  return (
    <ThemeProviderContext.Provider value={value}>
      {props.children}
    </ThemeProviderContext.Provider>
  );
}

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined)
    throw new Error("useTheme must be used within a ThemeProvider");

  return context;
};
