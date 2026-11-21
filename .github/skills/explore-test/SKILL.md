---
name: explore-test
description: >
  Test a feature of the built MixJam Techno application by use, and report its
  bugs and usability issues. Use for a hands-on test session of one feature.
---

# Explore-test a feature

An exploratory test session uses the feature to make a real output. The agent
drives the built application through the agent control channel. This skill owns
the test method. The [operate-app skill](../operate-app/SKILL.md) owns the
mechanics of the channel. [REFERENCE.md](REFERENCE.md) owns the note format, the
issue categories, the evidence methods, and the report format. Read a section of
REFERENCE.md when a step links to it.

Do not fix a defect during the session unless the owner asks. Do not commit.

## Ask when in doubt

Ask the owner before you continue if one of these is true:

- The request does not name the subject, or the name fits more than one
  subject.
- The request lacks the output, the size limit, the playback decision, or the
  sound sources, and the code and the docs give no clear default.
- A step of the plan has two or more valid interpretations, and the
  interpretations give different results.
- The code, the docs, and the help text do not show if a behavior is a defect.

Ask all open questions together, in one message. Give a recommended answer
for each question. If a question has a sensible default, and a wrong guess is
easy to correct, use the default. Write the default in the notes and in the
report.

## 1. Define the scope

1. Name the subject: the feature, module, board, tool window, modal, panel, or
   component that the owner wants tested. If the request does not name one
   subject, ask the owner.
   Completion criterion: One subject has one name.
2. Read the code, the pages of `docs/` that name the subject, and the help text
   of the subject.
   Completion criterion: You know each source of the subject.
3. List each function of the subject: each control, key, gesture, mode, file
   format, and output.
   Completion criterion: The list names every function that the three sources
   show.
4. Save the list as the coverage checklist in `tmp/SESSION/`. `SESSION` is a
   short name of the session.
   Completion criterion: The file exists, and each function has one line.

## 2. Plan the task

1. Choose a realistic task that produces a real output. The output is a file, a
   song, a pattern, a sound, or a window state that the owner can inspect.
   Completion criterion: The task has one output and one acceptance check.
2. Make the task use all functions of the checklist. A poor result that uses
   all functions has more value than a good result that uses few.
   Completion criterion: Each function of the checklist is in a step of the plan.
3. Treat a size limit from the owner as a maximum. Do not make the task larger
   than necessary for the checklist.
   Completion criterion: The plan agrees with each limit that the owner gave.
4. If you cannot use a function, write the reason in the checklist now.
   Completion criterion: Each function has a planned step or a reason.
5. If a parameter of the task is missing or unclear, follow the section "Ask
   when in doubt".
   Completion criterion: The plan has no open question, or the owner answered
   each question.

## 3. Prepare the session

1. Build the release executable as `AGENTS.md` states.
   Completion criterion: The executable is newer than the last source change.
2. Ask the owner if the owner wants to listen to the output or to watch the
   window.
   Completion criterion: The owner answered before the window opens.
3. If the owner wants playback, plan the playback steps. If not, plan to verify
   the output by measurement only.
   Completion criterion: The plan states the playback decision.
4. Start an isolated session as the operate-app skill states. Use an empty
   `MIXJAM_TECHNO_DATA_DIR` in `tmp/SESSION/`.
   Completion criterion: `agent wait idle=true` replies with `"ok":true`.

## 4. Run the session

1. Open the subject. Send `ui` and read the names of its controls.
   Completion criterion: You compared the names in `ui` with the checklist and
   with the visible labels.
2. Operate the subject only through named controls (`ui`, `click`, `set`).
   Use `pointer`, `key`, `type`, and `scroll` only where no named control
   exists.
   Completion criterion: Each gesture has a note that states why no named
   control was available.
3. Follow the plan. After each step, read `status` and `state`, and mark the
   function on the checklist.
   Completion criterion: Each used function has a result: works or defect.
4. Write a note during the session, at the moment that you see the item. Do not
   write the notes after the session. Use the note kinds in
   [REFERENCE.md](REFERENCE.md#note-kinds).
   Completion criterion: Each defect, each usability issue, and each verified
   function has a note with the step that caused it.
5. After a defect, check that the application is usable. If it is usable,
   continue the plan. If it is not usable, record the state and restart the
   session.
   Completion criterion: The plan continues, or the restart has a note.
6. If you cannot decide that a behavior is a defect, write a `question` note.
   Then continue the plan.
   Completion criterion: The note exists, and the report lists the question for
   the owner.
7. Test these edges of each function that the plan allows:
   - the limits of a value
   - an empty state
   - undo and redo
   - a close and reopen of the subject

   Completion criterion: Each edge test has a result in the notes.

## 5. Verify the output with evidence

1. Take screenshots into `tmp/SESSION/` at each important state. Do not judge
   by eye alone.
   Completion criterion: Each claim of the report has an evidence file or a
   `state` reply.
2. Measure the output file with a small script, as
   [REFERENCE.md](REFERENCE.md#evidence-methods) states.
   Completion criterion: Each measured claim has a number from a script.
3. Save the output and the evidence in `tmp/SESSION/`.
   Completion criterion: The output file exists and opens.

## 6. End the session

1. End the session as the operate-app skill states.
   Completion criterion: The process has ended.
2. Keep the notes, the output, and the evidence. Remove other temporary files.
   Completion criterion: `tmp/SESSION/` has no file that the report does not use.

## 7. Report

1. Write the report in the format of [REFERENCE.md](REFERENCE.md#report-format).
   Completion criterion: The report has the issue groups in the required
   order, the verified list, and the checklist. Another agent can repeat each
   issue from its steps. The checklist has no function without a result or a
   reason.
2. Keep the report short.
   Completion criterion: The report has no text that does not state an issue,
   a verified function, or a coverage fact.

## Completion criterion

The work is complete when these conditions are true:

- Each function of the checklist has a result or a stated reason.
- Each issue has reproduction steps and evidence.
- You closed the session.
- You delivered the report.
