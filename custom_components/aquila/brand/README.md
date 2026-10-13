# Aquila component branding

`brand/icon.png` is intentionally **not** committed here.

Home Assistant serves a custom integration's branding from
`custom_components/aquila/brand/` (a square PNG, at least 128×128, ideally
256×256 or 512×512). Committing a placeholder bitmap that is not a real icon
would ship a broken image, so this note stands in for the asset instead.

To add branding:

1. Drop a square PNG at `custom_components/aquila/brand/icon.png`
   (and optionally `logo.png`).
2. Use the Aquila radar mark (the add-on uses `mdi:radar` as its panel icon).
3. Keep the file small; it is served through the HA frontend.

A real icon is a release-packaging follow-up, not part of ticket 08.
