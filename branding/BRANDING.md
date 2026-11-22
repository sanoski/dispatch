# Dispatch branding

Approved design: https://claude.ai/artifact/XNXEmKsAkS4tQ5xJSL14CY
(icon concept "D2. Crossing signal", settings window artboard "Settings window, rebranded")

## Files in this folder

| File                                                           | Use                                                             |
| -------------------------------------------------------------- | --------------------------------------------------------------- |
| `dispatch-icon.svg`                                            | Master app icon (crossbuck, mast, crossing lights, sound waves) |
| `dispatch-icon-1024.png`                                       | Source for `bun tauri icon` to generate every app icon size     |
| `tray-idle.svg`, `tray-recording.svg`, `tray-transcribing.svg` | Simplified tray icons, light strokes for dark taskbars          |
| `tray-*-dark.svg`                                              | Same, dark strokes for light taskbars                           |

Tray states work like a real crossing: idle = both lights off, recording = right light lit, transcribing = both lit.

## Colors

| Token              | Hex       | Use                                    |
| ------------------ | --------- | -------------------------------------- |
| Background         | `#0F1720` | Main window background                 |
| Sidebar / chrome   | `#0B121A` | Sidebar, title bar, footer             |
| Card               | `#142029` | Settings group cards                   |
| Border             | `#22323F` | Card borders and dividers              |
| Control border     | `#2D4050` | Buttons, dropdowns                     |
| Text               | `#E8EFF4` | Primary text                           |
| Muted text         | `#8FA3B3` | Section labels, footer                 |
| Accent (ice blue)  | `#7DD3FC` | Selected nav item, toggles, lit signal |
| Accent on light bg | `#0284C7` | Accent where the background is light   |
| Steel              | `#A9BCCB` | Icon mast and bars                     |
| Icon tile          | `#0E1A24` | App icon background                    |

Replace Handy's pink accent everywhere with the ice blue accent. Text on an ice blue fill uses `#0B121A`.

## Type

- Wordmark and headings: Space Grotesk 700
- UI text: Inter (400, 500, 600)
- Bundle the fonts locally; do not load them from Google at runtime.

## Changes in the app

1. Replace the pink "handy" logo at the top of the sidebar with the icon (36 px) plus the word "Dispatch" in Space Grotesk 700, 24 px.
2. Swap the pink accent for ice blue: selected sidebar item, toggles, focus rings, links.
3. Replace the waving-hand icon on General with a settings-sliders icon.
4. Rename the "Post Process" section to "Cleanup".
5. Footer shows "Dispatch v0.1.0" (start Dispatch's own version numbering).
6. Window title, tray tooltip, and About page say Dispatch. About page credits Handy by CJ Pais (MIT).
7. Regenerate app icons from `dispatch-icon-1024.png` with `bun tauri icon`, and replace the tray PNGs in `src-tauri/resources` by rendering the tray SVGs at the same pixel sizes as the existing files.
