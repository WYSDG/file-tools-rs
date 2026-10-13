# File Tools — project notes & handoff brief

> **Read this whole file before touching code.** It is the single source of truth for any AI assistant or person
> continuing the project. Update §6 (Status) and §12 (Changelog) at the end of every work session.

---------------------------------------------------------------------------------------------------

## 0. PROMPT FOR THE NEXT AI (copy/paste this as your first message in a new chat, with the project zip attached)

```
You are continuing a personal Linux desktop app called "File Tools" (Rust + GTK4). Read NOTES.md in the attached zip
completely before doing anything. Then:

1. Do NOT start coding yet. In 10 lines or fewer, tell me what you understood: what the app is, the current status
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

### UI layout (v9)
```
menubar (File Edit View Tools Help)
toolbar:  [Open][Paste][Undo][Redo]  [filename ………………]  [Save As][Copy][Reset][Tabs][Settings]   (big icon+label buttons)
┌ rail (tool icons, grouped) ┬ canvas (zoom bar + preview + fade slider) ┬ right dock: tool settings (top) / HISTORY (bottom) ┐
status bar: size · zoom · cursor x,y · ………………………… N steps (· "preview not applied yet")
```

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

---------------------------------------------------------------------------------------------------

## 6. Status (update every session)

**Done (v1 → v9):** Rust port; pipeline + undo/redo; 5 themes with orange modified sliders; icon rail; launcher/CLI open; proxy preview (Adjust, Fill, Paperize);
halftone/ASCII bases + ASCII un-stretch; transparency-aware Halftone; Paperize v2; Crop commit bug fixed; step records; **v9 UI shell: menu bar, big-button toolbar with Undo/Redo,
History dock (rollback list), status bar (size/zoom/cursor/steps), File ▸ Open Recent; old bottom chip strip and saved-files list removed.**

**Untested by the owner:** v8 and v9 (owner will test v9, which includes v8). Unit tests: 8 pass (+2 ignored perf/sample tests).

**Open / unverified:** Wayland behaviour of popups/menus; Pixelate/Outline/Watermark with transparent input; the new menu bar on Hyprland; `librsvg` presence on the owner's system.

---------------------------------------------------------------------------------------------------

## 7. ROADMAP — do exactly one round per session; each round ends with one zip for the owner to test

**Why this order:** history, projects, recents and recipes all need "every step recorded with tool + settings" (done). The UI is built on top of that.

### Round 4 — Tool blocks, customisation, finish the shell  *(NEXT)*
Goal: fix the tab clutter. Replace the per-tab toggles ("Tabs" menu) with **category blocks** the owner can organise.
* Left rail = blocks (Isolate, Adjust, Color, Effects, Style, Utility, + user-created). Each block has a header and large icon buttons.
* **Drag to reorder blocks and tools**, including moving a tool between blocks (GTK4 `DragSource`/`DropTarget`). Persist order/membership/visibility in `settings.json`/`Layout`
  (new fields must default sanely; unknown/new tools appear at the end of their default block).
* "Customize" mode (button/menu): show/hide tools, rename/add/delete blocks. A **Find tool** search box (filters the rail and the Tools menu) and optional pinned favourites.
* Remove the `Tabs ▾` toolbar button and the old tab-visibility popover once Customize works.
* History panel: clicking a step should also **restore that tool's controls from `HistoryEntry.params`** (the tab's `apply_preset_data`-style function) when it makes sense.
* Tool-options bar: slim row under the toolbar showing the active tool's icon+name and Process/Reset/Presets (inspector keeps the full controls).
* Acceptance: reorder survives a restart; hidden tools disappear from rail+menu; Ctrl+Z/History still work; no regression in themes (test dark/black/white).

### Round 5 — Files & projects
* Rename/replace the old saved-files list with **Recent files** (menu already exists; add a dock/list if useful) and **Projects = folders** under `~/Pictures/FileTools/`.
* **Save As** dialog: choose *default folder* / *existing project* / *new project*, name, format.
* **Project file** (`.ftproj`, a zip like paint.net's `.pdn`): `manifest.json` {version, app version, steps:[{tool, label, params}], current_step}, `original.<ext>` (embedded copy),
  optional cached step outputs, `thumb.png`, `result.png`. "Save Project" button writes it next to the exported image. Opening a project restores History and lets the owner roll back and re-edit.
  Replay = apply each step's `params` to its tool in order (reuses the preset mechanism). Add a schema `version` for forward compatibility.
* Persist the pipeline across restarts (optional "restore last session").

### Round 6 — Proper AI masking (not an eraser!)
Owner's explicit requirement: *"don't just paint a colour and delete everything in there"*. Design:
1. AI proposes a mask (rembg with `-om`, alpha-matting flags `-a -ae -af -ab` are already supported).
2. On the main canvas the owner paints **Keep** (green) / **Remove** (red) hints. Hints are *constraints that guide the AI*: build a **trimap** from the AI mask + hints and
   re-run alpha matting (pymatting via rembg) on the uncertain band so edges stay clean. Hints never delete pixels directly.
3. The mask is its own layer shown as an overlay (invert, grow/shrink, feather, opacity). Then apply: remove background / keep subject / blur background / crop to subject.
4. Keep the plain **Eraser/Restore brush** as a separate, clearly named tool.
5. Later options: SAM point/box prompts (`rembg -m sam -x`, large model download); object **inpainting** (separate tool, big model).
* Replace `widgets/mask_canvas.rs` usage in the Remove tab. Mask strokes live in image coordinates; store the mask as a PNG in the project.

### Round 7 — Recipes, batch, drag-and-drop
Save a whole pipeline (list of tool + params) as a recipe; run it on a folder (background thread, progress, CPU-capped); drag files onto the window; multi-file open.

### Round 8 — PDF workspace
A separate workspace (pages, not one image) sharing toolbar/Recent/Projects. Page grid with thumbnails; select, delete, extract, **reorder by drag**, merge, split, rotate; "replace or load alongside?" when
loading another PDF; batch load. Implement with `qpdf` + `pdftoppm`. Extras by priority: images↔PDF, compress (ghostscript), protect/unlock, page numbers/watermark, OCR (tesseract), metadata strip.

### Later / ongoing
* Engine: more tabs on the proxy preview; in-memory image buffers; then layers + pixel tools ("paint.net-lite": brush/eraser, selections, transforms, undo by pixel diffs, OpenRaster project format).
* Battery/CPU: no idle redraws/timers, lower thread priority, in-process effects instead of subprocesses.
* Paperize v3: seed/shuffle, per-edge control, corner folds, tape/pins, custom paper colour.
* Wire `Gradient` into the rail or drop it.

---------------------------------------------------------------------------------------------------

## 8. CLEANUP / THINGS TO CHANGE (do when touching the area)
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
* **v9** — UI shell: menu bar (File/Edit/View/Tools/Help, shortcuts, Open Recent), big icon toolbar with Undo/Redo, History dock with click-to-rollback (future steps dimmed), status bar
  (size/zoom/cursor/steps), `tools/interact.sh`, expanded NOTES. Old bottom chips + saved-files list removed.
* **v8** — Paperize rewritten (torn paper border + weathering); history entries carry `params` + `live`; undo fixes; Crop commit fix; Halftone transparency fix.
* **v7** — Paperize (layered, superseded); canvas checkerboard; accent-button label fix; `run_live_rust`/`spawn_work`.
* **v6** — orange modified sliders; switch/blue-border fixes; Fill proxy preview; Phosphor icons on the rail.
* **v5** — sound moved to Settings; halftone/ASCII bases; ASCII un-stretch; themes renamed (dark/black) + monochrome accents; Adjust proxy preview.
* **v4** — halftone/ASCII overlay; CPU-capped builds; NOTES.
* **v3** — theme rewrite; popover closing; CLI open. **v2** — OLED theme, launcher, perf fixes.
