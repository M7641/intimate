/// <reference types="vite/client" />

// cytoscape-fcose ships no type declarations; we register it as a Cytoscape
// layout extension at runtime (see CataloguePage).
declare module "cytoscape-fcose";
