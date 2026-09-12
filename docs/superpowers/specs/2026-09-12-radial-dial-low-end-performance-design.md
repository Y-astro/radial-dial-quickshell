# Radial Dial Low-End Performance Design

## Goal

Reduce the radial dial’s measurable CPU and interaction cost on integrated graphics while preserving its current behavior and the richer visual profile on capable hardware.

## Measurement

Measure the complete Quickshell process under matched conditions with the radial module disabled, loaded but closed, and actively used. Repeat runs at idle and under a controlled CPU workload. Record process CPU, memory, open/close latency, failures, and visible interaction regressions. Retain a change only when repeated measurements improve beyond baseline variance.

## Design

Use owner commit `e2f27a8` as the base. Investigate three independent mechanisms in order: explicit threaded Canvas painting for the low-end profile, coalescing or removing redundant low-end repaint requests, and ensuring all menu rendering and timers cease after close. Establish a new compound baseline after every retained experiment.

The low-end profile keeps immediate selection feedback while reducing decorative animation. The high-performance profile retains framebuffer rendering, blur, and existing animations. Automatic hardware detection remains the default and existing manual profile overrides remain supported.

## Compatibility and correctness

Opening, closing, hover selection, submenus, keyboard shortcuts, hold-and-flick activation, drag reordering, browser-tab actions, customizer behavior, and standalone/end4-pC integration must continue to work. The live shell is updated only after static checks and controlled benchmarks pass. The previous installed module remains available through Git history for rollback.

## Delivery

Commit retained changes on `perf/low-end-idle-rendering`, include benchmark evidence and profile documentation, run an independent review, and install the reviewed result into the current end4-pC configuration.
