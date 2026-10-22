# File Tools — project notes & handoff brief

> **Read this whole file before touching code.** It is the single source of truth for any AI assistant or person
> continuing the project. Update §6 (Status) and §12 (Changelog) at the end of every work session.

---------------------------------------------------------------------------------------------------

## 0. PROMPT FOR THE NEXT AI (copy/paste this as your first message in a new chat, with the project zip attached)

```
You are continuing a personal Linux desktop app called "File Tools" (Rust + GTK4). Read NOTES.md in the attached zip
completely before doing anything. Then:

1. Read §7.0 (bug-report triage) and §13 (the current "next prompt"). Do NOT start coding yet. In 10 lines or fewer, tell me what you understood: what the app is, the current status
   (§6), and which roadmap round is next (§7). Wait for my "go".
2. When I say go, implement exactly the next round from §7 — nothing more. Respect §2 (limitations), §3 (build/test),
   §9 (conventions/pitfalls) and §10 (token-saving rules). Edit files in place and return ONE zip (no target/ folder).
3. Verify before you hand over: `cargo check`/`cargo test`, and for any UI change run the app headless and look at a
   screenshot (tools/shot.sh, tools/interact.sh). If you cannot compile, say so clearly instead of guessing.
4. Finish with: what changed (short), how to apply it (fish-shell commands), what I should test (checklist), what you
   could NOT verify, and which round comes next. Update NOTES.md (§6 Status, §12 Changelog, and §7 if the plan changed).
5. Keep replies short and in plain language. Ask at most one question at a time; prefer a sensible default.
```

---------------------------------------------------------------------------------------------------

## 1. What this is

A personal **desktop image toolkit** (Rust + GTK4, Linux). You open an image, apply tools one after another (each tool
takes the previous tool's result — a *pipeline*), and save. Tools: AI background removal (rembg), Adjust, Crop &
Transform, Fill Background, Palette, Duotone, Halftone, ASCII art, Pixelate, **Paperize** (torn-paper border + weathered
grunge), Outline, Watermark, Convert. Long term it should grow into a light **paint.net-style editor** (layers, masks, pixel
editing) with **projects**, **history rollback**, **recipes/batch**, a **PDF workspace** and proper **AI masking**.

**Owner profile (important for how you communicate):** beginner programmer, uses AI assistants to build this. CachyOS (Arch),
Hyprland + Caelestia shell, **fish** shell (not bash!), Ryzen 5 5500U laptop (12 threads, iGPU, no GPU compute), 1080p screen,
black/OLED-friendly themes. Wants: short plain-language replies, large readable buttons, a paint.net-like look, laptop-friendly
CPU/battery use. The owner tests every zip by hand and reports back with screenshots.

---------------------------------------------------------------------------------------------------

## 2. LIMITATIONS & HARD CONSTRAINTS (read carefully)

* **Owner is on a free AI plan with tight token limits.** Long chats are expensive: keep replies short, never paste whole files,
  avoid re-reading big files, batch related changes, and suggest a fresh chat (with this file) when a conversation gets huge.
* **No GPU compute** (ROCm was tried and removed). rembg runs on CPU: AI removal takes seconds-to-minutes on big images.
* **Laptop heat:** a full dependency rebuild once hit 90 °C. Always use `./cool.sh` (CPU-capped cargo). Never change
  `[profile.dev]` opt-levels or Cargo.lock lightly — it forces a full dependency rebuild. `.cargo/config.toml` limits jobs to 4.
* **Wayland/Hyprland is the real target; you can only test on X11** (Xvfb). Popup/focus/clipboard behaviour can differ
  (GTK popovers are non-autohide on purpose — see §5.7). Say so explicitly when something is Wayland-sensitive.
* **ImageMagick:** the owner has ImageMagick **7** (`magick`). Sandboxes often only have IM6 (`convert`); a `magick` shim and
  removing the `@*` policy line in `/etc/ImageMagick-6/policy.xml` is needed to test ASCII (`label:@file`).
* **rembg lives in a Python venv** at `~/bg-remover-env` and is called by absolute path (no venv activation needed).
* **SVG icons need `librsvg`** (gdk-pixbuf SVG loader). Without it the app falls back to text-only buttons (`icons.rs`).
* **Previews are not history:** big images (long side > 1800 px) are previewed on a 1280 px copy for Adjust/Fill/Paperize; the
  full-resolution result is produced only on Save/Copy/tab switch/Process (see §5.3). The step appears in History only then.
* **Not persisted yet:** History/pipeline is lost on exit (projects come in Round 5). Settings, themes, presets, tab layout are saved.
* **No GUI test framework.** Logic is unit-tested (`cargo test`); UI is verified by headless screenshots.
* **GTK4 has no global "disable tooltips" switch** — `settings_panel::apply_tooltips` walks the widget tree (call it again after creating new tooltip-bearing widgets). Rounded popover corners only look right with a compositing window manager (Wayland yes; the Xvfb/openbox test setup shows black corners — not a bug).
* **WebP export:** the `image` crate only writes lossless WebP; lossy quality goes through ImageMagick (`magick -quality`) with a lossless fallback.
* **Default GTK widgets are still used** for the colour picker, dropdowns, switches, scrollbars and file dialogs. The owner wants these replaced by custom, themed widgets (see Round 4b).
* **Only Adjust, Fill and Paperize use the fast proxy preview.** Others render at full resolution on every live tick.
* The `next_page` "value assigned is never read" compiler warning in `app.rs` is a known harmless leftover of the `register_page!` macro.

---------------------------------------------------------------------------------------------------

## 3. Build / run / test

```fish
./cool.sh run          # debug run, CPU-capped (default 50% of all threads; PCT=35 ./cool.sh run)
./cool.sh check        # fast compile check — after every edit
./cool.sh test         # unit tests (slow/hot to compile the first time)
./install.sh           # release build + launcher "File Tools" + icon (~/.local)
file-tools photo.png   # open an image directly (also "Open With" in the file manager)
```
* Rust 1.91+, GTK 4.12+ (`gtk4` crate 0.9, feature `v4_12`). Owner has newer toolchains; keep code compatible with 1.91.
* Runtime tools: `magick`, `wl-copy`/`wl-paste`, `sox`, `exiftool`, `qpdf`, `pdftoppm`, `fc-match`, `~/bg-remover-env/bin/rembg`.
* **`cargo check` can report "Finished" in ~1 s even after big edits if mtimes don't change; when in doubt `touch src/*.rs` and
  do a real `cargo build`.**

### Headless verification (do this for every UI change)
Sandbox packages (Ubuntu/Debian): `apt install rustc-1.91 cargo-1.91 libgtk-4-dev libcairo2-dev pkg-config build-essential xvfb xdotool
imagemagick openbox fonts-dejavu-core adwaita-icon-theme librsvg2-common` (run `apt-get update` first; use `RUSTC=/usr/bin/rustc-1.91
cargo-1.91 …`; background installs die when the shell call ends — run them in the foreground).
* `tools/shot.sh <theme> out.png [image]` — one screenshot of the app.
* `tools/interact.sh <theme> [image]` with `EXTRA='xdotool …; import -window root /tmp/x.png'` — scripted clicks/keys + screenshots.
  `xdotool key ctrl+Return` triggers "Process" for the active tool (more robust than clicking). Click coordinates change whenever
  the rail/icons change — **take a screenshot and measure first** (the screenshot you *view* is scaled ≈ 1372/1500; multiply by ≈ 1.093).
* Always test a dark system theme **and** `SYSDARK=0` (light), because the owner's system is dark but themes can be light.
* Shell gotcha: `kill $pid; wait $pid` makes a script exit 143 — don't chain scripts with `&&`.

---------------------------------------------------------------------------------------------------

## 4. Architecture map (src/, ~10k lines)

| File | Role |
|---|---|
| `main.rs` | declares modules, calls `app::run()` |
| `app.rs` (~1.9k) | `AppState` (shared via `Rc`), window layout, rail, `register_page!` macro (one block per tab), history/pipeline logic (`commit_step`, `undo`, `redo`, `rewind_to`, `goto_step`, `refresh_panels`), key shortcuts, CLI file arg (`INITIAL_FILE`), toolbar/status bar/History dock construction |
| `menus.rs` | menu bar (File/Edit/View/Tools/Help), all `win.*` actions, accelerators, *Open Recent* submenu (`fill_recent`), Shortcuts/About dialogs |
| `icons.rs` | bundled Phosphor SVG icons (`assets/icons/*-symbolic.svg`, MIT) compiled in, written to `~/.cache/file-tools/icons` at startup. `tab_icon()` (rail) and `ui_icon()` (toolbar). Returns `None` without SVG loader → text-only |
| `tabs/base.rs` (~800) | `BaseTab`: shared tab UI (title, Process button, status, presets, `tracked_slider` w/ orange "modified" state), live-preview scheduling, `spawn_processing*` (ImageMagick jobs), `spawn_work` (generic blocking job + spinner + commit), `run_live_proxy` (magick + proxy), `run_live_rust`/`run_rust_full` (Rust jobs), `get_preset_data` (settings snapshot) |
| `tabs/*.rs` | one file per tool: `adjust crop fillbg palette(+palette_container) duotone halftone ascii pixelate outline watermark convert remove paperize` (+`gradient`, not on the rail) |
| `widgets/preview.rs` (~1000) | the canvas: zoom/pan, original-vs-result fade, **checkerboard behind transparency**, eyedropper, crop overlay, `set_on_cursor` (status bar), pub zoom buttons/label; `surface_from_rgba` |
| `widgets/mask_canvas.rs` | small brush editor in the Remove tab (draws mask strokes; checkerboard for transparency) — **to be replaced by the real masking tool in Round 6** |
| `settings_panel.rs` | `Settings` (JSON `~/.config/file-tools/settings.json`), settings popover, **theme system** (`palette()`, `THEME_CSS`, `apply_color_scheme`) |
| `settings.rs` | `Layout` (which tabs are enabled — missing names default to enabled), per-tab `TabPresets` |
| `util.rs` | `run_capture`, `resolve_font`, `image_dimensions` (header-only fast path), `preview_proxy` (1280 px cached copy), clipboard, colour math, palette extraction, done-sound |
| `config.rs` | paths: config dir, tmp files, save dir `~/Pictures/FileTools`, bg-remover binaries |

### UI layout (v10)
```
toolbar:  [Open][Recent▾][Paste][Undo][Redo]  [filename ………………]  [Save As][Copy][Reset][Tabs][Settings]   (big icon+label buttons)
┌ toolbox ─────┬ canvas (preview, checkerboard behind transparency, fade slider at the bottom) ┬ right dock: tool settings / HISTORY ┐
│ ↔ size btn   │                                                                                  │                                      │
│ icon grid    │                                                                                  │                                      │
status bar:  size · cursor x,y · ………………… [− 100% + Fit 1:1] · N steps (· "preview not applied yet")
```
* **No menu bar** (owner asked for it to be removed). `menus.rs` still registers every `win.*` action + accelerators (Ctrl+O/S/Q, zoom keys) and builds the *Open Recent* model that the toolbar **Recent** button shows.
* **Toolbox** = icon-only `FlowBox` (no text, no group headers). The `↔` button cycles 3 sizes (small 1 column / medium 2 columns / large 2 columns + names); stored in `Settings.toolbox_size`. Tool visibility from the Tabs popover is applied with a `FlowBox` filter (`AppState.rail_hidden` + `refresh_rail_groups`). If SVG icons are unavailable the names are shown instead.
* **Zoom controls** (`Preview.controls`) are hosted in the status bar, not on the canvas.

---------------------------------------------------------------------------------------------------

## 5. Key concepts (how things really work)

### 5.1 Pipeline & history
`AppState.history: Vec<HistoryEntry{label, output_path, params, live}>`; `redo_stack` holds undone entries. Every tool reads the
current head (last entry's file, or the initial image) and writes a new temp file. **Rules (v8):**
* Explicit **Process** (go button / Ctrl+Enter) always creates a **new step**.
* Consecutive **live-preview** runs of the same tool collapse into **one step** (`commit_step`: replace only if last entry is `live` and same label).
  `AppState.live_run` is set to true while a live trigger runs, so commits get tagged (`BaseTab::set_live_trigger` wraps the trigger).
* `params` = `BaseTab::get_preset_data()` (same JSON the Presets menu uses). This is the seed for projects/recipes/batch.
* `undo()`: if a live preview is pending it is cancelled; else the last step is undone. Ctrl+Z is ignored while typing in a text field.
* `goto_step(n)` (History panel): 0 = original, n = after step n; later steps stay for redo.
* The active tool keeps its **own** input (the image before its step) so live tweaks never feed back into themselves (`broadcast_input(skip_active)`).

### 5.2 Transparency rules
Tools that read pixel values must respect alpha. RemBg leaves the *original colours hidden under transparent pixels*. Halftone treats
transparent = no dot, semi-transparent = smaller dot (`transparent_pixels_get_no_dots` test). ASCII → blank for alpha<32. Duotone keeps alpha.
**Verify Pixelate/Outline/Watermark/Palette with RemBg output (not yet done).** The canvas always draws a checkerboard behind the image.

### 5.3 Live preview flavours (`tabs/base.rs`)
0. **Rust jobs** (Paperize): `run_live_rust(app, make)`; `make(full)` reads controls → `RustJob = Box<dyn FnOnce(&Path /*input*/, f64 /*scale*/) -> Result<PathBuf,String> + Send>`.
1. **Classic** (most tabs): live trigger = `run()` → full-res commit every tick.
2. **Proxy** (Adjust, Fill): `run_live_proxy(app, build)`; big images preview on `util::preview_proxy` (1280 px, cached, returns `(path, scale)`),
   result shown **without committing**; `AppState::set_pending_commit(closure)` registers the deferred full-res commit.
   `flush_pending()` is called by `save_as`, `copy_result_image` and tab switches. `drop_pending()` (also bumps `live_gen` so late results are ignored)
   is called by `commit_step`, `rewind_to`, `redo`, `set_universal_input`, `load_from_history`. Pixel-valued params must be multiplied by `scale`.
   Not suitable: Duotone (already in-process), Gradient (generator).
* **Crop is preview-only on live** (`run(app,false)`), committed only by Process; state (box/rotate/flip/resize) is cleared after commit.

### 5.4 Themes (`settings_panel.rs`)
5 themes: **dark** (grey), **black** (pure #000), **white**, blue, red. `palette(theme)` → colours incl. `modified` (orange "changed from default"),
`on_accent`, `dark` flag. `THEME_CSS` is a template with `$tokens`. `apply_color_scheme()` forces GTK's light/dark variant. GTK hard-codes a blue border on
`scale > trough > highlight` and `button.suggested-action` — those are overridden explicitly. Old names migrated once via `Settings.theme_v`.
dark/black/white use black-and-white accents on purpose.

### 5.5 Icons
Phosphor "regular" SVGs copied from the npm package `@phosphor-icons/core` into `assets/icons/ft-<name>-symbolic.svg`; add to `ICONS` and map in
`tab_icon()`/`ui_icon()`. GTK recolours `-symbolic` icons to the text colour. (Lucide's stroke icons do **not** recolour — use fill-based sets.)

### 5.6 Menus & actions
`menus::install()` registers `win.*` actions; tool entries use `win.tool('<Tab name>')` which toggles the rail button. Undo/Redo/Copy/Paste shortcuts live in the
window key controller (capture phase) — don't also register accelerators for them or they'll fire twice.

### 5.7 Popovers
The settings popover is `autohide(false)` and closes via ✕, Esc, click-outside (window-level gesture) or the gear — some Wayland compositors never dismiss GTK popups.

### 5.8 Tool-specific notes
* **ASCII:** `probe_cell_metrics` measures cell size + ImageMagick label padding (two probe renders, cached); the grid is chosen so the finished text layer matches the image
  aspect (±2 columns); trailing newline is trimmed before rendering (it adds a blank row); image bases are scaled to *cover* + cropped, never stretched.
* **Halftone:** renders at 3× supersampling in pure Rust; background options: None / Black & white / Pure black / Original image / White / Custom.
* **Paperize:** pure-Rust renderer (`render_paperize`): optional torn border (style 0 = paper border around the photo, style 1 = tear eats into the photo; multi-scale noise
  tear, pale fibrous rim, drop shadow, paper kind, background), then weathering (fade, yellowing, stains, mottling, burnt edges, dust, scratches, creases) and grain.
  Output canvas can be larger than the input. Noise coordinates use "real" pixels (`x/scale`) so preview == final.
* **Remove:** rembg/backgroundremover via absolute venv paths; sound-when-done is a *global* setting (Settings → Sound).

### 5.9 Transparent PNG/WebP handling (owner priority)
* Hidden colours under fully transparent pixels are cleared on load: `util::sanitize_transparency` (header check for an alpha channel → copy with RGB zeroed, keeps the file name, written under the tmp dir) is called from `AppState::set_universal_input`. RemBg output is cleaned by the internal job step `@clean-alpha` (handled in `BaseTab::spawn_impl`, Rust, exact; **do not use ImageMagick `-alpha associate/disassociate` — it corrupts alpha**).
* Tools must still be alpha-aware (Halftone/ASCII/Duotone/Paperize are). **Not yet audited with transparent input:** Pixelate, Outline, Watermark, Palette, Fill Background, Convert — put this in the bug-fix round.

### 5.10 Canvas compositing & fade slider
Layers: checkerboard → original (opaque) → *(result over its own checkerboard)* painted at `fade` opacity. That is a true crossfade original↔result, so removed backgrounds fade out through the checkerboard (no jump at 100%). The slider shows for the tools in `FADE_TABS`.

### 5.11 Crop & Transform (v10)
No generic Process button. Rotate/flip buttons apply immediately; "Rotate by angle", "Crop to selection", "Resize" and "Trim empty borders" each apply their own operation via `run_ops(ops_mask)`; every action = one History step; the crop overlay and consumed settings are cleared afterwards; Ctrl+Enter = crop to selection. No live preview for this tool.

### 5.12 Paperize silhouette
The torn border is built from a signed distance field of the image's alpha (`signed_distance`, Felzenszwalb EDT): a cut-out PNG gets a border hugging its real outline, transparent areas stay transparent, an opaque photo is a rounded rectangle. Style 1 erodes the subject instead. Crumple shading on paper; weathering on everything.

### 5.13 Document core (v11) — "what is the current image?"
* **`AppState::current_head()` is the single source of truth** (last History step, else the opened file). Save As, Copy, the status bar and tool inputs all use it. (`current_result` is legacy; don't use it.)
* `open_path()` = new document (Open, Recent, Paste): history **and redo** are cleared, the file-name box resets. `reset_to_original()` pushes an **undoable "Reset" step** (output = the original file); Ctrl+Z brings the edits back.
* `sync_tool_inputs()` (after Undo/Redo/History click): every tool gets the head as input, **except the active tool when the head step was made by that tool** — then it gets that step's *own input* (previous step) and its controls are restored from `HistoryEntry.params` via `BaseTab::apply_preset_quiet` (no re-render). This fixed Paperize stacking on itself. `HistoryEntry.tool` = tab that produced the step.
* **File name box:** `set_name_auto()` keeps it in sync with the pipeline until the owner types a name (`name_edited`), then tools stop overwriting it. Opening a new document resets it. Enter/click-away drops focus.
* **Focus:** a window-level click gesture clears keyboard focus after any click that isn't on a text field (no lingering blue highlight); button focus outlines are removed in CSS.
* **Toasts (`toast.rs`):** `app.toast("msg")` and `app.toast_actions(msg, [(label, cb)…], secs)` (used for the ✓ Overwrite / ✕ confirmation). Use them for every user action that has no visible result.
* **Save As (`save_dialog.rs` + `export.rs`):** name, folder, PNG/JPEG/WebP, quality (default 100), optional resize with ratio lock; always saves the head, even with zero steps; overwrite asks via toast when "Ask before overwriting" is on; honours the exiftool metadata settings. The old **Convert tool is to be removed** (its job is now this dialog) — not done yet.
* Settings now wired: *Show tooltips*, *Default AI model* (`AppState.on_default_model` → `RemoveTab::select_model_key`).
* Sliders (`BaseTab::tracked_slider`) pass plain scroll to the page; Ctrl+scroll adjusts them.

### 5.14 AI plan (decided with the owner — implement in "AI v2")
Python/rembg is slow because every run starts a new Python process, imports onnxruntime and reloads the model. The models are plain ONNX files, so run them **in-process from Rust with ONNX Runtime (`ort` crate, `load-dynamic`)**, keep sessions in memory (load in the background at startup), cap CPU threads (4–6) for laptop heat. No GPU (AMD iGPU unsupported).
* **System dependency (owner approved):** ONNX Runtime from the distro (`sudo pacman -S onnxruntime` — *verify the exact package/version*; `ort` 2.0.0-rc.x expects a matching ORT major/minor; pin versions). **Must appear in the README.**
* **Models** (downloaded on first use into `~/.local/share/file-tools/models/`, SHA-256 verified, progress toast; the Settings list must show ALL of these by default so any can be downloaded):
  | Purpose | Model | Size | License |
  |---|---|---|---|
  | Auto cutout, **default** | silueta | ~42 MB | Apache-2.0 |
  | Auto cutout, small/fast | u2netp | ~5 MB | Apache-2.0 |
  | Auto cutout, sharper | isnet-general-use | ~170 MB | Apache-2.0 |
  | Auto cutout, best (optional) | BiRefNet-lite | ~214 MB | MIT |
  | Click/box/brush object selection | MobileSAM (encoder ≈27 MB + decoder ≈16 MB) | ~60 MB | Apache-2.0 |
  (Sizes/licenses come from rembg/MobileSAM/samexporter listings — **re-verify each license file before publishing**.)
* **Do not commit weights to git** (GitHub 100 MB file limit). Ship `models/manifest.json` {id, url, sha256, size, license, purpose}; host files as GitHub Release assets or link the original sources; include each model's LICENSE/NOTICE text in the repo. Project license: **MIT** (`LICENSE`).
* **UX:** Mode = *AI Mask* (default; auto-detect, then click/box/brush to add or subtract; refine edges with matting) or *Erase*. No mask → whole-image background removal. Mask is a toggleable overlay on the main canvas (the small preview in the Remove tab goes away). Reset must reset brush size.
* MobileSAM: encode the image once (≈1–3 s on CPU, estimate), then each prompt returns a mask almost instantly (decoder). Models can be tested in the sandbox (rembg's release assets and PyPI's `onnxruntime` wheel are reachable).

### 5.15 README to-do (write later, keep this list current)
Install/build (`./cool.sh`, `./install.sh`), **system packages**: ImageMagick 7, wl-clipboard, sox, exiftool, qpdf, poppler (pdftoppm), librsvg, **onnxruntime**, fontconfig; first-run model downloads + manifest; supported formats; keyboard shortcuts; screenshots; license (MIT) + third-party licenses (Phosphor, model licenses); how to report bugs.

---------------------------------------------------------------------------------------------------

## 6. Status (update every session)

**Done (v1 → v11):** see §12. **v11 = Round 1 "Core v2"**: single source of truth for the current image, Save As dialog (+export engine), undoable Reset, Recent popover list, toasts, file-name box rules, tool-input fix after Undo (Paperize stacking), restored settings on History navigation, focus-highlight fix, tooltips + default-AI-model settings wired, slider scroll fix, MIT license.

**Untested by the owner:** v11. Unit tests: 12 pass (+2 ignored).

**Known/unfinished from the owner's bug report:** everything in §7.0 marked "Planned".

**Open / unverified:** Wayland behaviour; rounded popover corners on a real compositor; `librsvg` on the owner's system; History list colour on the owner's newer GTK.

---------------------------------------------------------------------------------------------------

## 7. ROADMAP — one round per session; each round ends with one zip for the owner to test

### 7.0 Bug-report triage (owner's report, v10 testing) — where each item is handled
**Done in v11 (Round 1):** Save As after Undo/History/Recent/without edits (+ format choice, quality, resize) · toasts for save/copy/paste/open/recent/reset · file-name box overwritten by tools · name box focus/highlight · Reset = undoable step · opening Recent resets the document · Recent list updates + long names · MenuButton look (Recent/Tabs/Settings) · "Show tooltips" · "Default AI model" · lingering blue button highlight · Paperize stacking after Undo · restore settings when going back in History · sliders swallowing vertical scroll.
**Planned — Round 2 (Shell v2):** square popover corners (CSS done, needs compositor check) · presets popup rework (small text, white theme) · toolbox = ONE scrollable column · compare/preview slider scaling (halftone/ASCII output size vs original) and compare modes (Settings: previous step / before this tool / original) · image goes black when zoomed in (Cairo limit — draw only the visible part) · tool-name consistency (toolbar vs inspector title, "Palet", "(…)" in Paperize, Watermark shows "Gradient") · remove the small preview in Remove · eyedropper "magnifying glass" confusion · custom colour picker + global primary/secondary colours with expandable palette generator · docked, collapsible, fixed-size panels (no resize handles; reset on restart) · minimal Layers panel (image + toggleable mask overlay).
**Planned — Round 3 (Engine v2: Rust, alpha-aware):** Adjust (brightness/contrast/vignette must not touch alpha; **saturation bug = ImageMagick `-modulate` hue sent as 0 instead of 100**; warmth far too weak) · Pixelate lag above 50 · Outline lag on thick outlines (use the EDT in `paperize.rs`) and rename "Smoothness"→"Blur" · Fill Background (blur-original always white; stray 🔍) · Halftone (lines vs dots identical, shape ignored in lines mode, backgrounds must only colour the subject not transparent pixels, output size vs original) · ASCII (lag at large widths → render in-process with cairo/pango, remove/repurpose font size, brightness −100 vanishing, shared Background function) · Gradient rewrite (linear angle really rotates the gradient, colour ratio/stops 60/40, offset x/y, size as numeric inputs with ratio lock, "background for current image" = image size, radial centred, remove useless angle slider) · shared Background function for Halftone/ASCII/Paperize/Outline/Fill · **transparency audit with a test per tool** · Convert tool removed (now Save As).
**Planned — Round 4 (Tools v2):** Crop (aspect-ratio presets Free/1:1/4:3/3:2/16:9/4:5/5:4/9:16/3:4/2:3 + custom + portrait/landscape flip; **phone-gallery style live rotate** with cut-off corners darkened; crop box must not flash/reset when clicking the same pixel and stays dimmed while active; resize removed from Crop or given a ratio lock) · Paperize v3 (remove Background option; paper colour must also colour the torn rim/edges; weathering only on the subject; **"cutout/scissors" mode with straight-ish edges** replacing the bitey "torn photo edges"; canvas **grows** (owner decision); the weird transparent margin/harsh expansion) · Watermark (tab title bug, **text-only rich text box** with per-selection font/bold/italic via GtkTextView tags + Pango render; saves its content in presets).
**Planned — Round 5 (AI v2):** §5.14.
**Planned — Round 6 (Projects & files):** projects = folders, project file (`.ftproj` zip: manifest + original + steps/params + thumbnail), session restore.
**Planned — Round 7:** recipes + batch + drag-and-drop. **Round 8:** PDF workspace (qpdf + pdftoppm; page grid, reorder, extract, merge, split, rotate; later OCR/compress/protect).
**Owner decisions on record:** rotate = phone-gallery style · Paperize on opaque photos grows the canvas · docked fixed-size panels, no draggable resizing · global primary/secondary colours (+ expandable palette generator) · compare modes in Settings · AI object recognition > masking UI polish; Rust ONNX, models downloadable, silueta default and the others in the list · watermark = text only · MIT licence · system ONNX Runtime package OK (document in README).

## 8. CLEANUP / THINGS TO CHANGE (do when touching the area)
* `menus::install` still returns an unused `PopoverMenuBar` (menu bar was removed from the window); keep the actions, drop the widget code when convenient. `rail_group_headers/rail_group_tabs` fields are unused now.
* After Round 4 remove: `Tabs ▾` button + popover, `Layout::is_enabled` UI, unused `strip_box`/`refresh_strip` chip code, `history_list`/`refresh_history` (replaced by Recent menu), `hist-pane` CSS.
* `refresh_strip()` is now just a trigger for `refresh_panels()` — rename it.
* Presets for Paperize/Halftone store dropdown *indices*; changing option order breaks old presets — consider storing names.
* `next_page` warning in `register_page!`.
* Consider moving `RAIL_LAYOUT`/tool metadata into a single `ToolInfo` registry (name, short, tooltip, group, icon, constructor) — needed for Round 4 customisation, menus and Find tool.
* `mask_canvas.rs` dead-ends once Round 6 lands.

---------------------------------------------------------------------------------------------------

## 9. Conventions & pitfalls we already hit
* gtk4-rs: `gtk::prelude::*`; `Label::set_xalign`; `DropDown::from_strings`; clones for closures (`let b = base.clone();`). Avoid `RefCell` borrow overlaps in callbacks that re-enter (`set_value` triggers handlers).
* A name `ctrl` shadowed the closure parameter in the key handler — use distinct names.
* `format!("{}", f64)` is fine for ImageMagick numeric args; pixel-valued args must be scaled by the proxy `scale`.
* `image::imageops::blur` handles RGBA; fill transparent pixels' RGB before blurring or edges go dark.
* ImageMagick `label:@file` adds padding and treats a trailing newline as an extra row.
* Don't `kill`/`wait` in scripts you chain with `&&`. Don't background `apt-get` across shell calls.
* GTK CSS: `color` inherits; `.inspector-pane label { color }` also recolours labels inside buttons — add explicit overrides for accent buttons.
* Cairo ARGB32 is premultiplied BGRA on little-endian.
* Never leave a settings field without a default (`#[serde(default)]` on `Settings`; per-field default for migration flags).

---------------------------------------------------------------------------------------------------

## 10. Working agreement for AI assistants (token-saving)
* Edit in place; return **one zip** (exclude `target/` and `.git/`). Never paste whole files back and forth.
* Compile-check every change; unit-test logic; screenshot UI. Don't re-verify things that already passed.
* Batch related changes (3–5 per round) but keep each batch compilable. Prefer `grep`/line ranges over reading whole files.
* Replies: what changed, how to apply, what to test, what's unverified, what's next. No long recaps. One question at most.
* Owner's shell is **fish**; give fish-compatible commands (`cd ~/Projects; unzip -o ~/Downloads/<zip>; cd file-tools-rs; ./cool.sh run`).

---------------------------------------------------------------------------------------------------

## 11. Decisions & non-goals
* Stay on GTK4 (Wayland-native); keep the image engine independent of UI so a UI swap remains possible.
* No paid AI tools recommended; free options (Aider/OpenCode + free keys, Gemini CLI — check current limits) are fine. Local LLMs: poor fit (CPU only).
* Scope target for editing = "paint.net-lite", not Krita/GIMP.

## 12. Changelog
* **v11** — Round 1 "Core v2": `current_head()` truth, Save As dialog + `export.rs`, undoable Reset, `open_path`, Recent popover, toasts, name box rules, `sync_tool_inputs` (+ `HistoryEntry.tool`, settings restore), focus-clearing, tooltip + default-model settings, slider scroll, button look, MIT `LICENSE`, expanded NOTES (AI plan, README list, triage, next-AI guide).
* **v10** — menu bar removed; icon-only toolbox with 3 sizes; zoom controls in the status bar; Recent button; Crop rebuilt (single-purpose actions, no Process button); fade slider = true crossfade and shown for all effect tools;
  Paperize border follows the subject silhouette (EDT) + crumple; transparent PNG/WebP clean-up (`sanitize_transparency`, `@clean-alpha`); History panel explicit colours; toolbox_size setting.
* **v9** — UI shell: menu bar (File/Edit/View/Tools/Help, shortcuts, Open Recent), big icon toolbar with Undo/Redo, History dock with click-to-rollback (future steps dimmed), status bar
  (size/zoom/cursor/steps), `tools/interact.sh`, expanded NOTES. Old bottom chips + saved-files list removed.
* **v8** — Paperize rewritten (torn paper border + weathering); history entries carry `params` + `live`; undo fixes; Crop commit fix; Halftone transparency fix.
* **v7** — Paperize (layered, superseded); canvas checkerboard; accent-button label fix; `run_live_rust`/`spawn_work`.
* **v6** — orange modified sliders; switch/blue-border fixes; Fill proxy preview; Phosphor icons on the rail.
* **v5** — sound moved to Settings; halftone/ASCII bases; ASCII un-stretch; themes renamed (dark/black) + monochrome accents; Adjust proxy preview.
* **v4** — halftone/ASCII overlay; CPU-capped builds; NOTES.
* **v3** — theme rewrite; popover closing; CLI open. **v2** — OLED theme, launcher, perf fixes.

---------------------------------------------------------------------------------------------------

## 13. HOW TO BRIEF THE NEXT AI (and the prompt to use right now)

**Rule for every AI that finishes a round:** before handing over, (1) update §6 Status, §7.0/roadmap statuses and §12 Changelog, (2) rewrite the block below so the *next* AI knows exactly what to do, (3) put a one-paragraph summary of what you could **not** verify.
**How to build the next prompt:** state the round name from §7, list the concrete items (copy from §7.0), name the files likely involved, repeat the constraints that matter for that round (§2), say how to verify (§3), and say what the owner should test. Keep it under ~25 lines.

### CURRENT NEXT PROMPT (paste into a new chat with the zip attached)
```
Read NOTES.md fully (especially §0, §2, §5.13, §7). Next is ROUND 2 — "Shell v2" (paint.net-style UI polish), items listed under
"Planned — Round 2" in §7.0. Do not touch the engine/tool internals except where an item requires it.
Key decisions: toolbox = ONE scrollable column (3 sizes already exist; remove the 2-column grid), docked collapsible panels with FIXED sizes (no drag handles),
custom colour picker + global primary/secondary colours with an expandable palette generator, a canvas rewrite that draws only the visible region
(fixes the black image when zoomed in) and scales original/result to the same rect (fixes the compare slider), compare modes in Settings,
consistent tool names everywhere (see list in §7.0), remove the small preview in the Remove tab, rework the Presets popover (readable in the white theme).
Verify with tools/shot.sh + tools/interact.sh in dark AND light themes; run cargo test; deliver one zip; then update NOTES.md (§6, §7.0, §12, §13).
Before coding, reply in <=10 lines with your understanding and wait for "go".
```
