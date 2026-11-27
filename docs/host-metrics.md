# Host metrics

SoW §10 asks for performance metrics, the first being **plane assignment
time**. This file records what the portable core costs on a development
host. Nothing here is a hardware result: no board, DRM device or display
driver is involved.

## What is measured

`crates/composition-fake/examples/plan_time.rs` builds capability sets on the
fake backend and times one `Engine::validate` call per sample: the planner
search plus the fake backend's `check` (which accepts everything). Nothing is
committed. Timing uses `std::time::Instant` only, one call per sample, after a
warm-up of a tenth of the iterations; the `Instant` overhead (a few tens of
nanoseconds) is included in every figure.

All capability sets have one 1920x1080 output. The first plane is a primary
plane with a fixed zpos, XRGB8888 and ARGB8888, no scaling, no plane alpha and
no NV12. The other planes are overlays with ranged zpos, scaling (up to 8x up
and 4x down), plane alpha and NV12. The sets are:

- 2 planes
- 4 planes
- 8 planes with DPU-like resource groups: 2 scalers shared by planes 2 to 5
  (used only by scaled layers), 1 YUV converter shared by planes 4 to 7 (used
  only by NV12 layers), at most 6 enabled planes
- the maximum of `limits.rs`: 16 planes and 8 resource groups

Frames have 1, 2, 4 and `MAX_LAYERS` (16) layers, in five mixes. Every layer
has a 1280x720 destination, offset by 16 pixels per layer, and z-order equal
to the frame index:

- `plain`: opaque XRGB8888, 1280x720 source crop, so unscaled
- `scaled`: XRGB8888, 640x360 source crop, a 2x upscale
- `yuv`: NV12, 1280x720 source crop, unscaled
- `alpha`: premultiplied ARGB8888 with plane alpha 0x8000, unscaled
- `mixed`: the four kinds in turn, with z-order opposite to frame order

The `result` column is what `validate` returned, because the planner takes a
different path when no assignment exists. `ok` is a found plan. `NoPlane`
means the search proved that no legal assignment exists. In these sets that
has two causes: the layers outnumber the planes that can take them (the
primary plane cannot scale, blend with plane alpha or read NV12, so `scaled`,
`yuv` and `alpha` layers can only use the overlays, while `plain` layers can
use every plane), and the DPU-like groups cap a frame at 6 planes.
`LimitExceeded` means the default search limit (20 000 steps) stopped the
search. Failing cases are timed too, but only `ok` rows are plane assignment
times in the strict sense.

## How to run

```sh
cargo run --release -p composition-fake --example plan_time
# optional: iterations per case (default 20000)
cargo run --release -p composition-fake --example plan_time -- 100000
```

## One run

Host measurement 2026-11-28, 11th Gen Intel(R) Core(TM) i7-1165G7 @ 2.80GHz,
release build, 20 000 iterations per case, not a hardware result. Times in
nanoseconds per `Engine::validate` call.

```text
capabilities                layers mix     result          min   median      p99       max
2 planes                         1 plain   ok              132      137      224     15101
2 planes                         1 scaled  ok              137      141      163      1236
2 planes                         1 yuv     ok              135      140      172      4845
2 planes                         1 alpha   ok              133      137      161      8077
2 planes                         1 mixed   ok              133      136      221      4793
2 planes                         2 plain   ok              207      214      339      6869
2 planes                         2 scaled  NoPlane         203      208      319      4826
2 planes                         2 yuv     NoPlane         175      205      295      7993
2 planes                         2 alpha   NoPlane         202      207      325     17534
2 planes                         2 mixed   NoPlane         201      206      237      4766
2 planes                         4 plain   NoPlane         324      329      459     10633
2 planes                         4 scaled  NoPlane         332      337      452      9318
2 planes                         4 yuv     NoPlane         332      337      477     12217
2 planes                         4 alpha   NoPlane         325      329      348      4279
2 planes                         4 mixed   NoPlane         292      339      479     12568
2 planes                        16 plain   NoPlane        1046     1210     1657     13681
2 planes                        16 scaled  NoPlane        1065     1230     1594     14194
2 planes                        16 yuv     NoPlane        1070     1237     1570     12807
2 planes                        16 alpha   NoPlane        1020     1188     1522      9387
2 planes                        16 mixed   NoPlane        1131     1306     1714     13737
4 planes                         1 plain   ok              144      148      227      4904
4 planes                         1 scaled  ok              148      154      223     12877
4 planes                         1 yuv     ok              148      152      268      8402
4 planes                         1 alpha   ok              143      147      169      3918
4 planes                         1 mixed   ok              125      147      235      5702
4 planes                         2 plain   ok              228      235      389      9066
4 planes                         2 scaled  ok              238      245      418     15969
4 planes                         2 yuv     ok              235      240      272      4897
4 planes                         2 alpha   ok              230      237      458     14142
4 planes                         2 mixed   ok              206      241      266      6302
4 planes                         4 plain   ok              407      415      659      7699
4 planes                         4 scaled  NoPlane         492      579      894     14521
4 planes                         4 yuv     NoPlane         554      577      837     12809
4 planes                         4 alpha   NoPlane         474      559      871     10028
4 planes                         4 mixed   NoPlane         575      598      889      6646
4 planes                        16 plain   NoPlane        1198     1389     1844     19085
4 planes                        16 scaled  NoPlane        1435     1450     1928     15452
4 planes                        16 yuv     NoPlane        1259     1464     1943      7403
4 planes                        16 alpha   NoPlane        1162     1369     1793     12428
4 planes                        16 mixed   NoPlane        1319     1526     2108     15840
8 planes, DPU-like groups        1 plain   ok              166      175      194      7668
8 planes, DPU-like groups        1 scaled  ok              154      183      202      3559
8 planes, DPU-like groups        1 yuv     ok              175      179      203      5032
8 planes, DPU-like groups        1 alpha   ok              166      172      324      6788
8 planes, DPU-like groups        1 mixed   ok              165      172      280     14078
8 planes, DPU-like groups        2 plain   ok              240      280      474      6427
8 planes, DPU-like groups        2 scaled  ok              299      312      467      1280
8 planes, DPU-like groups        2 yuv     ok              296      307      534     15536
8 planes, DPU-like groups        2 alpha   ok              238      279      301      3392
8 planes, DPU-like groups        2 mixed   ok              294      308      526      8712
8 planes, DPU-like groups        4 plain   ok              501      508      720      9137
8 planes, DPU-like groups        4 scaled  ok              551      563      960      7353
8 planes, DPU-like groups        4 yuv     ok              542      549      916     11062
8 planes, DPU-like groups        4 alpha   ok              497      516      777      6176
8 planes, DPU-like groups        4 mixed   ok              545      554      945      5209
8 planes, DPU-like groups       16 plain   NoPlane        1700     1751     2689     23038
8 planes, DPU-like groups       16 scaled  NoPlane        1853     1874     2948     21994
8 planes, DPU-like groups       16 yuv     NoPlane        1862     1888     3018     19541
8 planes, DPU-like groups       16 alpha   NoPlane        1641     1724     2506     18539
8 planes, DPU-like groups       16 mixed   NoPlane        1902     1939     2858     24183
max (16 planes, 8 groups)        1 plain   ok              217      310      501     27731
max (16 planes, 8 groups)        1 scaled  ok              235      247      429     13506
max (16 planes, 8 groups)        1 yuv     ok              233      242      468     20145
max (16 planes, 8 groups)        1 alpha   ok              210      279      634     19341
max (16 planes, 8 groups)        1 mixed   ok              216      360      694     18339
max (16 planes, 8 groups)        2 plain   ok              370      653      833     33238
max (16 planes, 8 groups)        2 scaled  ok              410      688      957     21289
max (16 planes, 8 groups)        2 yuv     ok              406      412      723     14086
max (16 planes, 8 groups)        2 alpha   ok              364      384      706     17146
max (16 planes, 8 groups)        2 mixed   ok              398      407      740     15203
max (16 planes, 8 groups)        4 plain   ok              675      685     1221     14859
max (16 planes, 8 groups)        4 scaled  ok              758      771     1361     10808
max (16 planes, 8 groups)        4 yuv     ok              754      765     1316     12147
max (16 planes, 8 groups)        4 alpha   ok              656      685     1190     13083
max (16 planes, 8 groups)        4 mixed   ok              751      768     1288     15128
max (16 planes, 8 groups)       16 plain   ok             2490     2854     4705     17527
max (16 planes, 8 groups)       16 scaled  LimitExceeded    45974    57704    94409    327124
max (16 planes, 8 groups)       16 yuv     LimitExceeded    47930    59262    94475    133899
max (16 planes, 8 groups)       16 alpha   LimitExceeded    48038    58232    94551    149258
max (16 planes, 8 groups)       16 mixed   LimitExceeded    47448    57981   100432    169000
```

## Reading the table

- Found plans cost about 140 ns (1 layer, 2 planes) to about 770 ns (4 layers,
  16 planes) at the median on this host, and 2.9 µs for 16 `plain` layers on
  16 planes; cost grows with layers and planes.
- `NoPlane` rows are quick because the search fails early. With 2 planes only
  `plain` fits two layers: `scaled`, `yuv` and `alpha` have one usable
  overlay. With 4 planes, 4 layers fit only as `plain` (the primary plane
  takes the bottom layer). With the DPU-like groups, 16 layers exceed the
  6-plane cap.
- The p99 and max columns are dominated by scheduler and cache noise of an
  ordinary desktop, not by the planner: single-sample maxima of tens of
  microseconds appear in every group. Compare medians.
- 16 non-`plain` layers on the maximum capability set exhaust the default
  search limit and return `LimitExceeded` after about 60 µs at the median.
  This is the worst case the limits allow, and it is bounded.
- The fake backend's check is trivial. A real backend's `TEST_ONLY` commit
  would add to this and is not measured here.
