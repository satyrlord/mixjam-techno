# Application icon

The product name header, the native window, and the Windows executable use the
canonical 256 by 256 transparent RGBA image `mixjam-techno.png`. Edit this
source with raster tools. `src/main.rs` embeds the PNG in the native viewport.
`build.rs` links the ICO into the executable. The ICO contains 16, 24, 32, 48,
64, 128, and 256 pixel images. It covers the window, taskbar, and Windows file
icons.

Run `python tools/assets/render-app-icon.py` to regenerate the ICO from the PNG.
The helper also prepares the 40-pixel header PNG offline with LANCZOS
resampling, in the `40` subfolder. It serves the nominal 40-point header at
100% display scaling, the only scale in scope. The UI draws it at exactly one
image pixel per display pixel. The header does not resize the image at run
time.
The helper does not change the canonical PNG. Run it with `--check` to compare
the PNG format, transparency, antialiased edges, the header PNG, and every ICO
size with a fresh derivation. The helper uses only Pillow 12.3.0 from
`local-data/icon-renderer`. You do not need Python or Pillow to build or run
the app.

To install the renderer in a new checkout, run:

```powershell
python -m pip install --target local-data/icon-renderer Pillow==12.3.0
```

The PNG is the only input to the ICO helper.
