import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Play, Square, Wifi } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { ConnectionStrategy, RepoEntry, ServerState } from "@/types";

interface DetailPanelProps {
  repo: RepoEntry | null;
  serverState: ServerState;
  repoIndex: number;
  onServerStateChange: (id: string, state: ServerState) => void;
}

export function DetailPanel({
  repo,
  serverState,
  repoIndex,
  onServerStateChange,
}: DetailPanelProps) {
  const [strategy, setStrategy] = useState<ConnectionStrategy | null>(null);
  const [copied, setCopied] = useState(false);

  // Detect the best connection path whenever panel mounts or server starts.
  useEffect(() => {
    invoke<ConnectionStrategy>("detect_connection_strategy")
      .then(setStrategy)
      .catch(console.error);
  }, [serverState.running]);

  if (!repo) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground text-sm">
        Select a repo from the sidebar.
      </div>
    );
  }

  const port = serverState.running
    ? (serverState.port ?? 7682 + repoIndex)
    : 7682 + repoIndex;

  const connectHost = strategy?.host ?? "127.0.0.1";
  const repoNameEnc = encodeURIComponent(repo.name);
  const uri = serverState.running
    ? `nomadterm://${connectHost}:${port}?token=${repo.token}&repo_id=${repo.id}&repo_name=${repoNameEnc}&tls=0`
    : null;

  async function handleToggle() {
    if (!repo) return;
    if (serverState.running) {
      await invoke("stop_server", { repoId: repo.id });
      onServerStateChange(repo.id, { running: false, port: null });
    } else {
      const assignedPort = 7682 + repoIndex;
      await invoke("start_server", {
        repoId: repo.id,
        repoToken: repo.token,
        repoPath: repo.path,
        port: assignedPort,
      });
      onServerStateChange(repo.id, { running: true, port: assignedPort });
    }
  }

  async function handleCopy() {
    if (!uri) return;
    await navigator.clipboard.writeText(uri);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  const strategyBadge =
    strategy?.kind === "tailscale"
      ? { label: "Tailscale", variant: "success" as const }
      : strategy?.kind === "lan"
        ? { label: "LAN only", variant: "warning" as const }
        : { label: "Local only", variant: "outline" as const };

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      {/* Repo header */}
      <div>
        <h2 className="text-xl font-semibold">{repo.name}</h2>
        <p className="text-sm text-muted-foreground mt-0.5 break-all">
          {repo.path}
        </p>
      </div>

      <Separator />

      {/* Status + controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <Badge variant={serverState.running ? "success" : "outline"}>
          {serverState.running ? (
            <>
              <span className="h-1.5 w-1.5 rounded-full bg-green-400 mr-1.5 animate-pulse" />
              Running
            </>
          ) : (
            "Stopped"
          )}
        </Badge>
        <Badge variant={strategyBadge.variant}>{strategyBadge.label}</Badge>
        {serverState.running && (
          <span className="text-sm text-muted-foreground">
            <Wifi className="inline h-3.5 w-3.5 mr-1" />
            Port {port}
          </span>
        )}
        <Button
          onClick={handleToggle}
          variant={serverState.running ? "destructive" : "default"}
          size="sm"
          className="ml-auto"
        >
          {serverState.running ? (
            <>
              <Square className="h-3.5 w-3.5 mr-1.5" />
              Stop
            </>
          ) : (
            <>
              <Play className="h-3.5 w-3.5 mr-1.5" />
              Start
            </>
          )}
        </Button>
      </div>

      {/* QR + URL (only when running) */}
      {serverState.running && uri && (
        <>
          <Separator />
          <div className="space-y-4">
            <p className="text-sm font-medium">Scan with the NomadTerm mobile app</p>

            {/* QR code */}
            <div className="inline-block p-3 bg-white rounded-lg">
              <QRCodeSVG value={uri} size={240} />
            </div>

            {/* URL display */}
            <div className="flex items-start gap-2">
              <code className="flex-1 text-xs bg-muted rounded px-2 py-1.5 break-all text-muted-foreground">
                {uri}
              </code>
              <Button
                variant="outline"
                size="sm"
                onClick={handleCopy}
                className="shrink-0"
              >
                <Copy className="h-3.5 w-3.5 mr-1.5" />
                {copied ? "Copied!" : "Copy"}
              </Button>
            </div>

            {strategy?.kind === "tailscale" && (
              <p className="text-xs text-green-400 bg-green-500/10 border border-green-500/20 rounded px-2 py-1.5">
                Remote access is ready through Tailscale. This QR should work
                from outside your home as long as Tailscale is active on your
                phone too.
              </p>
            )}

            {strategy?.kind === "lan" && (
              <p className="text-xs text-yellow-500 bg-yellow-500/10 border border-yellow-500/20 rounded px-2 py-1.5">
                Tailscale was not detected, so this connection is LAN-only. For
                coffee-shop or mobile-data access, install and enable Tailscale
                on both this machine and your phone.
              </p>
            )}

            {strategy?.kind === "local_only" && (
              <p className="text-xs text-yellow-500 bg-yellow-500/10 border border-yellow-500/20 rounded px-2 py-1.5">
                No reachable LAN or Tailscale IP was detected. NomadTerm fell
                back to local-only mode on this machine.
              </p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
