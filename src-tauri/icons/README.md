# App Icons

`icon.png` is Donna's source app icon (1024x1024), referenced by `../tauri.conf.json`.

To generate the full platform icon set (`.ico`, `.icns`, and the various PNG sizes that
some targets require) from this source, run:

```bash
npm run tauri icon src-tauri/icons/icon.png
```

This produces the complete icon set Tauri expects for packaged builds across macOS,
Windows, and Linux. For `tauri dev` and macOS bundles, the single `icon.png` is enough
to get started.
