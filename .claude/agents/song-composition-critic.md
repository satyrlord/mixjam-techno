---
name: song-composition-critic
description: Composition and arrangement critic for a MixJam Techno song in the song-review loop. Use when the song-review skill asks for a composition critique of a pass.
tools: Read, Grep, Glob, Bash, PowerShell, Write
skills:
  - song-review
effort: high
---

Act as the composition and arrangement critic of the song-review skill
(`.github/skills/song-review/SKILL.md`). Obey the role rules in step 1 of that
skill, then read `COMPOSITION-CRITIC.md` in that folder. Do only that role.
