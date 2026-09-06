import type { ComponentProps } from "solid-js";

export default function SolidLogo(props: ComponentProps<"svg">) {
  return (
    <svg
      viewBox="0 0 166 155.3"
      xmlns="http://www.w3.org/2000/svg"
      role="img"
      aria-label="SolidJS logo"
      {...props}
    >
      <path
        fill="#76b3e1"
        d="M163.7 35S110-4.7 69 4.2l-3 1c-6.2 2-11.4 5-14.3 9l-2 3-15 26 26 4c11 7 25 10 38 7l46 9 18-32z"
      />
      <path
        fill="#518ac8"
        d="M163.7 35S110-4.7 69 4.2l-3 1c-6.2 2-11.4 5-14.3 9l-2 3-15 26 26 4c11 7 25 10 38 7l46 9 18-32z"
        opacity=".3"
      />
      <path
        fill="#76b3e1"
        d="M52 35l-4 1c-17 5-22 21-13 35 10 13 31 20 48 15l62-21S92 27 52 35z"
      />
      <path
        fill="#fff"
        d="M134 80a37 37 0 0 0-45-26L27 75 7 109l112 19 20-36a37 37 0 0 0-5-12z"
        opacity=".3"
      />
      <path
        fill="#fff"
        d="M122 102a37 37 0 0 0-45-26L15 97s53 40 94 31l3-1c17-5 23-21 10-25z"
      />
    </svg>
  );
}
