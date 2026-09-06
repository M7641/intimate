import { Button } from "@/components/ui/button";
import { FaDownload } from "react-icons/fa";

type CsvExportButtonProps<T extends Record<string, unknown>> = {
  data: T[];
  filename?: string;
  headers?: { key: keyof T; label: string }[];
};

export default function CsvExportButton<T extends Record<string, unknown>>({
  data,
  filename = "export",
  headers,
}: CsvExportButtonProps<T>) {
  const exportToCsv = () => {
    if (!data || data.length === 0) return;

    const keys = headers
      ? headers.map((h) => h.key)
      : (Object.keys(data[0]) as (keyof T)[]);

    const headerRow = headers
      ? headers.map((h) => h.label).join(",")
      : keys.join(",");

    const rows = data.map((row) =>
      keys
        .map((key) => {
          const value = row[key];
          if (value === null || value === undefined) return "";
          const stringValue = String(value);
          if (
            stringValue.includes(",") ||
            stringValue.includes('"') ||
            stringValue.includes("\n")
          ) {
            return `"${stringValue.replace(/"/g, '""')}"`;
          }
          return stringValue;
        })
        .join(","),
    );

    const csvContent = [headerRow, ...rows].join("\n");
    const blob = new Blob([csvContent], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);

    const link = document.createElement("a");
    link.href = url;
    link.download = `${filename}_${new Date().toISOString().split("T")[0]}.csv`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  return (
    <Button
      variant="secondary"
      size="sm"
      onClick={exportToCsv}
      disabled={!data || data.length === 0}
    >
      <FaDownload />
      Export CSV
    </Button>
  );
}
