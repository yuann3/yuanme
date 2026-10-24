# Image fixtures

Photos from Wikimedia Commons, each dedicated to the public domain under CC0 1.0, cut down and re-encoded with macOS `sips`, `ffmpeg` and `cwebp` so that between them they cover what the image pipeline must handle.

| File | Source | What it tests |
|---|---|---|
| `beach.jpg` | [Tropical-beach.jpg](https://commons.wikimedia.org/wiki/File:Tropical-beach.jpg), Brodie Vissers | A plain sRGB photo with no profile, 1600×1066 |
| `park.jpg` | [Baishamen Park beach - 01.JPG](https://commons.wikimedia.org/wiki/File:Baishamen_Park_beach_-_01.JPG), Anna Frodesiak | Stored 1440×1080 and turned by EXIF orientation 6, as a phone stores a portrait photo. It also carries a GPS tag and an sRGB ICC profile, which no output may keep |
| `fallingwater.jpg` | [Fallingwater - DSC05643.JPG](https://commons.wikimedia.org/wiki/File:Fallingwater_-_DSC05643.JPG), Daderot | Converted to Display P3 with its ICC profile, 720×1280 |
| `shaokao.webp` | [Holiday Beach shaokao area - 01.jpg](https://commons.wikimedia.org/wiki/File:Holiday_Beach_shaokao_area_-_01.jpg), Anna Frodesiak | Lossy WebP with alpha: the photo inside a transparent circle, 1200×900 |

Changing a fixture changes the hashes in `src/image/snapshots/`.
