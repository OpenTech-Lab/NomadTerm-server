import { FolderOpen, Plus, Trash2 } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import type { RepoEntry, ServerState } from "@/types";

interface RepoPanelProps {
  repos: RepoEntry[];
  selectedId: string | null;
  serverStates: Record<string, ServerState>;
  onSelect: (id: string) => void;
  onReposChange: (repos: RepoEntry[]) => void;
}

export function RepoPanel({
  repos,
  selectedId,
  serverStates,
  onSelect,
  onReposChange,
}: RepoPanelProps) {
  async function handleAdd() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || typeof selected !== "string") return;
    try {
      const entry: RepoEntry = await invoke("add_repo", { path: selected });
      onReposChange([...repos, entry]);
      onSelect(entry.id);
    } catch (e) {
      console.error("add_repo failed:", e);
    }
  }

  async function handleRemove() {
    if (!selectedId) return;
    try {
      await invoke("remove_repo", { id: selectedId });
      const updated = repos.filter((r) => r.id !== selectedId);
      onReposChange(updated);
      onSelect(updated[0]?.id ?? "");
    } catch (e) {
      console.error("remove_repo failed:", e);
    }
  }

  return (
    <aside className="flex flex-col w-52 shrink-0 border-r border-border bg-card">
      {/* Header */}
      <div className="px-4 py-3 flex items-center justify-between">
        <span className="text-xs font-semibold uppercase tracking-widest text-muted-foreground">
          Repos
        </span>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={handleAdd}
          title="Add repo"
        >
          <Plus className="h-4 w-4" />
        </Button>
      </div>

      <Separator />

      {/* Repo list */}
      <ScrollArea className="flex-1 py-1">
        {repos.length === 0 && (
          <p className="px-4 py-6 text-xs text-muted-foreground text-center">
            No repos yet.
            <br />
            Click + to add one.
          </p>
        )}
        {repos.map((repo) => {
          const running = serverStates[repo.id]?.running ?? false;
          const isSelected = selectedId === repo.id;
          return (
            <button
              key={repo.id}
              onClick={() => onSelect(repo.id)}
              className={cn(
                "w-full flex items-center gap-2 px-3 py-2 text-sm text-left transition-colors",
                isSelected
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-accent/50 text-foreground"
              )}
            >
              {/* Running indicator dot */}
              <span
                className={cn(
                  "h-2 w-2 rounded-full shrink-0",
                  running ? "bg-green-400" : "bg-muted-foreground/40"
                )}
              />
              <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
              <span className="truncate">{repo.name}</span>
            </button>
          );
        })}
      </ScrollArea>

      <Separator />

      {/* Remove button */}
      <div className="p-2">
        <Button
          variant="ghost"
          size="sm"
          className="w-full text-destructive hover:text-destructive hover:bg-destructive/10"
          disabled={!selectedId}
          onClick={handleRemove}
        >
          <Trash2 className="h-3.5 w-3.5 mr-1.5" />
          Remove
        </Button>
      </div>
    </aside>
  );
}
