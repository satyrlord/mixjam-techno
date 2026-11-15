# Agent control

`gui --agent` lets a local program operate the window. The channel has two
parts, one on each side of the application boundary.

## The server

`src/agent.rs` is backend code without GUI dependencies. With its
`src/agent/` modules, it does these tasks:

- It defines the typed requests and replies.
- It encodes them as JSON lines.
- It serves them on the loopback interface.
- It contains the `agent` command-line client.

The server queues each request for the frontend and wakes the frontend. It
does not interpret a request.

Each session has a new key in `agent-PORT.key` in the data folder. The key file
uses the default permissions of the file system. The application sets no
restrictive access control list. Thus any user or program that can read the key
can send requests. The server closes a connection that does not start with the
key, or that sends a malformed line. An HTTP form post without the key does not
get to the queue. If the window does not take a request within 30 seconds, the
request fails and cannot run later.

When the server is dropped, it removes the session key, closes the active
connections, and clears its window wake callback. It also wakes the listener and
waits until the listener releases the port. If the server cannot connect to
wake the listener, it does not wait. Thus the window can still close.

## The frontend driver

`src/ui/agent.rs`, with its `src/ui/agent/` modules, is the frontend driver.
`eframe` calls it before each frame. It reads the controls from the
accessibility tree that egui made for the previous frame. It acts through the
input of that frame:

- For a named control, it sends an accessibility action.
- For a position, it sends pointer and keyboard events.

An accessibility `Click` does not move the keyboard focus, and a pointer click
does. Thus the driver runs `Step::Defocus` before it: it takes the focus from
the focused control, unless that control is the target. Without it, the keys
that follow a `click` go to the control that was focused earlier. For example,
the routing of Ctrl+Z between the Step synth board and the song follows the
focus. `state.focus` and `ui.focus` name the focused control, so that an agent
can check the focus without a screenshot.

A request replies after the frames that show its result. Arrangement requests
call the same `Session` operations as the lane gestures, because the lanes are
one painted surface without separate controls. The driver holds no project
state. It gets to the backend only through `application.rs`, and the source
guard covers it.

## Modals and file choices

egui does not mark the controls behind a modal as disabled, and an
accessibility action gets to them. Thus each modal names itself with
`name_dialog`, and the driver refuses a control outside the open modal. Typed
arrangement commands obey the same boundary. When the dialog collapses, it
releases those commands. To set a radio button to true selects it. To clear the
selected button, choose another option.

Each file choice is a `FileRequest`. Without an agent, `ask_path` shows the
native dialog and does the request immediately. With an agent, the request
waits in the Choose path modal. An agent cannot operate a native dialog, and a
blocked window cannot reply. A typed path gets the same result as a chosen
path, with the file extension that a save dialog adds.

## Control names

An agent or a screen reader can use a control only when the control has an
accessible name. Give a new control visible label text, `widget_info`, or
`name_control`. Each name must be unique among the controls that can be on
screen together:

- The `Mixer` button of the tool rail is the only control with that name.
- The controls of each board carry the name of the board.
- The filters of the sound library keep their own names.

If a control is painted on a shared surface, add it to the arrangement requests
instead.

egui makes the surface of an open menu, context menu, or drop-down list a Tab
stop, and gives it no name. `name_open_menus` names that surface `Menu` at the
end of each frame. A menu that opens from another menu gets the name `Menu 2`.
