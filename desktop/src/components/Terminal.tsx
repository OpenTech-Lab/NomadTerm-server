import { useEffect, useRef } from "react";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import type { RepoEntry, ServerState } from "@/types";

interface TerminalProps {
  repo: RepoEntry | null;
  serverState: ServerState;
}

/**
 * Embeds an xterm.js terminal connected via WebSocket to the running
 * nomadterm WS server for the selected repo.
 *
 * When no server is running, shows a placeholder message.
 */
export function Terminal({ repo, serverState }: TerminalProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Initialise xterm once.
  useEffect(() => {
    if (!containerRef.current) return;

    const term = new XTerm({
      theme: {
        background: "#0a0a0a",
        foreground: "#e5e7eb",
        cursor: "#4ade80",
        selectionBackground: "#4ade8033",
      },
      fontFamily: '"JetBrains Mono", "Fira Code", monospace',
      fontSize: 13,
      cursorBlink: true,
      allowTransparency: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());

    term.open(containerRef.current);
    fitAddon.fit();

    termRef.current = term;
    fitRef.current = fitAddon;

    // Resize observer keeps the terminal sized to its container.
    const ro = new ResizeObserver(() => fitAddon.fit());
    ro.observe(containerRef.current);

    return () => {
      ro.disconnect();
      term.dispose();
      termRef.current = null;
    };
  }, []);

  // Connect/disconnect WebSocket when server starts/stops.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;

    // Close any existing WS connection.
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    if (!serverState.running || !repo || !serverState.port) {
      term.clear();
      term.writeln(
        "\x1b[90mStart the server to connect the terminal.\x1b[0m"
      );
      return;
    }

    const port = serverState.port;
    const token = repo.token;
    const url = `ws://127.0.0.1:${port}/ws?token=${encodeURIComponent(token)}`;

    const ws = new WebSocket(url);
    ws.binaryType = "arraybuffer";
    wsRef.current = ws;

    term.clear();
    term.writeln(`\x1b[90mConnecting to ws://127.0.0.1:${port}/ws …\x1b[0m`);

    ws.onopen = () => {
      term.writeln("\x1b[32mConnected.\x1b[0m\r\n");
    };

    ws.onmessage = (ev) => {
      if (ev.data instanceof ArrayBuffer) {
        // Raw PTY bytes → write directly.
        term.write(new Uint8Array(ev.data));
      } else if (typeof ev.data === "string") {
        // JSON control frame — ignore in terminal view.
      }
    };

    ws.onerror = () => {
      term.writeln("\r\n\x1b[31mWebSocket error.\x1b[0m");
    };

    ws.onclose = () => {
      term.writeln("\r\n\x1b[90mDisconnected.\x1b[0m");
    };

    // Forward user keystrokes to the server as JSON Input messages.
    const dataHandler = term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        // The server doesn't yet route un-session-ID'd input, so we wrap it
        // in a JSON Input message. The first/only session is targeted.
        ws.send(
          JSON.stringify({
            type: "input",
            session_id: "__any__",
            data,
          })
        );
      }
    });

    return () => {
      dataHandler.dispose();
      ws.close();
      wsRef.current = null;
    };
  }, [serverState.running, serverState.port, repo]);

  return (
    <div
      ref={containerRef}
      className="flex-1 w-full h-full overflow-hidden bg-[#0a0a0a]"
      style={{ padding: "4px" }}
    />
  );
}
