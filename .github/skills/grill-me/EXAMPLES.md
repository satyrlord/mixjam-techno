# Decision examples

## Audio contract

Prompt: "Challenge the plan to add a fixed-size render buffer."

First inspect the current renderer and output-device contracts. Then resolve
the buffer, latency, sample rates, offline output, and audio evidence.

## Project data

Prompt: "Challenge the plan to store interface preferences in projects."

Inspect `local-data/research/PRODUCT.md`, if it exists, and the current project
schema. The product notes assign settings to the user's application-data folder.
State the conflict. Ask whether the owner intends to change that product rule.

## UI scope

Prompt: "Challenge the plan for a smaller editor window."

First inspect `docs/DESIGN.md` and the capture sizes in
[UI verification](../../../docs/development/verification.md#ui-verification).
Ask which editing tasks must remain visible and testable at the proposed size.
