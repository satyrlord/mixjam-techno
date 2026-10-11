# Application icon

The title branding, native window, and Windows executable use the canonical
256 by 256 transparent RGBA image `mixjam-techno.png`. Edit this source with
raster tools. The PNG is embedded in the native viewport. The ICO contains 16, 24, 32, 48,
64, 128, and 256 pixel images and is linked into the executable by `build.rs`.
This covers the window, taskbar, and Windows file icons.

Run `python tools/assets/render-app-icon.py` to regenerate the ICO from the PNG.
The helper also prepares header PNGs offline with LANCZOS resampling at 40, 60,
and 80 pixels, in matching numbered subfolders. These cover 100%, 150%, and 200%
display density for the nominal 40-point header. The UI chooses the closest prepared size and draws it
at exactly one image pixel per display pixel; unusual display scales can vary
its logical size slightly. The header performs no runtime image resizing.
The helper does not change the canonical PNG. Run it with `--check` to verify
the PNG format, transparency, antialiased edges, every display PNG, and every ICO size against a
fresh derivation. The tool uses only Pillow 12.3.0 from
`local-data/icon-renderer`. Neither Python nor Pillow is needed to build or run
the app.

To install the renderer in a new checkout, run:

```powershell
python -m pip install --target local-data/icon-renderer Pillow==12.3.0
```

See `assets/donors/ravefold/README.md` for source provenance and notices.
The artwork originated at RaveFold commit
`5965621be6ac2ea5330db9fb231755860617a285`. Its historical authoring-source SHA-256
is `b1e9954dcd1a639abf5eb0e18e320c76b9a08b39126e83801120cfb4b45b0d19`.
The canonical PNG adopted on 2026-10-11 has SHA-256
`0cedfad7c01e0f2a472e569ee66b96e8c9de87c197e0aeb137b00b3cd4ef17d0`.
These hashes document origin; the PNG is the only input to the ICO helper.
