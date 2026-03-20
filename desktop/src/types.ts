/** A registered repo entry — mirrors the Rust RepoEntry struct. */
export interface RepoEntry {
  id: string;
  path: string;
  name: string;
  token: string;
  added_at: number;
  last_seen: number | null;
  is_active: boolean;
}

/** Runtime state for a running WS server (tracked in React state). */
export interface ServerState {
  running: boolean;
  port: number | null;
}

export interface ConnectionStrategy {
  kind: "tailscale" | "lan" | "local_only";
  host: string | null;
  remoteCapable: boolean;
  secure: boolean;
}
