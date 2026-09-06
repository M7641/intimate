import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";

import "@/assets/index.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { ThemeProvider } from "./components/themeProvider.tsx";
import { buildRouter } from "@/routes";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

// This app is Redshift-only, so the router is built statically — no runtime
// warehouse-type discovery.
const router = buildRouter();

const rootElement = document.getElementById("root")!;
if (!rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);
  root.render(
    <StrictMode>
      <ThemeProvider defaultTheme="light" storageKey="vite-ui-theme">
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>
      </ThemeProvider>
    </StrictMode>,
  );
}
