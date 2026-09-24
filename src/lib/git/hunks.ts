import { Chunk } from "@codemirror/merge";
import { Text, type ChangeSpec } from "@codemirror/state";

const DIFF_CONFIG = { timeout: 500 };

/** I `to` dei chunk possono puntare un carattere oltre la fine del documento. */
const clamp = (position: number, doc: Text) => Math.min(position, doc.length);

export const toText = (content: string) => Text.of(content.split(/\r\n|\r|\n/));

export function firstLine(chunk: Chunk, buffer: Text) {
  return buffer.lineAt(clamp(chunk.fromB, buffer)).number;
}

export function lastLine(chunk: Chunk, buffer: Text) {
  return chunk.fromB === chunk.toB ? firstLine(chunk, buffer) : buffer.lineAt(chunk.endB).number;
}

/** Chunk che toccano le righe [from, to] del buffer (numeri di riga 1-based). */
export function chunksInLines(chunks: readonly Chunk[], buffer: Text, from: number, to: number): Chunk[] {
  return chunks.filter((chunk) => firstLine(chunk, buffer) <= to && lastLine(chunk, buffer) >= from);
}

/** `index` con applicati solo i chunk scelti (chunk calcolati tra index e buffer). */
export function applyChunks(index: Text, buffer: Text, chunks: readonly Chunk[]): string {
  let result = "";
  let position = 0;
  for (const chunk of chunks) {
    result += index.sliceString(position, clamp(chunk.fromA, index));
    result += buffer.sliceString(clamp(chunk.fromB, buffer), clamp(chunk.toB, buffer));
    position = clamp(chunk.toA, index);
  }
  return result + index.sliceString(position);
}

/** Contenuto da mettere in stage: index + le modifiche del buffer nelle righe [from, to]. */
export function stagedContent(index: Text, buffer: Text, from: number, to: number): { content: string; chunks: number } {
  const selected = chunksInLines(Chunk.build(index, buffer, DIFF_CONFIG), buffer, from, to);
  return { content: applyChunks(index, buffer, selected), chunks: selected.length };
}

/** Modifica che riporta un chunk (calcolato tra HEAD e buffer) al contenuto di HEAD. */
export function rollbackChange(head: Text, buffer: Text, chunk: Chunk): ChangeSpec {
  return {
    from: clamp(chunk.fromB, buffer),
    to: clamp(chunk.toB, buffer),
    insert: head.sliceString(clamp(chunk.fromA, head), clamp(chunk.toA, head)),
  };
}
