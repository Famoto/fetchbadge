# Case and parts

Print `tray.stl` floor-down and `bezel.stl` face-down, PETG/ASA, 0.2 mm, 3 walls, ~40 g total. No supports.
Outer size ~127 x 72 x 17 mm. Stack inside, front to back: clear lens, board, LiPo (foam tape).

Board numbers are from LilyGO's official `shell/EPD47-S3.dxf`, but **measure your board** and adjust
`fit`, `back_parts`, `bat_t`, and the port cutouts in the `.scad`. If USB-C/button cutouts land on the
wrong side, set `mirror_ports = true`. Re-export from `case/`:

```sh
openscad -D 'part="tray"' -o tray.stl nametag_case.scad
openscad -D 'part="bezel"' -o bezel.stl nametag_case.scad
```

The back has a 100 x 50 x 0.8 mm recess for adhesive hook velcro, with 4 holes to stitch or zip-tie it down
(adhesive alone lets go in a warm con hall).

## Parts list

| # | Part | Notes | ~Price |
|---|------|-------|--------|
| 1 | LilyGO T5 4.7" E-Paper S3 V2.3 |  | 40€ |
| 1 | LiPo 3.7 V, 1000 mAh, **503450** (5 x 34 x 50 mm) | JST-PH 2.0 2-pin. **Check polarity against BAT+/BAT− on the board before plugging in**, cheap packs are often reversed | €6–10 |
| 1 | Clear acrylic or PETG sheet, 1 mm, cut to 118 x 63 mm | protects the 0.87 mm glass panel; set `lens_t=0` to skip | €3 |
| 4 | M2 x 8 self-tapping screws for plastic (pan head) | bezel to tray | €2 |
| 1 | Adhesive **hook** velcro, 50 mm wide, cut to 100 mm | backpack patch panels are usually loop; get loop if yours is hook | €4 |
| – | Double-sided foam tape | holds battery and board still | €2 |
| – | Thread + needle or 2 small zip ties | through the stitch holes | — |
| – | ~40 g PETG or ASA filament | PLA softens in a hot car | — |
| 1 | USB-C data cable | flashing and charging (charges in the case) | — |
