---
title: Diagnostics follow-ups
---

# Diagnostics follow-ups

Send Logs now carries the GPU, driver, video memory and RAM facts, the last five crash reports and DRED breadcrumbs from the first GPU hang. These parts need game hooks that core does not have, so none exists yet.

## Per-car load line on the select screen

Core names each bottom-level ray tracing build shape in `loader.log` the first time the game builds it, up to 800 shapes: `[gfx] rtas build <n> on list 0x<list> into 0x<dest>` with each geometry's vertex and index counts (`v<vertices>/i<indices>`) and formats. The menus alone build about 500 shapes. It also keeps the newest 64 builds of both levels on each command list, for up to 32 lists, and a crash report with a GPU device removal lists them after the DRED section, the list with the newest build first: `build <seq> frame <frame> list 0x<list> #<n>`, where `#<n>` is the build's place among the builds on that command list in that frame. The DRED section gives each command list node its address (`command list <name> (0x<list>)`), so the build a hang stops at is the `#<n>` on that list that matches the node's first unfinished `BUILDRAYTRACINGACCELERATIONSTRUCTURE`, and its index counts match the records of the mesh that holds it. Still missing: the `.erp` file name and slot of each mesh load, which needs the game's mesh loader located in `re/` and added as a binding.

## Session end classification

The qualifying or race result with positions and lap times, and the grid. The core wraps `Sess_Record_GroupTimes` and `Sess_Build_GroupResult` to rewrite grids, but nothing reads the finished result for the log. The work is a read-only reader over the group result rows described in the sessions page.

## Menu input events

Hover and click target ids on the front end. Core has no hook on the menu input path; `loader.log` records screen and flow events only.
