export interface GraphCommit {
  hash: string;
  parents: string[];
}

/** Tratto di mezza riga: da una corsia all'altra, nella metà alta o bassa della riga. */
export interface Segment {
  half: "top" | "bottom";
  from: number;
  to: number;
  color: number;
}

export interface GraphRow {
  lane: number;
  color: number;
  segments: Segment[];
  width: number;
}

const firstFree = (lanes: Array<string | null>) => {
  const free = lanes.indexOf(null);
  return free === -1 ? lanes.length : free;
};

/**
 * Assegna ogni commit a una corsia. Ogni corsia "aspetta" l'hash del prossimo commit che la
 * continua (il primo parent); i parent aggiuntivi dei merge aprono o raggiungono altre corsie.
 */
export function layoutGraph(commits: GraphCommit[]): GraphRow[] {
  const lanes: Array<string | null> = [];
  const colors: number[] = [];
  let nextColor = 0;

  const open = (hash: string) => {
    const lane = firstFree(lanes);
    lanes[lane] = hash;
    colors[lane] = nextColor++;
    return lane;
  };

  return commits.map((commit) => {
    const existing = lanes.indexOf(commit.hash);
    const lane = existing === -1 ? open(commit.hash) : existing;
    const segments: Segment[] = [];

    lanes.forEach((expected, index) => {
      if (expected !== null) segments.push({ half: "top", from: index, to: expected === commit.hash ? lane : index, color: colors[index] });
    });
    lanes.forEach((expected, index) => {
      if (expected === commit.hash && index !== lane) lanes[index] = null;
    });

    const [first, ...others] = commit.parents;
    const fromNode = new Set<number>();
    lanes[lane] = first ?? null;
    if (first) fromNode.add(lane);
    for (const parent of others) {
      const target = lanes.indexOf(parent);
      fromNode.add(target === -1 ? open(parent) : target);
    }

    lanes.forEach((expected, index) => {
      if (expected !== null) segments.push({ half: "bottom", from: fromNode.has(index) ? lane : index, to: index, color: colors[index] });
    });
    while (lanes.length && lanes[lanes.length - 1] === null) lanes.pop();

    const width = Math.max(lane, ...segments.flatMap((segment) => [segment.from, segment.to])) + 1;
    return { lane, color: colors[lane], segments, width };
  });
}
