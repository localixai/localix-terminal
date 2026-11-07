// A resize gesture owns one primary pointer. Work is demand-driven, at most once a frame;
// release uses its own coordinates, and cancellation always clears capture and pending work.
function resizeHandle(handle, { start, move, end }) {
  let pointer = null, latest = null, frame = 0;
  const finish = (event, cancelled) => {
    if (pointer === null || (event?.pointerId !== undefined && event.pointerId !== pointer)) return;
    const id = pointer;
    pointer = null;
    cancelAnimationFrame(frame);
    frame = 0;
    try {
      end(event?.clientX !== undefined ? event : latest, cancelled);
    } finally {
      latest = null;
      if (handle.hasPointerCapture?.(id)) handle.releasePointerCapture(id);
    }
  };
  handle.addEventListener("pointerdown", (event) => {
    if (pointer !== null || event.button !== 0 || event.isPrimary === false || event.ctrlKey) return;
    event.preventDefault();
    if (start(event) === false) return;
    pointer = event.pointerId;
    latest = event;
    // Synthetic events have no native pointer to capture; the window listeners still apply.
    try { handle.setPointerCapture?.(pointer); } catch {}
  });
  addEventListener("pointermove", (event) => {
    if (pointer === null || event.pointerId !== pointer) return;
    if (!(event.buttons & 1)) return finish(event, true);
    latest = event;
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (pointer !== null) move(latest);
    });
  });
  addEventListener("pointerup", (event) => {
    if (event.button === 0) finish(event, false);
  });
  addEventListener("pointercancel", (event) => finish(event, true));
  handle.addEventListener("lostpointercapture", (event) => finish(event, true));
  addEventListener("blur", () => finish(null, true));
  addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || pointer === null) return;
    event.preventDefault();
    finish(null, true);
  });
}
