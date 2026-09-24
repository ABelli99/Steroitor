export interface Span {
  from: number;
  to: number;
}

/** Blocco <<<<<<< … ======= … >>>>>>> (con sezione base opzionale, stile diff3). */
export interface Conflict extends Span {
  ours: Span;
  base: Span | null;
  theirs: Span;
}

export type Choice = "ours" | "theirs" | "ours-theirs" | "theirs-ours";

const isMarker = (line: string, marker: string) => line === marker || line.startsWith(`${marker} `);

interface Open {
  from: number;
  oursFrom: number;
  oursTo?: number;
  baseFrom?: number;
  baseTo?: number;
  theirsFrom?: number;
}

export function parseConflicts(text: string): Conflict[] {
  const conflicts: Conflict[] = [];
  let open: Open | null = null;

  for (let start = 0; start <= text.length; ) {
    const newline = text.indexOf("\n", start);
    const end = newline === -1 ? text.length : newline;
    const next = newline === -1 ? text.length : newline + 1;
    const line = text.slice(start, end).replace(/\r$/, "");

    if (isMarker(line, "<<<<<<<")) {
      open = { from: start, oursFrom: next };
    } else if (open && isMarker(line, "|||||||") && open.theirsFrom === undefined) {
      open.oursTo = start;
      open.baseFrom = next;
    } else if (open && line === "=======" && open.theirsFrom === undefined) {
      if (open.baseFrom !== undefined) open.baseTo = start;
      else open.oursTo = start;
      open.theirsFrom = next;
    } else if (open?.theirsFrom !== undefined && isMarker(line, ">>>>>>>")) {
      conflicts.push({
        from: open.from,
        to: next,
        ours: { from: open.oursFrom, to: open.oursTo! },
        base: open.baseFrom === undefined ? null : { from: open.baseFrom, to: open.baseTo! },
        theirs: { from: open.theirsFrom, to: start },
      });
      open = null;
    }

    if (newline === -1) break;
    start = next;
  }
  return conflicts;
}

export function resolution(text: string, conflict: Conflict, choice: Choice): string {
  const ours = text.slice(conflict.ours.from, conflict.ours.to);
  const theirs = text.slice(conflict.theirs.from, conflict.theirs.to);
  const pieces: Record<Choice, string> = {
    ours,
    theirs,
    "ours-theirs": ours + theirs,
    "theirs-ours": theirs + ours,
  };
  return pieces[choice];
}

/** Risolve tutti i conflitti con la stessa scelta, dal fondo per non spostare gli offset. */
export function resolveAll(text: string, choice: "ours" | "theirs"): string {
  return parseConflicts(text).reduceRight(
    (result, conflict) => result.slice(0, conflict.from) + resolution(result, conflict, choice) + result.slice(conflict.to),
    text,
  );
}
