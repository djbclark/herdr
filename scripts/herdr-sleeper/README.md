# herdr-sleeper — sleep idle resumable agents in Herdr panes, wake them later

A reference implementation of agent hibernation for Herdr, built entirely on
the public `herdr` CLI (no changes to Herdr itself). It exists to make the
semantics concrete for the Discussion proposing a native `herdr agent sleep`
/ `wake`; until then it is directly usable as-is.

**Problem.** Every idle agent pane keeps a live model-CLI process. On a
16 GB Mac with 14 idle Claude panes that was 2.0 GB RSS and a machine deep
into swap.

**What it does.** After an agent has been idle for a configurable window
(default 12 h) it submits `/exit`, keeps the pane (that is the restart slot
— `herdr agent start` needs a pane at a shell prompt), prints a wake hint
into the pane and prefixes the pane label with 💤. `herdr-sleeper wake
<name>` relaunches with the original argv plus `--resume <session id>`;
the session id comes from Herdr's own `agent_session` (reported by the
official Claude integration). Verified round-trip: same session id, prior
context intact.

Only Claude is handled today, because the idle clock is the mtime of
Claude's transcript (`~/.claude/projects/*/<id>.jsonl`) — Herdr exposes no
timestamps, only a `state_change_seq` counter. Other agents with session
refs would need their own idle signal.

## Use

```
scripts/herdr-sleeper/herdr-sleeper scan --dry-run      # what would be slept, and why not
scripts/herdr-sleeper/herdr-sleeper scan                # sleep every eligible agent
scripts/herdr-sleeper/herdr-sleeper list                # what is asleep
scripts/herdr-sleeper/herdr-sleeper wake <name|pane>    # or --all
scripts/herdr-sleeper/herdr-sleeper log                 # slept / woke / refused, newest last
scripts/herdr-sleeper/herdr-sleeper config              # effective settings and their sources
scripts/herdr-sleeper/herdr-sleeper install             # launchd job (macOS); prints a cron line elsewhere
```

Config: `~/.config/herdr-sleeper/config.toml` (`idle_hours`, `exclude`,
`interval_minutes`), overridable by `HERDR_SLEEPER_*` env and CLI flags.
State and logs: `~/.local/state/herdr-sleeper/`.

## Eligibility (all must hold)

Borrowed from Orca's agent-hibernation implementation and its bug history.

1. Agent kind `claude` with a session id and an on-disk transcript.
2. Herdr status `idle` or `done`; pane not focused; not in `exclude`.
3. Session id not open in any other pane.
4. Transcript older than the idle window.
5. Composer holds no draft (`/exit` would destroy it).
6. All of the above re-checked immediately before `/exit` (the scan tick
   is minutes old by then), including that `state_change_seq` has not moved.

On sleep the record is written *before* `/exit` and rolled back if the
process does not exit. On wake it refuses if the session id is live in any
pane or process (fork risk) or the transcript is gone, and never deletes a
record just because its pane vanished — it prints the manual resume command
instead. Every scan also snapshots pane→session id→argv to `panes.json`, so
`wake` still works after a crash that lost the journal.

## What Herdr would need for this to be native

1. A timestamp on agent state (`state_changed_at`) in `agent.list` /
   `agent.get` / `pane.get`, so idle time does not depend on reading an
   agent's transcript.
2. `herdr agent sleep <target>` / `herdr agent wake <target>` over the
   existing `AgentResumePlan` machinery (`src/agent_resume.rs`) — which
   should also carry the pane's original argv: today `plan()` builds
   `["claude", "--resume", <id>]` from the session ref alone, so flags such
   as `--dangerously-skip-permissions` are lost on restore.
3. Optionally `[session] sleep_idle_after = "12h"`.

## Tests

`uv run --with pytest python -m pytest scripts/herdr-sleeper -q`
