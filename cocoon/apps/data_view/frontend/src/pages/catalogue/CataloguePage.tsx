import { useEffect, useMemo, useRef, useState } from "react";
import { useSearch, useNavigate } from "@tanstack/react-router";
import cytoscape, {
  type Core,
  type EdgeSingular,
  type LayoutOptions,
} from "cytoscape";
import fcose from "cytoscape-fcose";
import { ZoomIn, ZoomOut, Maximize2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { SCHEMA_STORAGE_KEY } from "@/pages/_shared/prefs";

// Zoom bounds: stop the user from scrolling out into an empty void (or so far in
// that a single node fills the screen). fit() is clamped to these too.
const MIN_ZOOM = 0.2;
const MAX_ZOOM = 3;

// fcose spreads dense clusters and tiles disconnected nodes neatly — far
// better than built-in cose for FK graphs. Register once; fall back to cose if
// registration ever fails (the layout name is chosen accordingly below).
try {
  cytoscape.use(fcose);
} catch {
  // A second registration (HMR re-run of this module) throws but still leaves
  // fcose available, so there is nothing to recover here.
}

// Tuned cose, used only if an fcose layout run ever throws at runtime.
const COSE_FALLBACK: LayoutOptions = {
  name: "cose",
  animate: false,
  padding: 40,
  randomize: true,
  nodeDimensionsIncludeLabels: true,
  idealEdgeLength: 200,
  nodeRepulsion: 30000,
  nodeOverlap: 36,
  componentSpacing: 180,
  gravity: 0.2,
  numIter: 2000,
};

// fcose: spread the dense clusters, give edges room, and tile the many
// disconnected nodes into a tidy block instead of scattering them.
const FCOSE_LAYOUT = {
  name: "fcose",
  animate: false,
  randomize: true,
  padding: 40,
  nodeDimensionsIncludeLabels: true,
  nodeSeparation: 160,
  idealEdgeLength: 130,
  nodeRepulsion: 9000,
  gravity: 0.2,
  gravityRange: 3.0,
  packComponents: true,
  tile: true,
  tilingPaddingVertical: 16,
  tilingPaddingHorizontal: 16,
} as unknown as LayoutOptions;

import DataLoading from "@/components/DataLoad";
import FetchingIndicator from "@/components/FetchingIndicator";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useTheme } from "@/components/themeProvider";
import {
  useDataExplorerSchemas,
  useCatalogue,
  useLinkOverlap,
  type OverlapRequest,
} from "@/pages/_shared/api";
import type { Catalogue, CatalogueEdge } from "@/pages/_shared/types";
import {
  PALETTES,
  buildStylesheet,
  toElements,
  type ThemeName,
} from "./cyGraph";
import { OverlapPanel } from "./OverlapPanel";

export default function CataloguePage() {
  const search = useSearch({ strict: false }) as { schema?: string };
  // Seed from the URL first (a deep link wins), then the remembered choice,
  // then leave empty for the auto-select below.
  const [selectedSchema, setSelectedSchema] = useState<string>(
    () => search.schema ?? localStorage.getItem(SCHEMA_STORAGE_KEY) ?? "",
  );

  // Persist every change so the choice survives navigating away and back.
  useEffect(() => {
    if (selectedSchema) localStorage.setItem(SCHEMA_STORAGE_KEY, selectedSchema);
  }, [selectedSchema]);

  const schemas = useDataExplorerSchemas();
  // Auto-select schema: prefer "stage", else first available. Also corrects a
  // remembered schema that no longer exists in the available set.
  useEffect(() => {
    const list = schemas.data;
    if (list && list.length > 0) {
      if (!selectedSchema || !list.includes(selectedSchema)) {
        setSelectedSchema(list.includes("stage") ? "stage" : list[0]);
      }
    }
  }, [schemas.data, selectedSchema]);

  const catalogue = useCatalogue(selectedSchema);
  const navigate = useNavigate();
  const { theme } = useTheme();
  const palette = useMemo(
    () => PALETTES[theme as ThemeName] ?? PALETTES.light,
    [theme],
  );

  // ── Selection: a tapped edge, and which of its column pairs to validate ──
  const [selectedEdge, setSelectedEdge] = useState<CatalogueEdge | null>(null);
  const [linkIndex, setLinkIndex] = useState(0);

  const overlapRequest = useMemo<OverlapRequest | null>(() => {
    if (!selectedEdge || !selectedSchema) return null;
    const link = selectedEdge.links[linkIndex] ?? selectedEdge.links[0];
    if (!link) return null;
    return {
      schema: selectedSchema,
      sourceTable: selectedEdge.source,
      targetTable: selectedEdge.target,
      sourceColumn: link.source_column,
      targetColumn: link.target_column,
    };
  }, [selectedEdge, selectedSchema, linkIndex]);
  const overlap = useLinkOverlap(overlapRequest);

  // ── Cytoscape lifecycle ──────────────────────────────────────────────
  // Cytoscape is created lazily — only once its container actually has a pixel
  // size. Creating it while the flex layout still reports 0×0 yields a 0-size
  // renderer that never recovers, so the graph stays blank. A ResizeObserver
  // drives both the first creation and every later resize.
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<Core | undefined>(undefined);
  const renderedRef = useRef<Catalogue | null>(null);
  // The latest palette, read inside the (mount-only) ResizeObserver callback.
  const paletteRef = useRef(palette);
  paletteRef.current = palette;

  // The node tap handler is bound once at graph creation, so it must read the
  // current schema and navigate fn through refs — not the stale closure values.
  const selectedSchemaRef = useRef(selectedSchema);
  selectedSchemaRef.current = selectedSchema;
  const navigateRef = useRef(navigate);
  navigateRef.current = navigate;

  const sized = () => {
    const el = containerRef.current;
    return !!el && el.clientWidth > 0 && el.clientHeight > 0;
  };

  const highlight = (edge: EdgeSingular) => {
    const cy = cyRef.current;
    cy?.elements().removeClass("hl");
    edge.addClass("hl");
    edge.connectedNodes().addClass("hl");
  };

  // Re-frame the whole graph (the recovery if you lose the nodes off-screen).
  const resetView = () => {
    const cy = cyRef.current;
    if (!cy || cy.destroyed() || cy.elements().empty()) return;
    cy.animate({ fit: { eles: cy.elements(), padding: 40 } }, { duration: 250 });
  };

  // Zoom about the viewport centre, clamped by the instance min/max zoom.
  const zoomByFactor = (factor: number) => {
    const cy = cyRef.current;
    if (!cy || cy.destroyed()) return;
    cy.animate(
      {
        zoom: {
          level: cy.zoom() * factor,
          renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 },
        },
      },
      { duration: 150 },
    );
  };

  const createCy = () => {
    const cy = cytoscape({
      container: containerRef.current!,
      style: buildStylesheet(paletteRef.current),
      minZoom: MIN_ZOOM,
      maxZoom: MAX_ZOOM,
    });
    cyRef.current = cy;
    cy.on("tap", "edge", (evt) => {
      const edge = evt.target.data("edge") as CatalogueEdge;
      highlight(evt.target);
      setLinkIndex(0);
      setSelectedEdge(edge);
    });
    // Tapping a node opens that table's Table Info page (deep-linked by schema
    // + table, which TableInfoPage reads from the search params).
    cy.on("tap", "node", (evt) => {
      const table = evt.target.id();
      navigateRef.current({
        to: "/table-info",
        search: { schema: selectedSchemaRef.current, table },
      });
    });
    // Pointer affordance: nodes are clickable, the empty canvas is not.
    const container = cy.container();
    if (container) {
      cy.on("mouseover", "node", () => {
        container.style.cursor = "pointer";
      });
      cy.on("mouseout", "node", () => {
        container.style.cursor = "";
      });
    }
    // Tapping empty canvas clears the selection.
    cy.on("tap", (evt) => {
      if (evt.target === cy) {
        cy.elements().removeClass("hl");
        setSelectedEdge(null);
      }
    });
  };

  // Render the current catalogue data into the graph. Safe to call from both
  // the data effect and the ResizeObserver: it no-ops until the container is
  // sized, creates the graph on the first sized call, and only re-lays-out when
  // the data itself changed (a pure resize just keeps the canvas matched).
  const renderGraph = () => {
    if (!sized()) return; // wait for a real size — the ResizeObserver retries
    if (!cyRef.current) createCy();
    const cy = cyRef.current;
    if (!cy || cy.destroyed()) return;
    cy.resize();

    const data = catalogue.data;
    if (!data || data === renderedRef.current) return;
    renderedRef.current = data;

    cy.elements().remove();
    if (data.nodes.length === 0) return; // empty schema: nothing to lay out
    try {
      cy.add(toElements(data));
      cy.resize();
      // A single node has nothing to spread, so just centre it on a grid.
      // Otherwise run fcose, falling back to tuned cose if it throws.
      if (cy.nodes().length > 1) {
        try {
          cy.layout(FCOSE_LAYOUT).run();
        } catch (err) {
          console.error("fcose failed, falling back to cose", err);
          cy.layout(COSE_FALLBACK).run();
        }
      } else {
        cy.layout({ name: "grid", animate: false, padding: 30 }).run();
      }
      cy.fit(undefined, 40);
    } catch (err) {
      console.error("catalogue graph render failed", err);
    }
  };

  // The ResizeObserver is set up once, but `renderGraph` is a fresh closure each
  // render (it reads `catalogue.data`). Point a stable ref at the latest one so
  // the observer never calls a stale closure that still sees `data === undefined`.
  const renderGraphRef = useRef(renderGraph);
  renderGraphRef.current = renderGraph;

  // Set up the ResizeObserver once, and tear the graph down on unmount.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => renderGraphRef.current());
    ro.observe(el);
    return () => {
      ro.disconnect();
      cyRef.current?.destroy();
      cyRef.current = undefined;
      // The cy instance is gone; force the next render to repopulate (without
      // this, StrictMode's mount→unmount→mount leaves a fresh empty cy whose
      // repopulation is blocked by the `data === renderedRef` guard).
      renderedRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Re-render when the catalogue data changes.
  useEffect(() => {
    setSelectedEdge(null);
    renderGraph();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [catalogue.data]);

  // Re-skin the live graph on theme change.
  useEffect(() => {
    cyRef.current?.style(buildStylesheet(palette));
  }, [palette]);

  const stats = useMemo(() => {
    const d = catalogue.data;
    if (!d) return null;
    const declared = d.edges.filter((e) => e.kind === "declared").length;
    return {
      tables: d.nodes.length,
      declared,
      inferred: d.edges.length - declared,
      omitted: d.omitted_columns,
    };
  }, [catalogue.data]);

  const isEmpty =
    !catalogue.isPending &&
    !catalogue.isError &&
    (catalogue.data?.nodes.length ?? 0) === 0;

  // First load = nothing to show yet (no graph data, no error).
  const firstLoad = !catalogue.data && !catalogue.error;
  // A refetch (schema switch) with a graph already on screen: the previous
  // graph stays, so signal the refresh in the toolbar.
  const updating = catalogue.isFetching && !!catalogue.data;

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      {/* Top bar: schema picker, summary counts, legend */}
      <div className="flex items-center gap-4 px-4 py-2 border-b shrink-0">
        <Select value={selectedSchema} onValueChange={setSelectedSchema}>
          <SelectTrigger className="w-44">
            <SelectValue placeholder="Select a schema" />
          </SelectTrigger>
          <SelectContent>
            {(schemas.data ?? []).map((s) => (
              <SelectItem key={s} value={s}>
                {s}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {stats && (
          <div className="flex items-center gap-4 text-sm text-muted-foreground">
            <span>{stats.tables} tables</span>
            <span className="flex items-center gap-1.5">
              <span className="inline-block w-4 border-t-2 border-primary" />
              {stats.declared} declared
            </span>
            <span className="flex items-center gap-1.5">
              <span className="inline-block w-4 border-t-2 border-dashed border-muted-foreground" />
              {stats.inferred} inferred
            </span>
          </div>
        )}

        <div className="ml-auto flex items-center gap-4">
          <FetchingIndicator when={updating} />
          <span className="text-xs text-muted-foreground">
            Click a node to open its table, a link to validate it
          </span>
        </div>
      </div>

      {/* Graph + side panel */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        <div className="relative flex-1 min-h-0 min-w-0">
          {/* Sized by h-full/w-full, NOT `absolute inset-0`: cytoscape forces
              `position: relative` inline on its container, which cancels the
              absolute stretch and collapses the box to 0 height (then the canvas
              renders into nothing). h-full doesn't depend on positioning. */}
          <div ref={containerRef} className="h-full w-full" />

          {/* View controls: recover from getting lost, and step zoom. Shown
              only once there is a graph to act on. */}
          {!firstLoad && !catalogue.isError && !isEmpty && (
            <div className="absolute bottom-3 right-3 flex flex-col gap-1">
              <Button
                variant="outline"
                size="icon"
                onClick={() => zoomByFactor(1.25)}
                aria-label="Zoom in"
                title="Zoom in"
              >
                <ZoomIn className="size-4" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={() => zoomByFactor(0.8)}
                aria-label="Zoom out"
                title="Zoom out"
              >
                <ZoomOut className="size-4" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={resetView}
                aria-label="Reset view"
                title="Reset view (fit all)"
              >
                <Maximize2 className="size-4" />
              </Button>
            </div>
          )}

          {/* Loading / error overlay. Also covers a schema switch (`updating`):
              the old graph would otherwise sit there, stale, through the refetch
              and relayout, which reads as broken. An opaque backdrop hides it
              and the skeleton signals work in progress. */}
          {(firstLoad || updating || catalogue.isError) && (
            <div
              className={cn(
                "absolute inset-0 flex items-center justify-center",
                catalogue.isError ? "bg-background/60" : "bg-background/90",
              )}
            >
              <div className="w-full max-w-md p-4">
                <DataLoading
                  isPending={firstLoad || updating}
                  error={catalogue.error}
                  onRetry={() => catalogue.refetch()}
                  variant="chart"
                />
              </div>
            </div>
          )}

          {/* Empty state */}
          {isEmpty && (
            <div className="absolute inset-0 flex items-center justify-center">
              <p className="text-sm text-muted-foreground">
                No tables found in this schema.
              </p>
            </div>
          )}

          {/* Omitted-columns transparency note */}
          {(stats?.omitted.length ?? 0) > 0 && (
            <div className="absolute bottom-3 left-3 max-w-sm rounded-md border bg-card/90 px-3 py-2 text-xs text-muted-foreground">
              Omitted as too generic to link:{" "}
              {stats!.omitted.map((o, i) => (
                <span key={o.column}>
                  <span className="font-mono text-foreground">{o.column}</span>
                  <span> ({o.table_count} tables)</span>
                  {i < stats!.omitted.length - 1 && <span>, </span>}
                </span>
              ))}
            </div>
          )}
        </div>

        {selectedEdge && (
          <OverlapPanel
            edge={selectedEdge}
            linkIndex={linkIndex}
            onPickLink={setLinkIndex}
            overlap={overlap.data}
            isPending={overlap.isPending}
            error={overlap.error}
            onClose={() => {
              cyRef.current?.elements().removeClass("hl");
              setSelectedEdge(null);
            }}
          />
        )}
      </div>
    </div>
  );
}
