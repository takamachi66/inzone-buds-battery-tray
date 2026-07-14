# 開発・リリースガイド

GitHub Actions による CI / リリース自動化の運用手順をまとめます。

## ワークフロー一覧

| ワークフロー | ファイル | 発火条件 | 内容 |
|---|---|---|---|
| **CI** | `.github/workflows/ci.yml` | `main`/`develop` への push、および全 PR | fmt / clippy / build / test |
| **Release** | `.github/workflows/release.yml` | `v*` タグの push | バージョン検証 → release ビルド → zip → GitHub Release 公開 |

いずれも `windows-latest` ランナーで実行します。依存クレート（`hidapi` の `windows-native`、`winrt-notification`、`windows-sys`、`winreg`）が Windows 専用のため、他プラットフォームではビルドできません。

**ブランチの push** と **タグの push** は別物です。普段の push は CI のみ、リリースはタグを push した時だけ動きます。

## 日常の開発サイクル

```bash
git add -p
git commit -m "..."
git push            # main/develop への push で CI が発火
```

push すると Actions タブで CI が回り、次を順に検証します。

1. `cargo fmt --check`
2. `cargo clippy --all-targets --locked -- -D warnings`（警告 1 件でも失敗）
3. `cargo build --locked`
4. `cargo test --locked`

### push 前のローカル事前チェック

CI と同じ内容を手元で先に通しておくと、CI の失敗を防げます。

```bash
cargo fmt
cargo clippy --all-targets --locked -- -D warnings
cargo build --locked
cargo test --locked
```

### 注意点

- **fmt 忘れ**: `cargo fmt` を実行してから再コミット。
- **clippy の新規警告**: `-D warnings` により CI が失敗する。ローカルで解消してから push。
- **`--locked` 失敗**: 依存の追加・変更後に `Cargo.lock` を更新せずコミットすると失敗する。依存を触ったら `cargo build` で `Cargo.lock` を更新し、一緒にコミットする。
- **feature ブランチ**: `main`/`develop` 以外への直接 push では CI は走らない。PR を作成すると PR トリガーで CI が回る。
- 連続 push では同一ブランチの古い CI 実行が自動キャンセルされる（`cancel-in-progress`）。

## リリース手順

「`Cargo.toml` のバージョンを上げる → コミット → タグを打つ → タグを push」の順に行います。

```bash
# 1. Cargo.toml の version を次の未使用バージョンに更新（例: 1.0.3）

# 2. Cargo.lock を追従させる（自身のバージョンが記録されているため）
cargo build
git add Cargo.toml Cargo.lock
git commit -m "Release 1.0.3"

# 3. ブランチを push
git push

# 4. タグを打って push（ここで Release ワークフローが発火）
git tag v1.0.3
git push origin v1.0.3
```

タグ push 後、Release ワークフローが自動で以下を実行します。

1. タグと `Cargo.toml` のバージョン一致を検証（`v1.0.3` ⇔ `1.0.3`）。不一致なら失敗。
2. `cargo build --release --locked`
3. zip 化（`exe` ＋ `config/` ＋ `docs/` ＋ `README.md` ＋ `LICENSE`）
4. GitHub Releases に公開し、zip を添付（`GITHUB_TOKEN` を自動使用。追加のシークレット設定は不要）

### 注意点

- **バージョン不一致で失敗するのは仕様**（安全弁）。`Cargo.toml` を上げ忘れてタグだけ打つと止まる。上記 1→4 の順を守る。
- **`Cargo.lock` の更新忘れ**に注意。バージョンを上げたら必ず `cargo build` で `Cargo.lock` を追従させてコミットする。
- **タグは再利用できない**。既存タグと同名にはできないため、常に次の未使用バージョンを使う。
- **タグが指すコミットにワークフローが含まれている必要がある**。ワークフロー追加後のコミットにタグを打つこと。
- **やり直し**（リリースが途中失敗した場合）:
  ```bash
  git tag -d v1.0.3                 # ローカル削除
  git push origin :refs/tags/v1.0.3 # リモート削除
  # 修正後、再度タグを打ち直して push
  ```

## 全体像

```
[日常]  編集 → (ローカルで fmt/clippy/test) → commit → push(main/develop)
                                                   └─▶ CI 実行

[公開]  Cargo.toml version↑ → cargo build → commit → push
        → git tag vX.Y.Z → git push origin vX.Y.Z
                                    └─▶ Release 実行（検証 → build → zip → Releases 公開）
```

## 補足

- リリースされる exe は未署名のため、実行時に Windows SmartScreen の「発行元不明」警告が出る場合があります。解消にはコード署名証明書が必要です。
- プロトコル解析の詳細は [protocol.md](protocol.md) を参照してください。
