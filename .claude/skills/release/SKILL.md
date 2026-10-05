---
name: release
description: Luster の新しい版を、Swift Package（GitHub Releases）・Maven Central・pub.dev に出し、公証した macOS 用アプリをリリースに添付する。「リリースして」「0.x.y を出したい」「新しいバージョンを公開」などのときに使う。
---

# Luster のリリース

手順の正本は `CONTRIBUTING.md` の Releasing。このスキルは、Claude が進めるときに各段階で何を確かめれば「通った」と言えるかと、過去につまずいたところをまとめたもの。

公開先と、取り消せるかどうか:

| 公開先 | いつ公開されるか | 取り消し |
| --- | --- | --- |
| GitHub Releases（SwiftPM の XCFramework の zip） | `release.yml` を `publish=true` で実行したとき | draft のうちはできる。公開後は実質できない（タグとチェックサムが固定される） |
| Maven Central（`dev.famio:luster`・`luster-compose`） | 同上 | できない |
| Flutter 用のビルド済みエンジン（`precompiled_<hash>` の prerelease） | 同上 | ― |
| pub.dev（`luster`） | ユーザーが `dart pub publish` を実行したとき | できない（7 日以内なら retract のみ） |
| macOS 用アプリ（公証済みの zip を GitHub のリリースに添付） | `gh release upload` したとき | 添付は消せる |

## 守ること

- **取り消せない操作の前で必ず止めて、ユーザーの確認を取る**: `publish=true` の実行と `dart pub publish`。
- `Scripts/release.sh`・push・`gh release create`・`gh workflow run` は、自動モードの安全判定で止められることがある。止められたら回避しようとせず、ユーザーに `!` を付けて実行してもらう（入力欄の先頭に `!` を付けるとシェルのモードになる、と添える）。
- 各段階の「確認」をすべて満たしてから次へ進む。長い処理（`release.sh` は 5〜6 分、`publish=true` は 30〜40 分）はバックグラウンドで実行し、終わったら結果を読む。
- ワークフローは `gh workflow run` を打った時点の **main の定義** で動き、中では `inputs.version` のタグをチェックアウトする。ワークフローを直したら、実行の前に main へ push する。

## 0. 事前確認

- `main` にいて `origin/main` と同じ。直前の main の CI が成功している（`gh run list --limit 3`）。
- 作業ツリーがきれい。`release.sh` は未追跡ファイルがあっても止まる。手元だけのメモ（`PUBLISHING_TODO.md` など）は `.git/info/exclude` に足す。
- `Package.swift` がローカルの XCFramework（`path: "rust/target/apple/LusterFFI.xcframework"`）を指している。
- 版番号 X を決める（semver。0.x のうちは破壊的変更でマイナーを上げる）。
- `flutter/luster/CHANGELOG.md` に `## X` の項目を書いてコミットしておく。pub.dev の採点は CHANGELOG にその版があるかを見る。
- README の Install 欄の版番号は、**公開してから** 上げる（先に書くと、まだ取れない版を案内することになる）。
- Rust を変えた版かどうかを把握しておく。変えていなければ Flutter 用のビルド済みエンジンのハッシュは前の版と同じで、`flutter-binaries` はビルドを飛ばす（正常）。

## 1. dry run

```sh
Scripts/release.sh X --dry-run
```

確認:
- 終了コード 0
- `cargo test` がすべて `ok`、`swift test` の `Test run with … passed`
- parity が `svg: matches fixtures/golden`（fixtures のすべてで `artwork and badges match`）
- 最後の差分が `Package.swift`（URL とチェックサム）と版番号のファイルだけで、作業ツリーが元に戻っている

## 2. 本番（手元にコミットとタグを作るだけ）

```sh
Scripts/release.sh X
```

確認:
- `chore(release): X` と `chore: build against the local engine again after X` の 2 つのコミット
- 注釈付きタグ X が `chore(release): X` を指している（`git rev-parse --short X^{commit}`）
- `swift package compute-checksum rust/target/apple/LusterFFI.xcframework.zip` が、`git show X:Package.swift` の `checksum:` と一致
- HEAD の `Package.swift` はローカルのパスに戻っている。作業ツリーがきれい

## 3. push と draft

```sh
git push origin HEAD X
gh release create X rust/target/apple/LusterFFI.xcframework.zip --draft --verify-tag --title "Luster X" --generate-notes
```

確認（`gh release view X --json isDraft,assets`）:
- `isDraft` が true、`LusterFFI.xcframework.zip` が `uploaded` で、サイズが手元の zip と同じ
- 表示される URL が `untagged-…` なのは draft の仕様

## 4. 確認の実行（publish=false、何も公開しない）

```sh
gh workflow run release.yml -f version=X -f publish=false
gh run watch <run-id> --exit-status
```

確認:
- `apple`・`android`・`flutter` が success、`flutter-binaries` と `flutter-binaries-check` は skipped
- `apple` のログで `attached` と `tagged` のチェックサムが同じ、parity が通っている
- `android` のログで `:luster:signMavenPublication` と `:luster-compose:signMavenPublication` が実行されている（本物の GPG 鍵で署名できた証拠）
- `flutter` のログで、ビルドなしの `flutter analyze` が `No issues found!`、pub の dry run の警告が `flutter_rust_bridge` の固定 1 件だけ

**ここで止めて、`publish=true` に進んでよいかをユーザーに聞く。**

## 5. 公開（publish=true、取り消せない）

```sh
gh workflow run release.yml -f version=X -f publish=true
gh run watch <run-id> --exit-status
```

確認:
- 6 つのジョブがすべて success
- `gh api repos/famio/Luster/releases/latest --jq .tag_name` が X（`precompiled_…` は prerelease なので Latest にならない）
- zip がログインなしで取れる: `curl -sL -o /dev/null -w "%{http_code}" https://github.com/famio/Luster/releases/download/X/LusterFFI.xcframework.zip` が 200
- Maven Central: `android` のログに `Uploaded bundle to Central Portal … deployment id` と `Deployment is being published`。`https://repo1.maven.org/maven2/dev/famio/luster/X/luster-X.pom` と `luster-compose` が 200（反映に数十分かかることがある）
- ビルド済みエンジン: 手元で次を実行し、Apple 5 種と Android 4 種がすべて `OK`、`Crate hash` が `precompiled_<hash>` のタグと同じ
  ```sh
  cd flutter/luster/cargokit/build_tool
  fvm dart run build_tool verify-binaries --manifest-dir=../../rust/crates/luster-dart
  ```

## 6. pub.dev（ユーザーが実行する）

1. 手元で `cd flutter/luster && fvm flutter pub publish --dry-run`。警告は `flutter_rust_bridge` の固定 1 件だけであること（警告があると終了コードは 65）。
2. タグのあとに Rust を変えていないことを確かめる（変えるとハッシュが合わず、利用者がソースからビルドすることになる）。
3. ユーザーに実行してもらう:
   ```sh
   cd flutter/luster && fvm dart pub publish
   ```
   警告の確認には `y`。初回はブラウザで Google 認証。
4. 確認: `curl -s https://pub.dev/api/packages/luster` の `latest.version` が X。

pub.dev は、手元の dry run では出ない指摘で **サーバー側で拒否** することがある（「Message from server: …」）。拒否されたら何も公開されていないので、直してコミット・push してから、もう一度実行してもらう。過去の例:
- `LICENSE` file contains generic TODO（パッケージの LICENSE がテンプレートのままだった。今はトップの LICENSE へのリンク）
- Invalid `topics` value（topic は英小文字で始まり、英小文字・数字・ハイフンだけ）

この場合、pub.dev の X はタグより後のコミットから出ることになる。違いをユーザーに伝える。

## 7. macOS 用アプリ

公開済みのリリースにファイルを足すので、実行の前に一言確認する。

```sh
Apps/LusterMac/notarize.sh
gh release upload X Apps/LusterMac/.build/Luster-X-macos.zip
```

`notarize.sh` は Developer ID で署名してビルドし（hardened runtime とタイムスタンプ付き）、Apple の公証に出して、ステープルまでする。認証情報はキーチェーンの notarytool のプロファイル `luster-notary`（`xcrun notarytool store-credentials luster-notary --apple-id … --team-id 7G2J8YMD3U` で一度だけ保存。パスワードの入力があるのでユーザーが実行する）。アプリの版番号は `rust/Cargo.toml` から入る。

確認:
- 出力の `"status":"Accepted"`、`The staple and validate action worked!`、`source=Notarized Developer ID`
- 失敗すると Apple のログを表示して止まる。hardened runtime で拒否されるものがないかを読む
- 添付のあと、`https://github.com/famio/Luster/releases/download/X/Luster-X-macos.zip` がログインなしで 200
- 必要なら、zip を展開して `xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" Luster.app` でダウンロードの印を付け、`spctl --assess --type execute --verbose=2 Luster.app` が `accepted`

## 8. 仕上げ

- README の Install 欄の版番号を X にして、コミット・push（Kotlin の `dev.famio:luster:X`・`luster-compose:X`、pub の `^X`、SwiftPM の `from:`）
- 採点を見る（https://pub.dev/packages/luster/score 、公開から 1 時間以内に出る）。基準は 160/160。「Package is not compatible with the Flutter SDK」という SDK issues は、flutter_gpu の `dart:nativewrappers` が原因で点に影響しないので無視してよい
- 必要なら利用者の立場で試す
  - Flutter: 新しいアプリに `luster: ^X` を入れ、`/opt/homebrew/bin` から rustup を除いた PATH で `fvm flutter build macos --debug`。`build/macos/**/precompiled/<hash>/` にファイルが取ってこられていれば、ビルド済みエンジンが使われている（rustup があると cargokit は既定でソースからビルドする）
  - Swift: 一時的なパッケージで `.package(url: "https://github.com/famio/Luster.git", from: "X")` を `swift package resolve` と `swift build`

## 失敗したとき

- **5 より前（何も公開していない）**: draft とタグを消し、直してコミット・push してから 2 からやり直す。
  ```sh
  gh release delete X --yes
  git push origin :refs/tags/X
  git tag -d X
  ```
  zip はビルドし直すのでチェックサムも変わる。main には `chore(release): X` の組がもう 1 つ増えるが、公開済みの main の履歴は書き換えない。
- **5 より後**: GitHub のリリースと Maven Central は取り消せず、タグも作り直せない。直すなら次のパッチ版で出す。pub.dev だけ未公開なら、直した main から出してよい（タグとの違いを伝える）。

## 参考

- secrets（Repository secrets）: `MAVEN_CENTRAL_USERNAME`・`MAVEN_CENTRAL_PASSWORD`（Central Portal のトークン）、`SIGNING_KEY`・`SIGNING_PASSWORD`（GPG。指紋 `25AB56939599B26BCAB9D3342B7D1936ADB8B9A9`、有効期限 2029-10-03）、`CARGOKIT_PRIVATE_KEY`（ビルド済みエンジンの署名。公開鍵は `rust/crates/luster-dart/cargokit.yaml`）
- pub.dev のパブリッシャーは `famio.dev`
