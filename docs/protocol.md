# INZONE Buds HID Battery Protocol

This document summarizes the protocol details verified for INZONE Buds `VID=054C PID=0EC2`.

## HID Collections

The dongle exposes multiple HID collections on interface `5`. The battery query used by INZONE Hub is not the earlier `FF03/0020` feature report path. It uses:

- `usage_page=0xFF04`
- `usage=0x0001`
- report ID `0x02`
- interrupt IN endpoint `0x83`

## Active Query

The app sends a 64-byte output report. The report is sent via `WriteFile` on Windows because `HidD_SetOutputReport` may fail for this device.

Request layout:

```text
02 0C 01 00 FC 08 96 C3 41 04 01 SS 00 CC ...
```

Fields:

- byte `0x00`: report ID, always `0x02`
- byte `0x08`: request command, `0x41`
- byte `0x09`: subcommand, `0x04` for battery
- byte `0x0A`: request kind, `0x01`
- byte `0x0B`: sequence number
- byte `0x0D`: checksum-like byte, `0x9B + subcommand + sequence`

Example request for sequence `1`:

```text
02 0C 01 00 FC 08 96 C3 41 04 01 01 00 A0 ...
```

## Battery Response

The explicit response has:

```text
02 12 04 FF 0F 00 96 C3 14 04 10 SS 00 00 LL 00 RR FF CC ...
```

Fields:

- byte `0x00`: report ID, `0x02`
- byte `0x08`: response command, `0x14`
- byte `0x09`: subcommand, `0x04`
- byte `0x0A`: response kind, `0x10`
- byte `0x0B`: matching sequence number
- byte `0x0E`: left earbud battery
- byte `0x10`: right earbud battery
- byte `0x12`: case battery

Values:

- `0x00..0x64`: battery percent
- `0xFF`: unavailable

Verified states:

```text
Left only:
  left=100%, right=unavailable, case=100%

Right only:
  left=unavailable, right=100%, case=100%

Both earbuds out:
  left=100%, right=100%, case=100%
```

When both earbuds are in the case, the dongle may not respond to the active query. The app treats this as connected but unknown, and does not fall back to the older `A0/A1` heuristic to avoid false values.

## Notes

Earlier investigation of feature reports `A0/A1` on `usage_page=0xFF03 usage=0x0020` was useful for discovery but is not used for v1.0 battery polling.
