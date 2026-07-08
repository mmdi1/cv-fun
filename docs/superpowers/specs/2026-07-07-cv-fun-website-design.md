# cv-fun Website Design

## Goal

Create a concise, polished product homepage for `cv-fun` in the `website` Vue app.
The page should feel like an external product launch page, not an internal roadmap.

## Source Context

The product plan describes `cv-fun` as a macOS/Windows clipboard productivity tool.
Its first milestone is a Maccy-style clipboard history panel, with later parsing features for copied timestamps, dates, English words, URLs, JSON, Base64, UUIDs, and colors.

The homepage should communicate these product principles:

- Lightweight: background clipboard recording should stay quiet.
- Keyboard-first: search, select, copy, paste, pin, and delete should work without a mouse.
- Local-first: clipboard history is stored on the user's machine by default.
- Extensible: copied content parsing is rule-based and can grow over time.
- Configurable: history, privacy filters, hotkeys, and parser rules can be adjusted.

## Chosen Direction

Use the "minimal product launch page" approach:

- Black/white theme system with a visible theme toggle.
- Thin lines, restrained surfaces, and high-contrast typography.
- A first-screen product signal built around a realistic clipboard panel preview.
- Copy that presents `cv-fun` as a focused, useful product.
- No colorful gradients, marketing illustration, or game-like visual language.

The reference project `/Users/chsoxy/code/fmo.re/nfun` is used for structural inspiration only:

- Data-driven content where useful.
- A fixed top navigation.
- A strong hero section.
- Compact repeated sections.
- A footer that closes the page cleanly.

## Page Structure

The single-page homepage will include:

1. Fixed top navigation
   - Brand: `cv-fun`
   - Anchor links: history, parsing, privacy, roadmap
   - Theme toggle: switches between black and white themes.

2. Hero
   - Headline: positions `cv-fun` as a lightweight clipboard assistant.
   - Supporting copy: local-first, keyboard-first, clipboard history plus parsing.
   - Primary action: jump to feature overview.
   - Secondary action: jump to roadmap.
   - Product preview: a mock clipboard history panel with search, pinned entries, normal entries, parser result details, and shortcut hints.

3. Core feature cards
   - Clipboard history always at hand.
   - Keyboard-first workflow.
   - Smart copied-content parsing.
   - Privacy controls and local storage.

4. Parsing examples
   - Timestamp to formatted local time.
   - JSON validation and formatting.
   - URL breakdown.
   - Color conversion.

5. Privacy/local-first section
   - Explain pause recording, ignore next copy, sensitive pasteboard type filtering, and local storage defaults.

6. Short roadmap
   - M1: clipboard history foundation.
   - M2: keyboard operations.
   - M3: parser panel MVP.
   - M4: word translation.
   - M5: settings and persistence polish.

7. Footer
   - Short product summary and anchor links.

## Visual Design

The homepage should use only black, white, and neutral gray values.

White theme:

- Background: near-white.
- Text: near-black.
- Cards and preview panels: white with light gray borders.
- Accent: black-filled controls and tiny monochrome marks.

Black theme:

- Background: near-black.
- Text: near-white.
- Cards and preview panels: very dark gray with muted borders.
- Accent: white-filled controls and tiny monochrome marks.

Layout requirements:

- The hero should show a hint of the next section on common desktop and mobile viewport heights.
- Cards should use restrained radii of 8px or less.
- No nested UI cards.
- No decorative gradient orbs, bokeh blobs, or unrelated illustrations.
- Text must fit within controls and cards on mobile.
- The page should feel like an efficiency tool, not a SaaS marketing splash page.

## Implementation Boundaries

Implement inside `website` only.

Expected file changes:

- Replace the Vue starter content in `src/App.vue`.
- Replace starter CSS in `src/assets/base.css` and `src/assets/main.css`.
- Simplify `src/views/HomeView.vue` or remove route dependence from the rendered homepage.
- Optionally add `src/data/siteContent.ts` and small presentational components if it keeps the page readable.
- Remove unused starter imports from the rendered app path.

No backend behavior, downloads, analytics, or live clipboard functionality is required for this homepage.

## Verification

After implementation:

- Run the website build or type-check command available in `website/package.json`.
- Start the Vite dev server if dependencies are installed.
- Verify desktop and mobile rendering with browser screenshots when possible.
- Check both black and white themes.
