# File Tools — project notes (handoff document)

Read this first. It is written so any AI assistant or person can continue the project
without the old chat history. **Update the "Status" and "Changelog" sections at the end of every work session.**

## 1. What this is
A personal **desktop image toolkit** (GTK4 + Rust) for Linux. Pipeline of tools that each
take the previous tool's output: AI background removal (rembg/backgroundremover), adjust,
crop, fill background, palette, duotone, halftone, ASCII, pixelate, outline, watermark,
convert, plus a PDF tool. Long-term goal: grow into a light "paint.net-style" editor.

Owner's setup: CachyOS (Arch), Hyprland + Caelestia shell, **fish** shell, Ryzen 5 5500U laptop
(12 threads, integrated GPU, **no GPU compute**), 1080p screen. Owner is learning; explain briefly,
avoid jargon. Owner is on a **free AI plan with tight token limits** → see section 8.

## 2. Build / run
```fish
./cool.sh run            # debug run, CPU-capped (laptop gets hot otherwise)
./cool.sh check          # fast compile check — use this after every edit
./install.sh             # release build + installs launcher "File Tools" + icon
file-tools photo.png     # open an image directly (also "Open With" in file manager)
```
- Rust 1.91+ / GTK 4.12+ (crate `gtk4 0.9`, feature `v4_12`). Owner has newer; keep code compatible with 1.91.
- `.cargo/config.toml` limits parallel jobs to 4. `cool.sh` adds a systemd CPU quota (default 50%).
- **Do not change `[profile.dev]` opt-levels casually**: it forces a full dependency rebuild (CPU spike).
- Runtime tools: `magick` (ImageMagick 7), `~/bg-remover-env/bin/{rembg,backgroundremover}` (python venv,
  called by absolute path), `wl-copy`/`wl-paste`, `sox`, `exiftool`, `qpdf`, `pdftoppm`, `fc-match`.

## 3. Verifying UI changes without a screen (important for AI assistants)
`tools/shot.sh <theme> out.png [image]` runs the app in Xvfb and screenshots it. Setup on Ubuntu/Debian:
`apt install xvfb imagemagick openbox xdotool fonts-dejavu-core adwaita-icon-theme`.
Use `xdotool` for clicks. Always test dark **and** light system themes (`SYSDARK=0`). The owner's real
environment is Wayland/Hyprland, which this cannot reproduce (popup/focus behaviour differs).
Unit tests (5): `surface_from_rgba`, `image_base_modes`, `ascii_layer_matches_image_aspect` (needs `magick`), `proxy_only_for_big_images…` (halftone pixel checks). Compiling tests is slow/hot — use `./cool.sh test`.

## 4. Architecture map (src/, ~8k lines)
| File | Role |
|---|---|
| `main.rs` | just calls `app::run()` |
| `app.rs` (1.4k) | `AppState` (shared via `Rc`), window layout, rail, top bar, `register_page!` macro that registers each tab, history/pipeline logic, CLI file arg (`INITIAL_FILE`) |
| `tabs/base.rs` (600) | `BaseTab`: shared tab UI (title, Process button, status, presets, tracked sliders), **live preview scheduling**, `spawn_processing*` (runs ImageMagick commands off the UI thread) |
| `tabs/*.rs` | one file per tool (adjust, crop, fillbg, palette(+container), duotone, halftone, ascii, pixelate, outline, watermark, gradient, convert, remove) |
| `widgets/preview.rs` (940) | the big canvas: zoom/pan, original-vs-result fade, eyedropper, crop overlay; `surface_from_rgba` (RGBA→Cairo) |
| `widgets/mask_canvas.rs` | small brush editor in the Remove tab (draws mask strokes over the image, checkerboard for transparency) |
| `settings_panel.rs` | `Settings` (JSON in `~/.config/file-tools/settings.json`), settings popover, **theme system** |
| `settings.rs` | layout/tab-visibility + per-tab presets storage |
| `icons.rs` | bundled Phosphor SVG icons (`assets/icons/*-symbolic.svg`, MIT) compiled in, written to `~/.cache/file-tools/icons` at startup, tinted by GTK symbolic recolouring. Needs `librsvg` (pixbuf SVG loader); `tab_icon()` returns `None` without it → text-only buttons |
| `tabs/paperize.rs` | **Paperize** (paper cutout): pure-Rust renderer (k-means colour layers → stacked sheets with wobbly edges, drop shadows, edge highlights, optional paper grain + weathering). Transparent source areas stay transparent. Uses `BaseTab::run_live_rust` |
| `util.rs` | helpers: `run_capture`, `resolve_font`, clipboard, colour math (hex/HSL/tints/shades), palette extraction, done-sound, eyedropper button |
| `config.rs` | paths (`~/.config/file-tools`, tmp dir, save dir `~/Pictures/FileTools`, bg-remover binaries) |

### Key concepts
- **Pipeline/history**: `AppState.history` is a list of `HistoryEntry {label, output_path}`. Each tool reads the
  head's file, writes a new temp file, and `commit_history()` appends (or **replaces the last entry if the same tool ran again**). Chips in the bottom strip let you rewind; Ctrl+Z / Ctrl+Shift+Z undo/redo.
- **Live preview**: a tab with `set_live_preview(true, ms)` fires its live trigger after a debounce
  (`schedule_live_process`; if a job is running the timer retries). Two flavours:
  0. *Rust jobs* (**Paperize**, v7): `BaseTab::run_live_rust(app, make)` / `run_rust_full`; `make(full)` reads the controls and returns a `RustJob`
     (`(input, scale) -> output path`, `Send`) executed through the generic `BaseTab::spawn_work` (spinner/status/commit handling shared with the
     ImageMagick path). Same proxy + deferred-commit behaviour as flavour 2. Preview 1280px ≈0.4s vs full 6MP ≈3s (debug build).
  1. *Classic* (most tabs): trigger = `run()` → `spawn_processing` → full-res **commit** every tick (replaces last same-tool history entry).
  2. *Proxy* (**Adjust only so far**, v5): trigger = `run_live()` → `BaseTab::run_live_proxy(app, build)`. For images with
     long side > 1800px (`util::PROXY_TRIGGER`) it renders on a cached 1280px copy (`util::preview_proxy`, returns
     `(path, scale)`) and shows it via `show_result_preview` **without committing**; it registers
     `AppState::set_pending_commit(closure)` that renders full-res synchronously + `commit_history`.
     `AppState::flush_pending()` runs it; it is called by `save_as`, `copy_result_image` and tab switches.
     `drop_pending()` (also bumps `live_gen`, which makes late preview results get ignored) is called by
     `commit_history`, `rewind_to`, `redo`, `set_universal_input`, `load_from_history`.
     Small images fall back to the classic path. Pixel-valued params (blur/sharpen/vignette) are multiplied by `scale`.
     **To convert another tab:** make its `build_cmd` take `scale`, add a `run_live` like Adjust's, point its live trigger
     in `app.rs` at it. Converted so far: **Adjust, Fill Background**. Not suitable: Duotone (already in-process Rust),
     Gradient (generator: output size comes from its own sliders). Others (outline, pixelate, watermark…) have pixel-valued
     params — scale them by `scale` when converting.
- **Processing model**: most tabs build `magick` command lines and run them as subprocesses on a blocking
  thread (`gio::spawn_blocking`) → temp PNG → loaded for display. Halftone renders in pure Rust (`image` crate).
  ASCII renders text then `magick label:@file`. Remove runs rembg/backgroundremover (not live).
- **Themes**: `settings_panel.rs` → `palette(theme)` (colours) + `THEME_CSS` template (`$bg`, `$accent`… tokens) +
  `apply_color_scheme()` (forces GTK light/dark variant so built-in widgets match). 5 themes:
  **dark** (grey), **black** (pure #000, OLED), **white**, blue, red. dark/black/white use monochrome accents.
  Old names were migrated once (`Settings.theme_v`: old "black"→"dark", old "oled"→"black"). CSS overrides GTK named colours (`theme_*`, `accent_*`) so sliders/checks follow the accent.
  Canvas background/grid come from `palette().canvas` and luminance-adaptive grid in `preview.rs`.
- **Settings popover** is non-autohide on purpose (some Wayland compositors never dismiss GTK popups);
  closes via ✕, Esc, click-outside, or gear toggle (handlers attached to the window on first map).
- **Image loading**: `preview.rs::load_surface` decodes in-process via `image` (fast path), falls back to
  `magick` for formats `image` can't read (heic, svg, psd…).

## 5. Status
**Done (v1→v7):** Rust port; pipeline + history + undo/redo; live preview; tab organiser; 3→5 themes;
settings popover fixes; faster image loading; dev-profile optimisation; dropped-update fix; ASCII font via
fontconfig; launcher + icon + `install.sh`; CLI file open; transparent checkerboard in mask preview;
CPU-capped build scripts; screenshot tool; halftone/ASCII image-base overlay, ASCII un-stretch, sound moved to Settings, theme rename + monochrome accents,
proxy live preview for Adjust + Fill (engine step 1), orange "modified" sliders, readable switches, rail icons, canvas checkerboard behind transparency, Paperize tool.

**Ideas for Paperize v2:** seed/"shuffle" for the wobble + stains, paper colour/tint picker, cast shadow direction, per-layer colour override, embossed paper fibre relief, torn-edge mode, SVG/layered export.

**Open/unknown:** owner hasn't run the full test checklist; Wayland popup fix unverified on real hardware;
`next_page` unused-assignment warning in `app.rs` (harmless).

## 6. Roadmap (in priority order — reasoning included)
1. ~~Halftone/ASCII overlay mode~~ **DONE in v4.** Halftone & ASCII share the same base options: None (transparent) / Black & white / Pure black / Original image
   (+ White; halftone also Custom colour), "Image opacity %" for the two image bases, halftone "Dots use image colors". *Not done:* per-glyph colour in ASCII
   (needs coloured text rendering), brightness/contrast applied to the base layer.
2. **Engine (IN PROGRESS — step 1 done: proxy preview + deferred commit for Adjust, see §4).** Remaining: convert duotone/fillbg/gradient;
   then in-memory images.
   *Original plan:* keep images in memory as RGBA buffers
   (`Document`/`Layer`), port cheap per-pixel effects (adjust, duotone, fill, gradient, pixelate, vignette) to
   pure Rust `fn(&RgbaImage,&Params)->RgbaImage`, split **preview (downscaled proxy, e.g. ≤1280px) from commit
   (full-res)**, "latest wins" job scheduling. Keep `magick`/rembg only for heavy work. Pixel-valued params
   (blur radius, outline px, pixelate block) must be scaled by the proxy factor.
3. **UI revamp (paint.net/Photoshop-like)** — *step 1 done in v6: icons on the tool rail.* Remaining: status bar, tool-options bar, dock panels, toolbox/effects split. Plan: menu bar + tool-options bar, left toolbox, canvas with checkerboard,
   right dock panels (History, Layers, Colors, Properties), status bar (zoom, size, cursor xy). Split *tools*
   (crop, mask brush, eyedropper, watermark) from *effects/adjustments* (menu → inspector).
   **Icons:** bundle an MIT/ISC SVG set (Lucide or Tabler) in the binary, tint to theme colour in code;
   do not rely on the system icon theme or emoji (that is why earlier attempts failed). Keep short labels + tooltips.
4. **Recipes + batch:** save a whole pipeline as a recipe, run it on a folder. Drag-and-drop files onto the window.
5. **Packaging:** `install.sh` + `.desktop` exist (personal use). AppImage not needed.
6. **Pixel editing (long-term):** layers, brush/eraser, selections (marquee/lasso/wand), transforms, undo by pixel diffs,
   OpenRaster (.ora) project format. Scope target = "paint.net-lite", not Krita/GIMP. Needs step 2 first.
7. **Battery/CPU:** process only on change, no idle redraws, lower thread priority, in-process effects (no subprocess churn).

## 7. Decisions & non-goals
- Stay on GTK4 (Wayland-native, already invested); keep the engine independent of UI so a UI swap stays possible.
- No GPU compute (ROCm attempt was removed). rembg runs on CPU.
- Don't recommend paid AI tools; free options (Aider/OpenCode + free API keys, Gemini CLI — check current limits) are fine.
- Local LLMs are a poor fit (CPU only).

## 8. Working agreement for AI assistants (token-saving)
- **Edit files in place and return a zip or only the changed files**; never paste whole files back and forth.
- Compile-check every change (`./cool.sh check`; if you have a sandbox, install rustc + libgtk-4-dev + libcairo2-dev).
- Batch related changes (3–5 per round), but keep each batch compilable.
- Prefer targeted `grep`/line ranges over reading whole files. Don't re-verify things that already passed.
- Keep replies short: what changed, how to apply, what to test. No long recaps.
- Ask at most one question at a time; make a sensible default instead of asking when possible.
- Owner's text preferences: plain language, short, fish-shell commands.

## 9. Changelog
- **v3** — theme system rewrite (palette, light/dark scheme, named-colour overrides, OLED grid), popover closing,
  transparent preview checkerboard, CLI open, `Open With` desktop entry.
- **v2** — OLED/black themes, dev opt-level, dropped live updates fixed, in-memory image load, ASCII font fix, launcher.
- **v7** — **Paperize** tool + `run_live_rust`/`spawn_work`; canvas checkerboard for transparent areas (fixes "halftone disappears" — black dots on a black
  canvas); accent-button label colour fix (Process text was unreadable on black/dark).
- **v6** — modified sliders turn orange (`Palette.modified`), switch knob contrast, hard-coded GTK blue on slider/progress/suggested-action overridden,
  Fill uses proxy preview, bundled Phosphor icons on the rail (with no-SVG-loader fallback).
- **v5** — Remove tab sound controls removed (Settings → Sound now drives the done-sound); halftone/ASCII bases reworked;
  ASCII un-stretched (measured cell size + padding, trailing-newline blank row removed, base is cover-cropped not stretched);
  `util::image_dimensions` reads headers in-process; themes renamed (dark/black) + monochrome accents; engine step 1 (Adjust proxy preview).
- **v4** — halftone/ASCII overlay (image-based base layers, colour-from-image dots); CPU-capped build (`cool.sh`, `.cargo/config.toml`), thin LTO, `tools/shot.sh`, this file.
