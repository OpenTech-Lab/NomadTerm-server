import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal as TerminalIcon, LayoutDashboard } from "lucide-react";
import { cn } from "@/lib/utils";
import { RepoPanel } from "@/components/RepoPanel";
import { DetailPanel } from "@/components/DetailPanel";
import { Terminal } from "@/components/Terminal";
import type { RepoEntry, ServerState } from "@/types";

type ActiveTab = "dashboard" | "terminal";

export default function App() {
  const [repos, setRepos] = useState<RepoEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [serverStates, setServerStates] = useState<
    Record<string, ServerState>
  >({});
  const [activeTab, setActiveTab] = useState<ActiveTab>("dashboard");

  // Load repos on mount.
  useEffect(() => {
    invoke<RepoEntry[]>("list_repos")
      .then((r) => {
        setRepos(r);
        if (r.length > 0) setSelectedId(r[0].id);
      })
      .catch(console.error);
  }, []);

  // Listen to Tauri backend events for server lifecycle.
  useEffect(() => {
    const unlisten1 = listen<string>("server-started", (ev) => {
      const id = ev.payload;
      setServerStates((prev) => ({
        ...prev,
        [id]: { running: true, port: prev[id]?.port ?? null },
      }));
    });

    const unlisten2 = listen<string>("server-stopped", (ev) => {
      const id = ev.payload;
      setServerStates((prev) => ({
        ...prev,
        [id]: { running: false, port: null },
      }));
    });

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
    };
  }, []);

  function updateServerState(id: string, state: ServerState) {
    setServerStates((prev) => ({ ...prev, [id]: state }));
  }

  const selectedRepo = repos.find((r) => r.id === selectedId) ?? null;
  const selectedState: ServerState = selectedId
    ? (serverStates[selectedId] ?? { running: false, port: null })
    : { running: false, port: null };
  const selectedIndex = repos.findIndex((r) => r.id === selectedId);

  return (
    <div className="flex h-screen overflow-hidden bg-background text-foreground">
      {/* Left sidebar */}
      <RepoPanel
        repos={repos}
        selectedId={selectedId}
        serverStates={serverStates}
        onSelect={(id) => setSelectedId(id || null)}
        onReposChange={setRepos}
      />

      {/* Main area */}
      <div className="flex flex-col flex-1 min-w-0">
        {/* Tab bar */}
        <div className="flex items-center border-b border-border px-4 gap-1 shrink-0 h-10">
          <TabButton
            active={activeTab === "dashboard"}
            onClick={() => setActiveTab("dashboard")}
          >
            <LayoutDashboard className="h-3.5 w-3.5 mr-1.5" />
            Dashboard
          </TabButton>
          <TabButton
            active={activeTab === "terminal"}
            onClick={() => setActiveTab("terminal")}
          >
            <TerminalIcon className="h-3.5 w-3.5 mr-1.5" />
            Terminal
          </TabButton>
        </div>

        {/* Tab content */}
        <div className="flex-1 overflow-hidden flex">
          {activeTab === "dashboard" ? (
            <DetailPanel
              repo={selectedRepo}
              serverState={selectedState}
              repoIndex={Math.max(selectedIndex, 0)}
              onServerStateChange={updateServerState}
            />
          ) : (
            <Terminal repo={selectedRepo} serverState={selectedState} />
          )}
        </div>
      </div>
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "inline-flex items-center text-xs font-medium px-3 h-full border-b-2 transition-colors",
        active
          ? "border-primary text-foreground"
          : "border-transparent text-muted-foreground hover:text-foreground"
      )}
    >
      {children}
    </button>
  );
}
