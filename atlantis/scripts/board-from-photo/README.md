# board-from-photo

How `data/board.ron` was made from a top-down photo of the printed board.
One-off scripts, kept for the record. They expect `real_board.png` (the
photo as PNG, 1589 px square) in the working directory and run with
`uv run --with pillow --with numpy python <script>`.

1. `fit3.py`: measure the hex pitch and the grid phase. The pitch came from
   the autocorrelation of the sand texture (81.8 px, rotated -15 degrees);
   the phase from the bright hex vertices.
2. `classify.py`, `classify2.py`, `classify3.py`: one hex at a time, read
   the median colour and call it sea, sand (pale or brown), jungle or rock.
   Sand shade and position give the zone; rock hexes take the zone of their
   neighbours.
3. `crops.py`: labelled crops of each zone, to read the spawn icons by eye.
4. `gen_board.py`: the hand-read icons and throne outlines, plus the
   classified hexes, written out as the board file.

The icon key is rulebook page 6: red oval = melee, horned = ranged, disc =
heavy; blue clawed hand = melee, bony hand = ranged, burst = heavy.
