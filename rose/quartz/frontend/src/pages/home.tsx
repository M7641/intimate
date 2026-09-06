import { Show } from "solid-js";
import { useQuery } from "@tanstack/solid-query";

import Header from "@/components/Header";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type Hello = { message: string; framework: string };

async function fetchHello(): Promise<Hello> {
  const res = await fetch("/api/hello");
  if (!res.ok) throw new Error(`Request failed: ${res.status}`);
  return res.json();
}

export default function HomePage() {
  // TanStack Query caches the result on the client, exactly like the React template.
  const hello = useQuery(() => ({ queryKey: ["hello"], queryFn: fetchHello }));

  return (
    <div class="p-1 overflow-y-auto">
      <Header
        title="Quartz"
        subtitle="SolidJS · shadcn-solid · TanStack Router"
      />

      <div class="p-6 max-w-xl">
        <Card>
          <CardHeader>
            <CardTitle>FastAPI says…</CardTitle>
            <CardDescription>
              Fetched from <code>/api/hello</code> through TanStack Solid Query.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Show
              when={hello.data}
              fallback={
                <p class="text-muted-foreground text-sm">
                  {hello.isError
                    ? `Error: ${hello.error?.message}`
                    : "Loading…"}
                </p>
              }
            >
              {(data) => (
                <p class="text-lg">
                  {data().message} —{" "}
                  <span class="font-semibold">{data().framework}</span>
                </p>
              )}
            </Show>
          </CardContent>
          <CardFooter class="gap-2">
            <Button onClick={() => hello.refetch()}>Refetch</Button>
            <Button variant="outline" onClick={() => hello.refetch()}>
              Outline
            </Button>
          </CardFooter>
        </Card>
      </div>
    </div>
  );
}
