// Single-page app: the Axum binary serves one static build and every
// route falls back to index.html (adapter-static `fallback`, cf.
// docs/technical-specs.md §3). No server-side rendering or prerendering.
export const ssr = false;
export const prerender = false;
