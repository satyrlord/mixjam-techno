# MixJam Techno agent skills

These repository skills follow the current owner decisions. If they exist,
`AGENTS.md` and the private notes that
[Toolchain and folders](../../docs/DEVELOPMENT.md#toolchain-and-folders) names
record those decisions. The skills also work without them.
[ARCHITECTURE.md](../../docs/ARCHITECTURE.md) describes the native application
boundary. Use the current repository files to verify commands and behavior.
Historical donor instructions do not apply to this workspace.

Each skill's `description` owns its trigger. The action column copies the
first sentence of that description. If you change a description, change its row.

| Skill | Action |
| --- | --- |
| [album-analysis](album-analysis/SKILL.md) | Add or correct an Album Review: a deep-listening analysis of a released album, or of the owner's own album or song, saved through the agent channel. |
| [create-skill](create-skill/SKILL.md) | Create, change, or audit a repository skill: its triggers, steps, completion criteria, references, and writing. |
| [dead-code-audit](dead-code-audit/SKILL.md) | Audit MixJam Techno reachability and remove proven dead items. |
| [deslop](deslop/SKILL.md) | Remove evidence-backed slop from code, prose, data, configuration, and tests. |
| [explore-test](explore-test/SKILL.md) | Test a feature of the built MixJam Techno application by use, and report its bugs and usability issues. |
| [full-code-review](full-code-review/SKILL.md) | Review a MixJam Techno change set against current code and product contracts. |
| [grill-me](grill-me/SKILL.md) | Challenge a MixJam Techno plan or design before implementation. |
| [improve-codebase-architecture](improve-codebase-architecture/SKILL.md) | Improve MixJam Techno architecture and ownership boundaries, or refactor its code without a change in behavior. |
| [operate-app](operate-app/SKILL.md) | Operate the running MixJam Techno window through its agent control channel. |
| [run-quality-gate](run-quality-gate/SKILL.md) | Run or repair MixJam Techno's documented Rust and native-artifact checks. |
| [song-review](song-review/SKILL.md) | Review, rate, or master a song made with MixJam Techno. |

The writing policy for all skills is in
[deslop/PROSE.md](deslop/PROSE.md#writing-policy-asd-ste100).

Each song-review role also has an agent definition for each harness:
`.claude/agents/` for Claude Code and `.codex/agents/` for Codex. Each
definition is a thin wrapper that points to the song-review skill.
