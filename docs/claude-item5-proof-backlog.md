# Item 5 proof backlog for Claude

This file registers work that is deliberately outside the item 5 functional slice. It does
not claim that the listed obligations are complete.

- Formalize the complete `SceneUpdate` → candidate scene → native rebuild → frame commit
  path, including preservation of focus, pressed identity, viewport scroll and accessibility
  metadata. Existing supporting kernels remain separately scoped and cannot be composed by
  inspection into this whole-path theorem.
- Formalize the public full-scene caller orchestration, including resource rejection followed
  by successful reuse and the interaction between source patches and presentation versions.
- Specify and verify the window adapter's event-loop composition: physical-to-logical pointer
  conversion, focus loss/re-entry, command key routing, IME display ownership and retirement.
  Winit/softbuffer/X11 are explicit third-party TCB and are not modeled by that obligation.
- Exercise a live external editing/IME service and an authorized platform accessibility
  bridge. The examples currently consume injected display snapshots; no editor, IME engine,
  candidate service or system accessibility API is implemented here.
- Replace Xvfb evidence with authorized compositor, hardware Surface, real suspend/resume,
  device-loss and platform accessibility runs where those are part of final host acceptance.
- Repeat the recorded CPU baseline on the definitive release host and establish an explicit
  optimization threshold. The item 5 run records a same-VM regression reference only.
