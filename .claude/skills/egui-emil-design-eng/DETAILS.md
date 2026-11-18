# Details that make components loved

These principles come from building Sonner, a widely used web toast library, and apply to any component, including egui widgets and app-internal helpers.

## Building loved components

1. **Developer experience is key.** No setup ceremony. For an egui component: one struct in app state, one call per frame (`toasts.show(ctx)`), one call to use it from anywhere (`toasts.push("Saved")`). The less friction, the more it gets used.
2. **Good defaults matter more than options.** Ship it looking and moving well out of the box. Most callers never customise. The default curve, duration and visuals must be excellent, so put them in the component, not in its builder.
3. **Naming creates identity.** A memorable name beats a generic one. Sacrifice some discoverability for memorability when it fits.
4. **Handle edge cases invisibly.** Pause toast timers while the window is unfocused (`ctx.input(|i| i.focused)`). Treat the gaps between stacked toasts as part of the stack so hover doesn't flicker. Keep a drag attached to the widget when the pointer leaves it (egui does this). Users never notice these, and that is exactly right.
5. **Retarget, don't restart, for dynamic UI.** Toasts arrive in bursts. A start-time timeline jumps back to 0 when retriggered; `open_t`, `animate_value_with_time` and springs continue from where they are.
6. **Make it easy to try.** A small demo app (or an example in the repo) that people can click before they adopt the component lowers the barrier more than any docs page.

## Cohesion

Sonner's motion feels satisfying partly because the whole thing is cohesive: the curve and duration fit the design, the name, the page. It is slightly slower than typical UI motion and uses a gentle ease rather than a sharp ease-out, to feel elegant.

Match motion to the component's personality:

| Component mood | Motion |
| --- | --- |
| Professional tool, dashboard, editor | Crisp and fast: 100-200ms, strong ease-out, no bounce |
| Calm notifications | A little slower (about 400ms), gentle curve |
| Playful, consumer, onboarding | Can be bouncier: a spring with lower damping |

## Asymmetric enter and exit

Slow where the user is deciding, fast where the system is responding.

| Action | Timing |
| --- | --- |
| Hold-to-delete while held | 2s linear (deliberate) |
| Release before it completes | 200ms ease-out (snappy) |
| Popover open | 150-250ms ease-out |
| Popover close | Faster than open, or instant |

The hold-to-delete example in MOTION.md implements both halves.

## The opacity + height problem

When list items enter or leave (an expanding drawer, a list that grows), the opacity change has to work with the height change. In egui that height change is a layout animation: the one case where animating size *is* the effect. There is no formula. Adjust until it feels right:

- let height lead and opacity follow on enter; fade out first, then collapse on exit
- clip the content to the animating height (`painter.with_clip_rect` or a `ScrollArea` with a max height) so it doesn't spill
- keep the total short (200-300ms) so the list doesn't feel sluggish

## Review your work the next day

Look at motion again with fresh eyes. You will notice imperfections the next day that you missed while building. Play it in slow motion (MOTION.md has a duration multiplier) to catch timing issues invisible at full speed.

## Unseen details checklist

Run through this before calling a component done:

- [ ] Press feedback on everything pressable
- [ ] Nothing enters from scale 0; popovers grow from their trigger
- [ ] Exit is faster than enter
- [ ] Rapid toggling reverses smoothly instead of jumping
- [ ] Nothing animates on a keyboard shortcut
- [ ] Focus is visible; Escape closes overlays; gesture-only actions have a keyboard path
- [ ] Hit areas at least `interact_size`, and they don't move while the drawing animates
- [ ] Reduced motion keeps fades and drops movement
- [ ] No repaint loop when nothing is moving
