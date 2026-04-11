# INZONE Buds Battery Tray

Windows system tray battery monitor for Sony INZONE Buds.

## 日本語

### 概要

INZONE Buds の左イヤホン、右イヤホン、ケースのバッテリー残量を Windows のシステムトレイに表示する Rust 製アプリです。

対象は USB ドングル接続の INZONE Buds `VID=054C PID=0EC2` です。INZONE Hub と同じ HID collection に active query を送り、バッテリー情報を取得します。

### 対応環境

- Windows
- Sony INZONE Buds
- INZONE Buds の USB ドングル

### 使い方

GitHub Releases から zip をダウンロードし、展開後に以下を実行します。

```powershell
.\inzone-buds-battery-tray.exe
```

起動するとシステムトレイに常駐します。トレイアイコンの右クリックメニューで現在の残量を確認できます。

表示例:

```text
INZONE Buds: L 100% / R ? / C 100%
Left: 100%
Right: ?
Case: 100%
Status: Connected
Exit
```

### 表示仕様

- `L`: 左イヤホン
- `R`: 右イヤホン
- `C`: ケース
- `?`: 未接続、ケース内、または取得不可

確認済みの挙動:

- 左だけ出す: `Left: <percent>`, `Right: ?`, `Case: <percent>`
- 右だけ出す: `Left: ?`, `Right: <percent>`, `Case: <percent>`
- 両方出す: `Left: <percent>`, `Right: <percent>`, `Case: <percent>`
- 両方ケース内: ドングルが active query に応答しない場合があるため、推測値ではなく `?` を表示します。

### 設定

設定ファイルは `config/settings.json` です。存在しない場合は初回起動時に自動生成されます。

主な設定:

- `low_battery_threshold`: 低バッテリー通知のしきい値
- `poll_interval_ms`: バッテリー取得間隔
- `vendor_id`: Sony vendor ID
- `product_id`: INZONE Buds product ID

### 開発者向けコマンド

HID デバイス一覧:

```powershell
cargo run -- list-hid --vendor 054c --product 0ec2
```

INZONE Hub 互換の battery query:

```powershell
cargo run -- query-hub-battery --count 3 --timeout 3000 --interval-ms 1000
```

HID collection の状態取得:

```powershell
cargo run -- capture-state --all-collections baseline
```

状態差分:

```powershell
cargo run -- compare-state-dirs dumps\states\<left> dumps\states\<right>
```

feature report の時系列取得:

```powershell
cargo run -- capture-feature-series --all-collections --samples 80 --interval-ms 250 transition
```

feature report の時系列解析:

```powershell
cargo run -- analyze-feature-series dumps\series\<series_dir>
```

### 既知の制限

- Sony 公式アプリではありません。
- INZONE Buds 以外の INZONE 製品では未検証です。
- 両方のイヤホンをケースに入れた状態では、ドングルが battery query に応答しない場合があります。
- Release build の exe はコンソールを表示しません。CLI 調査コマンドは `cargo run -- ...` で実行してください。

## English

### Overview

INZONE Buds Battery Tray is a Windows system tray application that shows the battery levels of Sony INZONE Buds.

The app targets the INZONE Buds USB dongle `VID=054C PID=0EC2`. It queries the HID collection used by INZONE Hub and displays the left earbud, right earbud, and case battery state.

### Requirements

- Windows
- Sony INZONE Buds
- INZONE Buds USB dongle

### Usage

Download the zip from GitHub Releases, extract it, and run:

```powershell
.\inzone-buds-battery-tray.exe
```

The app starts in the system tray. Right-click the tray icon to see current battery values.

### Display Behavior

- `L`: left earbud
- `R`: right earbud
- `C`: case
- `?`: unavailable, disconnected, or in case

Confirmed behavior:

- Left earbud only: `Left: <percent>`, `Right: ?`, `Case: <percent>`
- Right earbud only: `Left: ?`, `Right: <percent>`, `Case: <percent>`
- Both earbuds out: `Left: <percent>`, `Right: <percent>`, `Case: <percent>`
- Both earbuds in case: the dongle may stop responding to the active battery query; the app shows unknown values instead of guessing from stale or unrelated reports.

### Developer Commands

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

Generated logs, dumps, and Wireshark captures are intentionally ignored by Git.

### Protocol

See `docs/protocol.md`.

### License

MIT
