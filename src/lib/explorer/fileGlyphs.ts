export interface GlyphPart {
  d: string;
  fill?: boolean;
  /** Colore fisso della parte; altrimenti quello dell'icona. */
  color?: string;
  transform?: string;
}

const frame = "M2.5 2.5h11a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z";
const ellipse = "M2 8a6 2.4 0 1 0 12 0a6 2.4 0 1 0-12 0";
const pythonHalf = "M8 1.5c-2.5 0-3 1-3 2v1.5h3.2v.6H3.6c-1.3 0-2.1 1-2.1 2.9s.8 2.9 2.1 2.9H5V9.6c0-1.1.9-2 2-2h3c.9 0 1.5-.7 1.5-1.5V3.5c0-1-1-2-3.5-2z";

const definitions = {
  code: [{ d: "M5.5 4.5L2 8l3.5 3.5M10.5 4.5L14 8l-3.5 3.5M9 3l-2 10" }],
  braces: [{ d: "M6 2.5H5A1.5 1.5 0 0 0 3.5 4v2.5L2 8l1.5 1.5V12A1.5 1.5 0 0 0 5 13.5h1M10 2.5h1A1.5 1.5 0 0 1 12.5 4v2.5L14 8l-1.5 1.5V12a1.5 1.5 0 0 1-1.5 1.5h-1" }],
  atom: [
    { d: ellipse },
    { d: ellipse, transform: "rotate(60 8 8)" },
    { d: ellipse, transform: "rotate(120 8 8)" },
    { d: "M8 7a1 1 0 1 0 0 2a1 1 0 1 0 0-2z", fill: true },
  ],
  python: [
    { d: pythonHalf, fill: true, color: "#3572a5" },
    { d: pythonHalf, fill: true, color: "#e8c547", transform: "rotate(180 8 8)" },
  ],
  cup: [{ d: "M3 7h8v3.5a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3zM11 8h1a1.5 1.5 0 0 1 0 3h-1M6 2c-.8 1 .8 2 0 3M8.5 2c-.8 1 .8 2 0 3" }],
  kotlin: [{ d: "M2.5 2.5h11L8 8l5.5 5.5h-11z", fill: true }],
  gear: [{ d: "M8 4a4 4 0 1 0 0 8a4 4 0 1 0 0-8zM8 6.5a1.5 1.5 0 1 0 0 3a1.5 1.5 0 1 0 0-3zM12 8h2M2 8h2M8 2v2M8 12v2M10.8 5.2l1.4-1.4M3.8 3.8l1.4 1.4M3.8 12.2l1.4-1.4M10.8 10.8l1.4 1.4" }],
  hexagon: [{ d: "M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5zM10 6.3a2.5 2.5 0 1 0 0 3.4" }],
  shield: [{ d: "M2.5 1.5h11l-1 11L8 14.5l-4.5-2zM10.5 4.5h-5l.3 3h4l-.3 3-1.5.6-1.5-.6-.1-1" }],
  brackets: [{ d: "M5.5 4L1.5 8l4 4M10.5 4l4 4-4 4" }],
  bezier: [
    { d: "M3 12.5C5 3 11 13 13 3.5" },
    { d: "M1.5 11h3v3h-3zM11.5 2h3v3h-3z", fill: true },
  ],
  hash: [{ d: "M6.5 2.5l-1.5 11M11.5 2.5l-1.5 11M2.5 6h11M2 10.5h11" }],
  list: [{ d: "M2.5 3.5h5M4.5 6.5h9M4.5 9.5h7M2.5 12.5h5" }],
  sliders: [
    { d: "M2.5 4.5h11M2.5 8h11M2.5 11.5h11" },
    { d: "M5 2.9a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2zM10 6.4a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2zM7 9.9a1.6 1.6 0 1 0 0 3.2a1.6 1.6 0 1 0 0-3.2z", fill: true },
  ],
  markdown: [{ d: "M2.5 3.5h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1zM3.8 10V6l1.7 2 1.7-2v4M11 6v4M9.5 8.5L11 10l1.5-1.5" }],
  database: [{ d: "M3 4c0-1.1 2.2-2 5-2s5 .9 5 2-2.2 2-5 2-5-.9-5-2zM3 4v8c0 1.1 2.2 2 5 2s5-.9 5-2V4M3 8c0 1.1 2.2 2 5 2s5-.9 5-2" }],
  terminal: [{ d: `${frame}M4.5 6l2 2-2 2M8 10.5h3.5` }],
  image: [
    { d: `${frame}M1.5 11.5l4-4 3 3 2-2 4 4` },
    { d: "M11 4.3a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0-2.4z", fill: true },
  ],
  lock: [{ d: "M4.5 7.5h7a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-5a1 1 0 0 1 1-1zM5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5M8 10v2" }],
  git: [
    { d: "M8 1.2l6.8 6.8L8 14.8 1.2 8z", fill: true },
    { d: "M8 4.8v6.4M8 6.6l2.2 2.2", color: "#fff" },
  ],
  docker: [
    { d: "M1.5 8.5h12.5c-.5 3-3 5-7 5-3 0-5-2-5.5-5z" },
    { d: "M3.5 5.5h2v2h-2zM6.25 5.5h2v2h-2zM9 5.5h2v2H9zM6.25 2.75h2v2h-2z", fill: true },
  ],
  vue: [
    { d: "M.8 2.5h3.1L8 9.6l4.1-7.1h3.1L8 14.5z", fill: true },
    { d: "M4.4 2.5h2L8 5.3l1.6-2.8h2L8 8.7z", fill: true, color: "#35495e" },
  ],
  svelte: [{ d: "M11 3.5c-1.5-1.5-4-1.5-5.5 0S4 7 6 8s4.5 1.5 4.5 3.5-2.5 2.5-4.5 1.5" }],
  gem: [{ d: "M4.5 2.5h7l3 3.5L8 14 1.5 6zM1.5 6h13M6 2.5L5 6l3 8 3-8-1-3.5" }],
  text: [{ d: "M3.5 1.5h6l3 3v10h-9zM9.5 1.5v3h3M5.5 7.5h5M5.5 10h5M5.5 12.5h3" }],
  file: [{ d: "M3.5 2.5h6l3 3v8h-9zM9.5 2.5v3h3" }],
} satisfies Record<string, GlyphPart[]>;

export type GlyphName = keyof typeof definitions;

export const glyphs: Record<GlyphName, GlyphPart[]> = definitions;
