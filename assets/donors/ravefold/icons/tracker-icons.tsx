export function TrackerIcon({
  name,
}: {
  name:
    | "play"
    | "pause"
    | "stop"
    | "start"
    | "search"
    | "loop"
    | "undo"
    | "redo"
    | "settings"
    | "warning"
    | "end"
    | "back"
    | "forward"
    | "inspector"
    | "mixer"
    | "save"
    | "export"
    | "erase"
    | "trash"
    | "exit";
}) {
  const paths = {
    play: "m8 5 11 7-11 7z",
    pause: "M8 5v14M16 5v14",
    stop: "M6 6h12v12H6z",
    start: "M5 5v14M19 5 8 12l11 7z",
    search: "M21 21l-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0",
    loop: "M4 8h14l-4-4M20 16H6l4 4M20 8v5M4 16v-5",
    undo: "M4 4v7h7M4 11a8 8 0 1 1 1 7",
    redo: "M20 4v7h-7M20 11a8 8 0 1 0-1 7",
    settings: "M4 6h16M4 12h16M4 18h16M8 3v6M16 9v6M10 15v6",
    warning: "m12 3 10 18H2zM12 9v5m0 3v1",
    end: "M19 5v14M5 5l11 7-11 7z",
    back: "M11 6 3 12l8 6zM21 6l-8 6 8 6z",
    forward: "m13 6 8 6-8 6zM3 6l8 6-8 6z",
    inspector: "M3 12h2l2-6 3 12 3-9 2 5 2-2h4",
    mixer: "M6 4v16M12 4v16M18 4v16M4 9h4M10 15h4M16 7h4",
    save: "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
    export: "M12 15V4m0 0L8 8m4-4 4 4M5 14v5h14v-5",
    erase: "m16 3 5 5-11 11H5l-2-2zM9 9l6 6M4 21h16",
    trash: "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
    exit: "M10 4H5v16h5M14 8l4 4-4 4M18 12H9",
  };
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d={paths[name]} />
    </svg>
  );
}
