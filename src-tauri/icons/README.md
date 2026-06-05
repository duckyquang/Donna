# App Icons

This folder holds the application icons referenced by `../tauri.conf.json`.

Before the first packaged build (Phase 1), generate the icon set from a single source
PNG using the Tauri CLI:

```bash
npm run tauri icon path/to/source-icon.png
```

This produces `icon.png`, `icon.ico`, `icon.icns`, and the various platform sizes Tauri
expects. They are intentionally not committed yet during Phase 0.
