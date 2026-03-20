# NomadTerm Windows Service (NSSM)

## Requirements
- Windows 10+ (for ConPTY support)
- [NSSM](https://nssm.cc) — Non-Sucking Service Manager

## Install

1. Download `nssm.exe` from https://nssm.cc/download

2. Open PowerShell **as Administrator** and run:
   ```powershell
   nssm install NomadTerm
   ```

3. In the NSSM GUI:
   | Field | Value |
   |-------|-------|
   | Path  | `C:\path\to\nomadterm.exe` |
   | Arguments | `--ws --bind-tailscale --port 7681` |
   | Startup directory | `C:\path\to\NomadTerm\server` |

4. Under the **Environment** tab, add:
   ```
   RUST_LOG=info
   ```

5. Click **Install service**.

## Control

```powershell
nssm start NomadTerm
nssm stop NomadTerm
nssm restart NomadTerm
nssm status NomadTerm
```

## Logs

```powershell
nssm edit NomadTerm   # → I/O tab → set stdout/stderr log files
```

## Notes
- Run as a **non-Administrator user** if possible (NSSM supports `Log on` tab).
- Keep Tailscale running so `--bind-tailscale` can bind to the 100.x.x.x IP.
