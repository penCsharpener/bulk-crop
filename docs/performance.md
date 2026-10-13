# Export speed

Measured for row 0001.17. The photos are synthetic, not real photos, because no real photo folder
was available. Real phone photos may decode more slowly, so treat these numbers as a first estimate.

## Setup

| | |
|---|---|
| Machine | 4 vCPU Intel Xeon 2.80 GHz, 16 GB RAM, Linux |
| Build | `cargo build --release -p pagecrop-cli` |
| Photos | 100 synthetic JPEG pages, 4000 × 3000 pixels, about 80 MB in total (about 0.8 MB each) |
| Photos made by | `pagecrop-cli gen-fixtures <dir> --count 100 --width 4000 --height 3000` |
| Command | `pagecrop-cli export <dir> --out <out> --long-edge 1600 --overwrite` |

Peak memory is the largest resident set size of the export process, read with Python's
`resource.getrusage(RUSAGE_CHILDREN)`, because `/usr/bin/time` is not installed here.

## Results

| Run | Total time | Photos per second | Peak memory |
|---|---|---|---|
| 1 | 36.9 s | 2.71 | 550 MiB |
| 2 | 35.9 s | 2.78 | 576 MiB |

Each run wrote 100 pictures, skipped none, and failed none.

## Verdict

The result is **below 5 photos per second** in both runs. The roadmap's Risks section names this as
the signal to revisit "Export memory grows with photo size and thread count". The measured peak of
about 140 MB per worker (four workers on this machine) matches that section's estimate of 150 MB per
worker for a 12-megapixel photo, so memory is not the surprise here. The speed is. The export runs
up to eight threads, and this machine has four cores, so the next step to look at is the decode and
resize of each photo.
