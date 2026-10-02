export const LATEST_RELEASE_API = "https://api.github.com/repos/ABelli99/Steroitor/releases/latest";

export interface Update {
  version: string;
  url: string;
}

interface Release {
  tag_name?: unknown;
  html_url?: unknown;
}

type Version = [number, number, number];

export function parseVersion(text: string): Version | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(text.trim());
  return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
}

const isNewer = (candidate: Version, current: Version) => {
  const difference = candidate.map((part, index) => part - current[index]).find((delta) => delta !== 0);
  return (difference ?? 0) > 0;
};

/** La release, se è più nuova di `current`. Bozze e pre-release sono già escluse da GitHub. */
export function newerRelease(current: string, release: Release): Update | null {
  if (typeof release.tag_name !== "string" || typeof release.html_url !== "string") return null;
  const candidate = parseVersion(release.tag_name);
  const installed = parseVersion(current);
  if (!candidate || !installed || !isNewer(candidate, installed)) return null;
  return { version: candidate.join("."), url: release.html_url };
}
