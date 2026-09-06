import { FaRegQuestionCircle } from "react-icons/fa";
import { cn } from "@/lib/utils";

/**
 * Small accessible "?" affordance that reveals a plain-language explanation on
 * hover or keyboard focus. Pure CSS (no portal/library) to stay light, and
 * surfaces the same text via `aria-label` for assistive tech.
 */
export default function InfoHint({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  return (
    <span
      className={cn("group/hint relative inline-flex align-middle", className)}
    >
      <button
        type="button"
        aria-label={text}
        className="text-muted-foreground/60 hover:text-foreground focus-visible:text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring/50 rounded-full cursor-help"
      >
        <FaRegQuestionCircle className="h-3 w-3" />
      </button>
      <span
        role="tooltip"
        className="pointer-events-none absolute top-full left-1/2 z-50 mt-1.5 w-48 -translate-x-1/2 rounded-md border bg-popover px-2 py-1.5 text-xs font-normal normal-case tracking-normal leading-snug text-popover-foreground opacity-0 shadow-md transition-opacity duration-150 group-hover/hint:opacity-100 group-focus-within/hint:opacity-100"
      >
        {text}
      </span>
    </span>
  );
}
