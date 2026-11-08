# The window's look, motion and words (UI)

How every part a person sees is drawn, how it moves and how it reads. This covers the native frame
and glass (AppKit), the strip, the side, the nav, the menus and suggestions (HTML in WKWebViews),
Settings, the mini and notices. Every visible change keeps these rules, in every state of
[the matrix](#the-matrix).

The rules come from Apple's Human Interface Guidelines and AppKit documentation for macOS 26,
the WWDC25 design sessions, measurements of the system on macOS 26.6, and WebKit's own sources.
Where Apple states a rule, it is followed. Where Apple gives no number, the number here is ours,
and it is marked *ours*. Don't change an Apple rule to fit a design. Change the design.

## The matrix

A change is done only when it looks right in each of these states, alone and together:

| Axis         | States                                                                  |
| ------------ | ----------------------------------------------------------------------- |
| Appearance   | light, dark (the person's theme, or the system's when it follows it)    |
| Transparency | glass at each level (Low, Standard, High), solid (Off)                  |
| Window       | windowed, full screen, tiled                                            |
| Focus        | key window, inactive window (another app or window is key)              |
| Accessibility | Reduce Transparency, Increase Contrast, Reduce Motion                  |
| Accent       | blue, another accent, Graphite                                          |

Ask the person to look, or take a `llx-desk window` screenshot of a tab. Compare against Finder,
Safari or System Settings side by side when no number decides.

## Layers

The window has two layers, and nothing mixes them.

- **Content.** Pages, terminals and the board are opaque. Their colours are their own, and the
  chrome never tints them. What an agent sees in a screenshot never depends on the glass.
- **Chrome.** The strip, the side, the nav, the menus and the mini float over the content, on the
  window's glass. Chrome is always in the window's theme, never in a page's colours.
- No glass on glass. A thing on the chrome (a row, a field, a button) uses a fill, never another
  material or blur.
- Floating chrome over content (menus, suggestions, the Events list) is solid with a hairline.
  It does not take a second blur: the page under it would show through as noise.

## Glass

The glass is one native `NSVisualEffectView` behind the whole window (`frame_macos.rs`), with
one neutral coat over it (an `NSBox` fill). The chrome pages are clear over it.

- **Material by purpose, not by look.** Light uses `sidebar` (7). Dark uses `hudWindow` (13).
  Blending is behind the window: the desktop shows through.
- **The material stays active** while another app is key. An agent may work in a window the
  person only watches, and an inactive material paints flat grey. The chrome shows that the
  window is inactive instead ([Focus and selection](#focus-and-selection)).
- **The coat is neutral**: near-black `#070707` or near-white `#f7f7f7`, at the alpha of the
  person's level (`frame::coat`). It is never a hue, a page's colour or a gradient.
- **No decoration over the glass.** No pools of colour, sheens, vignettes, noise or gradients.
  Apple says to remove custom backgrounds from toolbars and sidebars and let the material speak.
  Decoration reads as dirt on a light desktop, and as a fake wallpaper in full screen.
- **Solid when it must be.** The glass turns solid, as the person's Off is, in each of these:
  - Reduce Transparency;
  - Increase Contrast;
  - full screen, where no desktop lies behind the window to sample.

  One rule decides it: `frame::glass_level(chosen, solid)`.
- **Pages are told, never guess.** WebKit has no `prefers-reduced-transparency`. The app marks
  each page with attributes (below), and the pages style from them.

## What the pages are told

The app tells the pages the window's state through attributes on `<html>`. It sets them through
`Deck::say`, again on every refresh, so a page loaded again has them back. A page never infers
these from its own events.

| Attribute      | Meaning                                                              |
| -------------- | -------------------------------------------------------------------- |
| `data-glass`   | The window is glass: the page is clear, the material shows through.  |
| `data-reduced` | Reduce Transparency: every ground is solid.                          |
| `data-full`    | Full screen: no window buttons, so the crumbs take their room.       |
| `data-part`    | Which part this page is: `strip`, `side` or `nav`.                   |

Add a state here, not as a page's own guess. WebKit's `:window-inactive` matches, but it never
restyles an element already drawn, so the window's focus must be told the same way.

## The frame

- **The titlebar** is an empty unified-compact `NSToolbar`. AppKit centres the window's buttons
  in it and keeps them there through every resize. The strip draws the toolbar's contents under
  it.
  - `fullSizeContentView`, a transparent titlebar, a hidden title, no separator.
  - The strip's bar is 38 pt (`strip::BAR`). The nav is 40 pt (`strip::NAV`).
- **The window's buttons** are the system's: 14 pt, 23 pt apart, 12 pt from the left edge in a
  compact toolbar. The strip keeps 78 pt clear for them (`html[data-glass] #bar`). Never cover
  them with a page that takes their clicks.
- **Full screen** is the system's (the green button, ⌃⌘F, tiling):
  - the empty toolbar is let go, so no blank band covers the strip;
  - the pages are told `data-full`;
  - the glass turns solid;
  - leaving it puts back the frame the person had.

  Don't build a full-screen mode of our own.
- **Tiling** is the system's. The window allows it (`FULL_SCREEN_PRIMARY`).
- Don't draw window controls of our own. A copy that doesn't match exactly makes the app feel
  broken.

## Colour

All colours are tokens in `frame.css`, the one sheet every page is dressed in. A page doesn't
define colours of its own beyond its `--ground`.

- **Ink** is one ladder from the page's text colour:
  - `--ink` is 90%, `--ink-2` 68% (60% in dark), `--ink-3` 40%;
  - a step firmer under Increase Contrast.

  Text up to 17 pt keeps 4.5:1 against what is behind it; aim for 7:1 for small text.
- **Fills** mix `currentColor`, so one rule suits light and dark: `--ov-1` 5% to `--ov-4` 20%.
- **The accent** is the system's (`-apple-system-control-accent`). It is used sparingly: the
  selected row, the primary action, a focus ring. Never two tinted controls side by side.
- **Each custom colour** has a light, a dark and an Increase Contrast value. A colour without
  all three is a bug.
- **Never colour alone.** A state also changes shape, a glyph or words.
- Prefer WebKit's system keywords where one fits (`-apple-system-label`,
  `-apple-system-secondary-label`, `-apple-system-separator`,
  `-apple-system-selected-content-background`,
  `-apple-system-unemphasized-selected-content-background`, `-apple-system-alternate-selected-text`).
  They track the system per release and per appearance. Increase Contrast reaches the pages only
  through `@media (prefers-contrast: more)`.
- Status colours (red, orange, green) are for status only, never for decoration.

## Type

- **The system font** (`--ui-sys`: `-apple-system`), or the person's interface font
  (`--ui-font`). No manual `letter-spacing`: the system tracks SF at every size.
- **Sizes** follow macOS's text styles:

  | Style    | Size / line | Weight   | Use                                       |
  | -------- | ----------- | -------- | ----------------------------------------- |
  | Body     | 13 / 16     | Regular  | rows, menus, fields, buttons              |
  | Headline | 13 / 16     | Bold     | a heading inside a panel                  |
  | Callout  | 12 / 15     | Regular  | the strip's compact text                  |
  | Subhead  | 11 / 14     | Regular  | a row's second line, captions, tooltips   |
  | Footnote | 10 / 13     | Regular  | the smallest text allowed; badges         |
  | Title 3  | 15 / 20     | Semibold | a Settings section                        |

- Nothing under 10 pt. Avoid Light, Thin and Ultralight. Hierarchy is weight, size and ink,
  together.
- A long address or path truncates in the middle, and its whole text is in a tooltip.

## Size and shape

- **Hit targets** are at least 20 × 20 pt, and 28 × 28 by default. Adjacent buttons in a bar touch:
  their hit regions are contiguous, so a hover never flickers in a gap.
- **Control heights** follow macOS 26: small 20, regular 24, large 28 pt. The strip's buttons
  are 28.
- **Rows**:
  - side tab rows 32 to 36 pt (13 pt text, a 16 to 18 pt icon, 8 pt inset);
  - menu and suggestion rows 24 pt, with 5 pt padding above and below the list;
  - a separator is 11 pt tall with a hairline.
- **The side** is 248 pt by default, 200 to 480 pt when dragged, and never more than half the
  window. Shut, it is a 52 pt rail.
- **Radii** are tokens (`--r-1` 4, `--r-2` 6, `--r-3` 8, `--r-4` 10, `--r-pill`). Nested shapes are
  concentric: the inner radius is the outer radius minus the padding between them, never more.
  A capsule's radius is half its height.
- **Hairlines** are 0.5 pt at 2× (`--sh-hair`), a full point under Increase Contrast.
- **Spacing** keeps a 4 pt grid: 8 between related things, 12 to 20 between groups.
- **Elevation** has three steps, each with the hairline:
  - `--sh-hair` for every floating surface;
  - `--sh-pop` for menus, suggestions and popovers;
  - `--sh-lift` for a carried tab and notices.

  Dark needs about twice the shadow alpha, and its hairline carries the edge.

## Focus and selection

- **Hover** lights a row or a button at once, and the light goes out over `--t-out`. Only a
  colour moves. Rows in a list don't otherwise move on hover.
- **Press.** Every custom button has a pressed state (`--ov-3`), shown at once.
- **Selection:**
  - The shown tab's row is the accent with white words only while its list has focus and the
    window is key.
  - Otherwise the selection is grey (`-apple-system-unemphasized-selected-content-background`)
    and its words keep the normal ink.
  - The keyboard's pick and the pointer's hover are never the same look.
- **The inactive window recedes:**
  - glyphs drop to about 60% opacity;
  - selection turns grey;
  - hairlines lighten.

  The material stays as it is ([Glass](#glass)).
- **The focus ring** shows for the keyboard only (`:focus-visible`), around text fields and
  search fields. Lists show their pick by the row's highlight, not a ring.
- **The keyboard reaches everything:**
  - Tab and Shift-Tab move through the controls;
  - the arrow keys move inside a list or a menu;
  - Return and Space act;
  - Esc closes a menu and gives focus back to what opened it.
- Standard text editing works in every field: ⌘A, ⌘C, ⌘V, ⌘X, ⌘Z, and Option and Command with
  the arrows.
- The arrow cursor is used over controls and rows, the I-beam over editable text, and the
  pointing hand only over links. Chrome isn't selectable (`user-select: none`).

## Menus, suggestions and notices

- **Menus open in place** from what opened them (`transform-origin` at the anchor), with no
  travel. Suggestions appear as the person types, without an entrance animation.
- **Rows** are 24 pt with 13 pt text. The pointer lights a row in a tint of the accent
  (`--acc-2`). The keyboard's pick is the accent, solid, with white words (`--acc-sel`).
- **An item that opens more** (a window, a dialog, a question) ends with `…` (U+2026). An item that
  acts at once doesn't.
- **Groups** are split by separators, at most about three in a context menu. Submenus go one
  level deep.
- **Shortcuts** show in the menu bar's menus, never in context menus.
- **Unavailable items** are dimmed in menu bar menus and hidden in context menus.
- **Every action has a home in the menu bar.** A context menu or a toolbar button is a shortcut
  to it, never its only place.
- **Notices** don't carry the only copy of something important:
  - one with a timer pauses while the pointer or focus is on it, and has a close button;
  - they keep off the window's bottom edge;
  - they speak to VoiceOver (`role=status`, or `alert` for errors).

## Words

- **Title case**: menu items, buttons, tab and section names, column headings, panel titles
  ("Fonts").
- **Sentence case**: tooltips, field captions, explanations, and alert text that is a sentence.
- **Buttons** start with a verb ("Open Folder", not "OK"). Use "Cancel" to cancel, and avoid
  "Yes" and "No".
- **Tooltips** name what the control does, start with a verb, stay under about 70 characters
  and have no final period.
- **No "we"**, and no "your" or "my" where nothing is lost without it ("Projects", not "Your
  Projects"). An error says what happened near where it happened, without blame or codes.
- **Settings**, never "Preferences". It opens with ⌘, from the app menu.
- An empty state says what to do next, with a button.

## Motion

Motion says what changed and where it went. It is never decoration and never makes the person
wait.

- **Only `transform` and `opacity` move.** Never animate width, height, position, margins,
  `box-shadow` or a blur radius. To change elevation, cross-fade a shadow layer.
- **Frequent things don't animate:**
  - typing into suggestions;
  - moving between buttons;
  - switching tabs from the keyboard.
- **Everything can be interrupted.** A new click retargets from where the thing is now. Input is
  never blocked while something moves.
- **Direct manipulation follows the pointer one to one**, with no transition. That covers a
  carried tab, the side's edge and the split's divider. The spring starts on release.
- **Springs move things. Curves fade them.** Start at bounce 0. Add bounce (at most 0.15, and
  0.3 for a release with real momentum) only when the gesture carried momentum. A tap never
  overshoots.
- **Exits are faster than entrances** and fade only, without travel.
- **Native windows keep the system's animations.** A page inside a native motion doesn't run a
  second one of its own.

The tokens (`frame.css`) set the timings; a component never picks its own duration:

| Token          | Use                                              | Timing                              |
| -------------- | ------------------------------------------------ | ----------------------------------- |
| `--t-out`      | hover off, a colour going out                    | 120 ms, `--ease`                    |
| `--t-move`     | short moves                                      | 200 ms                              |
| `--spring`     | bounce 0: open from an anchor, a side, a layout  | `linear()` of a damped spring       |
| `--spring-b`   | bounce 0.15: siblings making room, a tab landing | `linear()` of a damped spring       |

- A spring's CSS duration is its settling time: about 1.47 × its perceptual duration at bounce
  0, and 1.39 × at 0.15. Design by the moment it has visibly arrived (90%, at 0.62 ×).
- Opacity is quicker than the move it goes with:
  - an entrance's fade takes about 100 ms;
  - an exit's fade 120 ms;
  - a cross-fade 150 ms.
- **Reduce Motion**:
  - nothing travels, scales or bounces; each move becomes a cross-fade of about 150 ms;
  - press feedback is colour only;
  - repeating motion (pulses, shimmer, spinners) stops at a still state;
  - dragging still follows the pointer.

  `frame.css` already limits every transition to colours and opacity under it.

## Settings

- **One window's tab**, opened by ⌘, from the app menu. There's no toolbar button for it.
- **Sections** are named in title case. Settings returns to the last section shown.
- **It holds few settings.** It never repeats one the system already has, except the
  window's own theme and transparency.
- **Controls by kind:**
  - a switch for one emphasized choice;
  - checkboxes for a hierarchy;
  - radio buttons or a pop-up for three or more exclusive options.

## Known gaps

What the code doesn't meet yet. Fix them through the mechanisms above, and remove a line here
when its gap is closed.

- The pages aren't told whether the window is key. An inactive window's selection stays the
  accent.
- The chrome WebViews don't set `_useSystemAppearance`. Without it, WebKit gives
  `-apple-system-control-accent` as fixed blue, whatever accent the person chose, and the focus
  ring's colour is fixed too.
- The springs in `frame.css` are 15 uniform stops, off by 2 to 3%. Regenerate them from the
  closed-form spring, simplified to within 0.1%, about 36 stops.
- Pages use hex and mixed colours where a system keyword fits (labels, separators, selection).

## Sources

- [Human Interface Guidelines](https://developer.apple.com/design/human-interface-guidelines):
  - Materials, Color, Dark Mode, Typography, Layout, Accessibility;
  - Toolbars, Sidebars, Windows, Going Full Screen, Panels;
  - Menus, Context Menus, Pop-up Buttons, Search Fields, Text Fields, Lists and Tables;
  - Focus and Selection, Keyboards, Pointing Devices, Motion, Writing, Settings, Alerts.
- AppKit: [NSVisualEffectView](https://developer.apple.com/documentation/appkit/nsvisualeffectview),
  [Adopting Liquid Glass](https://developer.apple.com/documentation/technologyoverviews/adopting-liquid-glass),
  [accessibilityDisplayShouldReduceTransparency](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducetransparency).
- WWDC25 [219 Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/),
  [310 Build an AppKit app with the new design](https://developer.apple.com/videos/play/wwdc2025/310/),
  [356 Get to know the new design system](https://developer.apple.com/videos/play/wwdc2025/356/).
  WWDC23 10158 Animate with springs, and WWDC18 803 Designing Fluid Interfaces.
