# herdr-sleeper — sleep idle resumable agents in Herdr panes, wake them later

A reference implementation of agent hibernation for Herdr, built entirely on
the public `herdr` CLI (no changes to Herdr itself). It exists to make the
semantics concrete for the Discussion proposing a native `herdr agent sleep`
/ `wake`; until then it is directly usable as-is.

**Problem.** Every idle agent pane keeps a live model-CLI process. On a
16 GB Mac with 14 idle Claude panes that was 2.0 GB RSS and a machine deep
into swap.

**What it does.** After an agent has been idle for a **user-chosen window**
(`idle = "12h"`, `"90m"`, `"1d"` — default 12 h) it submits `/exit`, keeps
the pane (that is the restart slot — `herdr agent start` needs a pane at a
shell prompt), prints a wake hint into the pane and prefixes the pane label
with 💤. `herdr-sleeper wake <name>` relaunches with the original argv plus
`--resume <session id>`; the session id comes from Herdr's own
`agent_session` (reported by the official Claude integration). Verified
round-trip: same session id, prior context intact.

Only Claude is handled today, because the idle clock is the mtime of
Claude's transcript (`~/.claude/projects/*/<id>.jsonl`) — Herdr exposes no
timestamps, only a `state_change_seq` counter. Other agents with session
refs would need their own idle signal.

## Use

```
scripts/herdr-sleeper/herdr-sleeper scan --dry-run      # every check incl. the composer; acts on nothing
scripts/herdr-sleeper/herdr-sleeper scan --idle 6h      # sleep every eligible agent idle ≥ 6 h
scripts/herdr-sleeper/herdr-sleeper list                # what is asleep, with phase
scripts/herdr-sleeper/herdr-sleeper wake <name|pane>    # or --all
scripts/herdr-sleeper/herdr-sleeper log                 # slept / woke / refused / reconciled, newest last
scripts/herdr-sleeper/herdr-sleeper config              # effective settings and their sources
scripts/herdr-sleeper/herdr-sleeper install             # launchd job (macOS); prints a cron line elsewhere
```

Config: `~/.config/herdr-sleeper/config.toml` (`idle` or `idle_hours`,
`exclude`, `interval_minutes`), overridable by `HERDR_SLEEPER_*` env and CLI
flags — lowest to highest precedence. State and logs:
`~/.local/state/herdr-sleeper/` (`sleeping.json` journal, `panes.json`
crash-recovery snapshot, `events.jsonl`, `sleeper.log`, `lock`).

## Safety rules (all fail closed)

Borrowed from Orca's agent-hibernation implementation and its bug history,
then hardened by two rounds of multi-model code review.

1. Eligible only when: agent kind `claude` with a session id and an
   on-disk transcript (newest copy wins); Herdr status `idle`/`done`;
   pane not focused; not in `exclude`; session id not open in another
   pane; transcript older than the window; composer *positively* empty
   (draft, unreadable screen, missing prompt glyph → refuse); original
   argv safely replayable (`--fork-session`, `--print`, `--session-id`,
   positional prompts, anything after `--` → refuse).
2. Everything is re-checked immediately before `/exit`, including that
   `state_change_seq` and the session id have not moved since the scan.
3. The record is written *before* `/exit`. If the agent is still there
   after the wait it stays as `exit-requested`; the next scan reconciles
   (dropped only if the process is really present, `asleep` if it left).
   A record is never deleted because its pane vanished — the manual
   resume command is printed instead. Every scan merges
   pane→session→argv into `panes.json`; `wake` recovers from it and
   persists the recovered entry before trying.
4. Wake refuses if the session id is live in any pane or any real
   `claude` process (`--resume <id>` / `--resume=<id>`, direct or
   node-launched) and refuses when that cannot be verified. Recovered
   argv goes through the same replay filter.
5. A non-object state file, malformed TOML, unknown keys, or a
   `nan`/negative/`inf` window (config, env or CLI) aborts `scan`/`install`
   — `wake`/`list`/`log` keep working. A file lock serialises overlapping
   runs; the journal is re-read under it before every write.
6. `install` builds the plist with `plistlib`, runs the same interpreter,
   and carries `HERDR_SLEEPER_*`/`XDG_CONFIG_HOME` from the installing
   shell; the cron fallback does the same and refuses intervals cron cannot
   express exactly.

Known limits: a window remains between the last recheck and Claude
consuming `/exit` — only a native Herdr operation can close it (below);
Claude only, for the reason above.

## What Herdr would need for this to be native

1. A timestamp on agent state (`state_changed_at`) in `agent.list` /
   `agent.get` / `pane.get`, so idle time does not depend on reading an
   agent's transcript.
2. `herdr agent sleep <target>` / `herdr agent wake <target>` over the
   existing `AgentResumePlan` machinery (`src/agent_resume.rs`) — which
   should also carry the pane's original argv: today `plan()` builds
   `["claude", "--resume", <id>]` from the session ref alone, so flags such
   as `--dangerously-skip-permissions` are lost on restore.
3. Optionally `[session] sleep_idle_after = "12h"` (off by default), with
   the eligibility rules above.

## Tests

`uv run --with pytest python -m pytest scripts/herdr-sleeper -q` — 51 tests
covering the decision logic and the sleep/wake state machine against a
fake `herdr`.
