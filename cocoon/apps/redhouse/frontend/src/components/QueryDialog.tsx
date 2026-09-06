import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

/// A "View" button that opens the full SQL text in a wide, scrollable dialog.
export default function QueryDialog({ query }: { query: string }) {
  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm" className="h-7 px-2 text-xs">
          View
        </Button>
      </DialogTrigger>
      <DialogContent className="w-[90vw] max-w-6xl sm:max-w-6xl">
        <DialogHeader>
          <DialogTitle>Full query</DialogTitle>
        </DialogHeader>
        <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded bg-muted p-4 text-xs font-mono">
          {query || "—"}
        </pre>
      </DialogContent>
    </Dialog>
  );
}
