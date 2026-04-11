# INZONE Buds Battery Tray

Windows tray application for showing Sony INZONE Buds battery levels.

This project targets INZONE Buds USB dongle `VID=054C PID=0EC2`. It queries the same HID collection used by INZONE Hub and shows left earbud, right earbud, and case battery state from the system tray.

## Status

Version: `1.0.0`

Confirmed behavior:

- Left earbud only: `Left: <percent>`, `Right: ?`, `Case: <percent>`
- Right earbud only: `Left: ?`, `Right: <percent>`, `Case: <percent>`
- Both earbuds out: `Left: <percent>`, `Right: <percent>`, `Case: <percent>`
- Both earbuds in case: the dongle may stop responding to the active battery query; the app shows unknown values instead of guessing from stale or unrelated reports.

## Requirements

- Windows
- Rust stable toolchain
- Sony INZONE Buds connected via the USB dongle

## Run

```powershell
cargo run
```

The app starts in the system tray. Open the tray menu to see the current battery values.

## Useful Commands

List matching HID devices:

```powershell
cargo run -- list-hid --vendor 054c --product 0ec2
```

Query the INZONE Hub-compatible battery report directly:

```powershell
cargo run -- query-hub-battery --count 3 --timeout 3000 --interval-ms 1000
```

Capture all HID collection state for protocol investigation:

```powershell
cargo run -- capture-state --all-collections baseline
```

Compare two captured state directories:

```powershell
cargo run -- compare-state-dirs dumps\states\<left> dumps\states\<right>
```

Capture feature reports over time:

```powershell
cargo run -- capture-feature-series --all-collections --samples 80 --interval-ms 250 transition
```

Analyze a captured feature series:

```powershell
cargo run -- analyze-feature-series dumps\series\<series_dir>
```

Generated logs, dumps, and Wireshark captures are intentionally ignored by Git.

## Protocol Summary

The implemented battery query uses:

- HID collection: `interface=5`, `usage_page=0xFF04`, `usage=0x0001`
- Output/Input report ID: `0x02`
- Request command: `0x41`
- Battery subcommand: `0x04`
- Response command: `0x14`

Battery response offsets:

- `0x0E`: left earbud battery
- `0x10`: right earbud battery
- `0x12`: case battery
- `0x00..0x64`: battery percent
- `0xFF`: unavailable

See `docs/protocol.md` for details.

## License

MIT
