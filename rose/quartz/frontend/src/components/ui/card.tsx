import type { Component, ComponentProps } from "solid-js";
import { splitProps } from "solid-js";

import { cn } from "@/lib/utils";

const Card: Component<ComponentProps<"div">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <div
      data-slot="card"
      class={cn(
        "bg-card text-card-foreground flex flex-col gap-6 rounded-xl border py-6 shadow-sm",
        local.class,
      )}
      {...rest}
    />
  );
};

const CardHeader: Component<ComponentProps<"div">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <div
      data-slot="card-header"
      class={cn("flex flex-col gap-1.5 px-6", local.class)}
      {...rest}
    />
  );
};

const CardTitle: Component<ComponentProps<"h3">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <h3
      data-slot="card-title"
      class={cn("font-semibold leading-none", local.class)}
      {...rest}
    />
  );
};

const CardDescription: Component<ComponentProps<"p">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <p
      data-slot="card-description"
      class={cn("text-muted-foreground text-sm", local.class)}
      {...rest}
    />
  );
};

const CardContent: Component<ComponentProps<"div">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <div data-slot="card-content" class={cn("px-6", local.class)} {...rest} />
  );
};

const CardFooter: Component<ComponentProps<"div">> = (props) => {
  const [local, rest] = splitProps(props, ["class"]);
  return (
    <div
      data-slot="card-footer"
      class={cn("flex items-center px-6", local.class)}
      {...rest}
    />
  );
};

export {
  Card,
  CardHeader,
  CardFooter,
  CardTitle,
  CardDescription,
  CardContent,
};
