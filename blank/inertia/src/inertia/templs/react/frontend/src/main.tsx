import { StrictMode } from "react";
import ReactDOM from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";

import "@/assets/index.css";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import {
  createRootRoute,
  createRoute,
  createRouter,
  Navigate,
} from "@tanstack/react-router";
import { Outlet } from "@tanstack/react-router";
import HomePage from "@/pages/home.tsx";
import { ThemeProvider } from "./components/themeProvider.tsx";

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const RootRoute = createRootRoute({
  component: Outlet,
  notFoundComponent: () => <Navigate to="/" />,
  errorComponent: ({ error }) => <div>{`Error: ${error.message}`}</div>,
});

const HomeRoute = createRoute({
  getParentRoute: () => RootRoute,
  path: "/",
  component: HomePage,
});

const routeTree = RootRoute.addChildren([HomeRoute]);

const router = createRouter({ routeTree });

const queryClient = new QueryClient();
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
