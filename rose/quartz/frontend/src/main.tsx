import { render } from "solid-js/web";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Navigate,
  Outlet,
  RouterProvider,
} from "@tanstack/solid-router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";

import "@/assets/index.css";

import HomePage from "@/pages/home";
import { ThemeProvider } from "@/components/themeProvider";

const RootRoute = createRootRoute({
  component: () => <Outlet />,
  notFoundComponent: () => <Navigate to="/" />,
  errorComponent: (props) => <div>{`Error: ${props.error.message}`}</div>,
});

const HomeRoute = createRoute({
  getParentRoute: () => RootRoute,
  path: "/",
  component: HomePage,
});

const routeTree = RootRoute.addChildren([HomeRoute]);

const router = createRouter({ routeTree });

// Register the router instance for type safety
declare module "@tanstack/solid-router" {
  interface Register {
    router: typeof router;
  }
}

const queryClient = new QueryClient();
const rootElement = document.getElementById("root")!;

render(
  () => (
    <ThemeProvider defaultTheme="light" storageKey="vite-ui-theme">
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ThemeProvider>
  ),
  rootElement,
);
