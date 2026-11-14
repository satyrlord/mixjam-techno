# Agent control

`gui --agent` lets a local program operate the window. The channel has two parts
on the two sides of the application boundary.

`src/agent.rs` is backend code without GUI dependencies. With its `src/agent/`
modules it defines the typed requests and replies, encodes them as JSON lines,
serves them on the loopback interface, and contains the `agent` command-line
client. It queues each request
for the frontend and wakes it. It does not interpret a request. Each session
has a new key in `agent-PORT.key` in the data folder. The key file uses the
file system's default permissions; the application sets no restrictive access
control list. Any user or program that can read the key can send
requests. A connection that does not start with the key, or that sends a
malformed line, is closed. An HTTP form post without the key does not reach the
queue. A request that the window does not take within 30 seconds fails and can
no longer run.
Dropping the server removes the session key, closes active connections, and
clears its window wake callback. It also wakes the listener and waits for it to
release the port. If the server cannot connect to wake the listener, it does not
wait, so the window can still close.

`src/ui/agent.rs`, with its `src/ui/agent/` modules, is the frontend driver.
`eframe` calls it before each frame.
It reads the controls from the accessibility tree that egui produced for the
previous frame, and it acts through that frame's input: an accessibility action
for a named control, or pointer and keyboard events for a position. A request
replies after the frames that show its result. Arrangement requests call the
same `Session` operations as the lane gestures, because the lanes are one painted
surface without separate controls. The driver holds no project state and
reaches the backend only through `application.rs`; the source guard covers it.

egui does not mark the controls behind a modal as disabled, and an
accessibility action reaches them. Each modal therefore names itself with
`name_dialog`, and the driver refuses a control outside the open modal.
Typed arrangement commands obey the same boundary. Collapsing the dialog
releases those commands. Setting a radio button to true selects it; clearing
the selected button requires choosing another option.

Every file choice is a `FileRequest`. Without an agent, `ask_path` shows the
native dialog and carries the request out at once. With an agent, the request
waits in the Choose path modal, because an agent cannot operate a native dialog
and a blocked window cannot reply. A typed path gets the same result as a
chosen one, including the file extension that a save dialog adds.

A control is usable by an agent, and by a screen reader, only when it has an
accessible name. Give a new control visible label text, `widget_info`, or
`name_control`. Keep each name unique among the controls that can be on screen
together: the lower panel names its tabs `Mixer tab`, `Mastering tab`, and so
on, while the tool rail has a `Mixer` shortcut. Sound-library filters keep their own names.
Add a control that is painted on a shared surface to the
arrangement requests instead.
