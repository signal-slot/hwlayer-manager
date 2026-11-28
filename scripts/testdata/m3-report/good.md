# M3 AM62P report (fixture)

Date: 2026-12-01

## Board, BSP, kernel and panel

- Board: Verdin AM62P
- BSP: Torizon fixture 1.0
- Kernel: 6.6.0-fixture
- Panel: fixture panel 1280x800

## Console (trace buffer)

```
SCENE 1 two-plane base+overlay commits=2 latched=2
SCENE 2 move commits=3 latched=3
SCENE 3 release commits=1 latched=1
SCENE 4 refused-format refused=1
SCENE 5 refused-overlap refused=1
SCENE 6 blend commits=2 latched=2
SCENE 7 z-order commits=2 latched=2
SCENE 8 final commits=1 latched=1
RESULT PASS
```

## Photographs and camera-check

![scene 1](scene1.png)

- Scene 1: camera-check PASS
- Scene 2: camera-check PASS
- Scene 3: camera-check PASS
- Scene 4: camera-check PASS
- Scene 5: camera-check PASS
- Scene 6: camera-check PASS
- Scene 7: camera-check PASS
- Scene 8: camera-check PASS

## Restore and Torizon confirmation

Restored firmware name and state; rebooted into the Torizon chain.
Torizon up over ssh, confirmed 2026-12-01.
