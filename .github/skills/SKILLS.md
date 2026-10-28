# MixJam Techno agent skills

These repository skills follow the current owner decisions. If they exist,
`AGENTS.md` and the private notes `local-data/research/PRODUCT.md` and
`local-data/research/COMPATIBILITY.md` record them; the skills work without
them. `docs/ARCHITECTURE.md` describes the native application boundary. Use current repository files to verify commands and
behavior. Historical donor instructions do not apply to this workspace.

| Skill | Primary action |
| --- | --- |
| [create-skill](create-skill/SKILL.md) | Create or revise a repository skill. |
| [dead-code-audit](dead-code-audit/SKILL.md) | Prove reachability and remove proven dead items. |
| [deslop](deslop/SKILL.md) | Remove evidence-backed problems in code, tests, data, or prose. |
| [full-code-review](full-code-review/SKILL.md) | Review changes and repair confirmed findings when requested. |
| [grill-me](grill-me/SKILL.md) | Resolve open product or design decisions before implementation. |
| [improve-codebase-architecture](improve-codebase-architecture/SKILL.md) | Improve application ownership boundaries. |
| [operate-app](operate-app/SKILL.md) | Operate the running application through its agent control channel. |
| [refactor](refactor/SKILL.md) | Improve structure while preserving behavior. |
| [run-quality-gate](run-quality-gate/SKILL.md) | Run or repair documented Rust and native checks. |
| [song-review](song-review/SKILL.md) | Review, rate, or master a song made with the application. |

The song-review roles also have agent definitions for each harness:
`.claude/agents/` for Claude Code and `.codex/agents/` for Codex. Each one is a
thin wrapper that points to the skill.
