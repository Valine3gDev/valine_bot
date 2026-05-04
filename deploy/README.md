# Valine Bot デプロイ

## 専用ユーザー

専用ユーザー `valine-bot` を作り、パスワードをロックします。

```sh
if ! id valine-bot >/dev/null 2>&1; then
  sudo useradd -m -s /bin/bash valine-bot
fi

sudo passwd -l valine-bot
sudo loginctl enable-linger valine-bot
```

以降の作業は専用ユーザーに切り替えて行います。

```sh
sudo -iu valine-bot
```

## インストール

`main` ではなく、release tag または commit SHA に固定した raw URL を使ってください。

```sh
podman quadlet install \
  https://raw.githubusercontent.com/Valine3gDev/valine_bot/vX.Y.Z/deploy/valine-bot.quadlets
```

このリポジトリの GHCR image は release workflow で `ghcr.io/valine3gdev/valine_bot:2.4.2` のような `v` なしの semver tag として発行されます。別バージョンを使う場合は、インストール後に `$HOME/.config/containers/systemd/valine-bot.container` の `Image=` を更新してください。

## env file

サンプルをダウンロードして、実際のパスワードに書き換えてからインストールします。

```sh
curl -fsSLo valine-bot.env \
  https://raw.githubusercontent.com/Valine3gDev/valine_bot/vX.Y.Z/deploy/valine-bot.env.sample

${EDITOR:-vi} valine-bot.env

install -D -m 600 valine-bot.env "$HOME/.config/containers/systemd/valine-bot.env"
rm -f valine-bot.env
```

## 起動

```sh
systemctl --user daemon-reload
systemctl --user enable --now valine-bot-db.service valine-bot.service
```

## 状態確認

```sh
systemctl --user status valine-bot-db.service
systemctl --user status valine-bot.service
```

## ログ確認

```sh
journalctl --user -u valine-bot.service -f
journalctl --user -u valine-bot-db.service -f
```

## 停止

```sh
systemctl --user stop valine-bot.service valine-bot-db.service
```

自動起動も無効にする場合:

```sh
systemctl --user disable valine-bot.service valine-bot-db.service
```

## unit 名

- `valine-bot-db.container` から `valine-bot-db.service` が生成されます。
- `valine-bot.container` から `valine-bot.service` が生成されます。
- `valine-bot.network` から network 用 unit (`valine-bot-network.service`) が生成されます。

## DB volume

DB データは Podman named volume の `valine-bot-db-data` に保存されます。rootless Podman では通常、実体は `~/.local/share/containers/storage/volumes/valine-bot-db-data/_data` 配下です。

```sh
podman volume inspect valine-bot-db-data
```
