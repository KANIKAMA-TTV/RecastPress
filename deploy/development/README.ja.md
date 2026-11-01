# 開発用 Docker / Compose 候補

2026-10-29 時点の静的な起動サンプル。この作業環境には Docker がなく、Docker build、Compose による構成解決、container 起動、native Linux 実行は未検証である。Linux 向け `cargo check` の既存成功も、この image の起動成功を代替しない。製品配布・本番運用の完成を示すものではない。

## 呼び出し側で用意するもの

builder と runtime image は必須指定で、既定 tag や架空 digest は置いていない。別の Docker 利用可能な環境で公式 registry の実 manifest、対象 CPU architecture、取得した digest を確認し、`repository@sha256:実際のdigest` の参照を用意する。builder は Rust **1.96.x**、Cargo、POSIX shell、必要な linker を含む Linux image を選ぶ。Dockerfile は実 `rustc --version` の 1.96 系を検査する。別の Rust 系列へ自動更新しない。runtime は builder が生成する実行ファイルの dynamic loader / libc / 必要 shared library と互換であることを別途確認する。glibc で生成した binary を互換性未確認の musl image や scratch へ載せない。

image は信頼でき、DB 資格情報・不必要な environment・volume・起動 hook を内蔵しないものを使う。Compose に host environment や `.env` を container へ一括転送する指定はないが、呼び出し側 image 自身の設定を除去・監査する仕組みではない。

artifact 用には既存の専用絶対ディレクトリを作り、公開してよい `public.json` と `public.receipt.json` だけを入れる。DB 設定、private 記事、元入力、資格情報は置かない。Linux UID/GID 65532 が directory を traverse し両ファイルを読める permission が必要。receipt は対応する bundle に対して既存 `bundle-receipt` CLI で生成する。bundle の確認と preparation は [公開準備記事](../../docs/prepared-posts-development.ja.md) と [固定 HTTP 実験](../../docs/alpha-publication-development.ja.md) を参照する。receipt の SHA-256 は byte identity であり、署名や公開権限ではない。

## 実行例（別の Docker 利用可能な環境）

以下はリポジトリ root で行う PowerShell 例。四つの変数は、呼び出し側で確認した実値を設定する。image 参照をここで補完・推測しない。

```powershell
$env:RECASTPRESS_BUILDER_IMAGE = Read-Host 'Verified Rust 1.96.x builder image reference'
$env:RECASTPRESS_RUNTIME_IMAGE = Read-Host 'Verified compatible runtime image reference'
$env:RECASTPRESS_ARTIFACT_DIR = Read-Host 'Existing absolute public-only artifact directory'
$env:RECASTPRESS_PUBLIC_HOST = 'localhost:8080'
docker compose -f deploy/development/compose.yaml config
docker compose -f deploy/development/compose.yaml build public
docker compose -f deploy/development/compose.yaml up -d public
curl.exe -i -H 'Host: localhost:8080' http://127.0.0.1:8080/health/ready
docker compose -f deploy/development/compose.yaml logs public
docker compose -f deploy/development/compose.yaml down
```

`config` は構成の確認であり起動成功の検証ではない。`build` は network から base image と locked Cargo dependencies を取得し得る。選ぶ image の digest、registry、architecture、network policy を実行側で確認する。builder は `cargo build --locked --release --package recastpress-server --bin recastpress` のみを実行する。workspace の manifest 解決に必要な四 crate の source は context に含めるが、全 workspace の release binary や DB/PHP ツールを起動しない。

public は container 内 `0.0.0.0:8080`、host 側は **127.0.0.1:8080** のみを公開する。CLI の `--host` は許可 Host と一致させる。公開ページの path も curl 等で確認し、GET/HEAD/ETag と停止後再起動の挙動を native 環境で検証する。healthcheck は無効にしており、image 内の curl・shell を前提にしない。上記 curl は host 上の手動観測である。

management は `management` profile が通常無効で、port を host に公開せず、network を持たず container loopback だけに bind する。試験上必要な場合だけ明示して起動する。

```powershell
docker compose -f deploy/development/compose.yaml --profile management up -d management
docker compose -f deploy/development/compose.yaml --profile management logs management
docker compose -f deploy/development/compose.yaml --profile management down
```

management readiness は現状 503 で、認証後の管理 mutation route は提供していない。起動しただけで readiness が成功すると扱わない。profile を有効にしても DB service / DB environment / artifact mount は追加されない。

## build context と runtime の範囲

root `.dockerignore` は default deny で、Cargo manifest/lock、四 crate の source、LICENSE / NOTICE / LICENSES と当該 Dockerfile だけを許可する。`.reference`、`target`、`.git`、assets、credentials、artifact、その他の未選択ファイルは送らない。許可 source tree 内でも `.env`、秘密鍵等を再度除外する。ソースへ資格情報を直接埋め込んだ場合を検出する仕組みではない。build context を拡張する際は再確認が必要である。

runtime へは server binary とリポジトリの LICENSE / NOTICE.md / LICENSES を copy する。base image と依存 package の配布条件、実 binary ごとの全 license/notice 収集、対応 source 提供を含む配布レビューは未完了であり、これらの copy だけで完了を主張しない。

両 process に numeric nonroot user、read-only root filesystem、capability 全 drop、no-new-privileges を指定する。public の bind mount は公開 artifact directory 一つだけで read-only。Docker socket、host source、DB directory を mount しない。これらは開発候補の指定であり、native 実測した security boundary、OS-user 分離、resource/RSS 上限、network egress 制御を示さない。Compose の override や呼び出し側 image 設定による変更も実行側で確認する。

公式仕様の参照： [必須 interpolation](https://docs.docker.com/reference/compose-file/interpolation/)、[service/profile・bind・security 設定](https://docs.docker.com/reference/compose-file/services/)、[Dockerfile ARG/FROM と multi-stage](https://docs.docker.com/reference/dockerfile/)、[build context / dockerignore](https://docs.docker.com/build/concepts/context/)。この候補では local text / YAML の静的確認だけを行い、実 Docker による受入確認は残している。
