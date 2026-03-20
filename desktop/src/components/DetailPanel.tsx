import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Copy, Play, Square, Wifi } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import type { RepoEntry, ServerState } from "@/types";

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
  const [host, setHost] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Detect reachable host whenever panel mounts or server starts.
  useEffect(() => {
    invoke<string | null>("detect_host").then(setHost).catch(console.error);
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

  const connectHost = host ?? "127.0.0.1";
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

            {!host && (
              <p className="text-xs text-yellow-500 bg-yellow-500/10 border border-yellow-500/20 rounded px-2 py-1.5">
                No reachable LAN or Tailscale IP detected. The phone must be on
                the same network as this machine.
              </p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
