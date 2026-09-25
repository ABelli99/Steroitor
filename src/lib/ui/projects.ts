export function folderName(folder: string | null) {
  return folder?.split(/[\\/]/).filter(Boolean).at(-1) ?? "Nessuna cartella";
}

export function folderInitial(folder: string | null) {
  return folder ? (folderName(folder).match(/\p{L}|\p{N}/u)?.[0] ?? "?").toUpperCase() : "?";
}
