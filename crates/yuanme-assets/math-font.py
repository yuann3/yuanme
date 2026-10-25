"""Builds src/font/yuanme-ncm-math.otf, the Engine's math font, and its NOTICE.

    python3 crates/yuanme-assets/math-font.py

It needs git and FontForge's `fontforge` module (`apt install python3-fontforge`).
CI's math-font job runs it on ubuntu-24.04 and fails if the committed files differ
from what it writes, so the font is always the one this script makes from the commit
below (#27, #52). Another FontForge than Ubuntu 24.04's may write other bytes.

The font is New Computer Modern Math, patched for MathML by math-core-fonts, whose own
build is the `apply_patches` called below. GUST Font License asks that a derived font
be renamed, and LPPL 1.3c, which it is under, asks for a notice of the changes in it,
for information on how to get the original, and that it not imply its authors support
it. This script does all three (ADR 0014).
"""

import hashlib
import os
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path

REPOSITORY = "https://github.com/tmke8/math-core-fonts"
REV = "2af58a8fcfd00dd250bdf9b67caaa8bc4751ccb8"

FAMILY = "yuanme NCM Math"
POSTSCRIPT = "yuanmeNCMMath-Book"

DESCRIPTION = f"""\
{FAMILY} is New Computer Modern Math Book 8.1.1 by Antonis Tsolomitis, renamed \
for yuanme. math-core-fonts ({REPOSITORY}, commit {REV[:12]}) changed it for MathML \
in browsers: the primes are their script-size forms, the accents are centred or \
lowered in their glyph boxes, breve and caron are drawn from their combining forms, \
and U+FE00 variation sequences are declared on the calligraphic capitals. No other \
glyph changed, and no outline was redrawn. The authors of New Computer Modern don't \
support this version: report problems to https://github.com/yuann3/yuanme/issues. \
The original is at https://ctan.org/pkg/newcomputermodern."""

LICENSE = (
    "This Font Software is licensed under the GUST Font License, which is LPPL 1.3c "
    "or later with a request to rename derived fonts."
)
LICENSE_URL = "https://www.gust.org.pl/fonts/licenses/GUST-FONT-LICENSE.txt"

HERE = Path(__file__).resolve().parent
OUT = HERE / "src" / "font"


def main():
    # FontForge stamps `head` with this rather than the time of the build.
    os.environ["SOURCE_DATE_EPOCH"] = "0"
    import fontforge

    with tempfile.TemporaryDirectory() as tmp:
        git = lambda *args: subprocess.run(["git", "-C", tmp, *args], check=True)
        git("init", "-q")
        git("fetch", "-q", "--depth", "1", REPOSITORY, REV)
        git("checkout", "-q", "FETCH_HEAD")
        source = Path(tmp) / "NewComputerModernMath"
        sys.path.insert(0, str(source))
        from patches import apply_patches

        sfd = source / "NewCMMath-Book.sfd"
        font = fontforge.open(str(sfd))
        apply_patches(font)
        rename(font)
        OUT.mkdir(parents=True, exist_ok=True)
        otf = OUT / "yuanme-ncm-math.otf"
        # FFTM holds FontForge's own build date.
        font.generate(str(otf), flags=("opentype", "no-FFTM-table"))
        gust = (source / "GUST-FONT-LICENSE.txt").read_text()
        notice(sfd, otf, gust, fontforge.version())


def rename(font):
    font.fontname = POSTSCRIPT
    font.familyname = FAMILY
    font.fullname = f"{FAMILY} Book"
    # Both identify New Computer Modern itself.
    font.xuid = ""
    font.uniqueid = 0
    english = "English (US)"
    names = {
        "Preferred Family": FAMILY,
        "UniqueID": f"{FAMILY} Book; math-core-fonts {REV[:12]}",
        "Descriptor": DESCRIPTION,
        "License": LICENSE,
        "License URL": LICENSE_URL,
    }
    font.sfnt_names = tuple(
        n for n in font.sfnt_names if not (n[0] == english and n[1] in names)
    ) + tuple((english, key, value) for key, value in names.items())


def notice(sfd, otf, gust, fontforge_version):
    sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    (OUT / "NOTICE").write_text(
        f"""\
yuanme-ncm-math.otf is {FAMILY}, the math font the Engine embeds and subsets
for each Site. crates/yuanme-assets/math-font.py builds it; don't edit it by hand.

{textwrap.fill(DESCRIPTION, 80)}

Built from:  {REPOSITORY} at {REV},
             NewComputerModernMath/NewCMMath-Book.sfd (sha256 {sha(sfd)})
Built with:  FontForge {fontforge_version}, SOURCE_DATE_EPOCH=0
Output:      sha256 {sha(otf)}

Each subset the Engine writes keeps the font's copyright, description and licence
names.

The font is under the GUST Font License, which follows:

{gust}"""
    )


if __name__ == "__main__":
    main()
