# Prose slop

Keep the true meaning and any required distinct voice. Remove prose that hides
the point or that conflicts with its owner.

## Writing policy: ASD-STE100

This section owns the writing policy for all documentation, skills, and agent
tools in this repository. Other files point here.

- Use strict ASD-STE100 Simplified Technical English (STE) for procedures,
  instructions, skills, agent definitions, safety text, and error messages.
- Apply the STE rules to descriptive technical documentation too.
- Do not apply STE to code, identifiers, command syntax, quoted owner words, or
  a required distinct voice. If a branch needs a distinct voice, the skill must
  name that branch and its boundary.

The official standard has copyright protection. Do not copy it into the
repository. If a source is necessary, link to the
[official ASD-STE100 site](https://asd-ste100.org).

## STE checks

Use these checks after the evidence pass. Form alone does not prove that a
claim is true.

- Use one word for one meaning. Use the term that the code uses.
- Use simple, common words. Technical names and technical verbs are permitted.
- Do not use phrasal verbs, contractions, idioms, or Latin abbreviations.
- For an instruction or a rule, use the imperative, "must", or "do not". Do not
  use "should", "may", or "might". Use "can" only for ability or possibility.
- Use simple tenses and the active voice. Avoid "-ing" verb forms.
- Write one topic in each sentence and one action in each procedural step.
- Keep procedural sentences to 20 words or fewer. Keep descriptive sentences
  to 25 words or fewer. A code span counts as one word.
- Put a condition before its instruction. Do not join clauses with a semicolon.
- Use noun clusters of three words or fewer.
- Use a vertical list for a sequence or for three or more parallel items.

Do not delete a needed fact to make a text shorter. STE changes the form, not
the content.

## Empty framing

Inspect these candidates:

- an introduction that delays the actual point
- a rhetorical question or fragment used only for emphasis
- a vague declaration with no concrete information
- text about what the document will say
- a transition that the heading already supplies

## Inflated style

Inspect these candidates:

- business or trend words that replace a specific claim
- repeated contrasts that restate one conclusion
- absolute claims without evidence
- repeated sentence patterns that imply false equivalence
- modifiers or passive clauses that hide the actor or claim

## Stale information

Inspect these candidates:

- historical notes with no current constraint
- TODO items with no owner, condition, or work item
- descriptions of removed files, commands, or behavior
- summaries that omit evidence needed for action

If history explains a current constraint or prevents a rejected decision, keep
it. If necessary, move that fact to its durable owner.
