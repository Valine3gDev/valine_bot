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
podman quadlet install -r \
  https://raw.githubusercontent.com/Valine3gDev/valine_bot/vX.Y.Z/deploy/valine-bot.quadlets
```

このリポジトリの GHCR image は release workflow で `ghcr.io/valine3gdev/valine_bot:2.4.2` のような `v` なしの semver tag として発行されます。別バージョンを使う場合は、インストール後に `$HOME/.config/containers/systemd/valine-bot.container` の `Image=` を更新してください。

## 更新

イメージタグなどを変更する場合:

```sh
nano $HOME/.config/containers/systemd/valine-bot.container

systemctl --user daemon-reload
systemctl --user restart valine-bot.service
```

ユーザーを切り替えずに更新する場合:

```sh
sudo -u valine-bot -H sh -lc '${EDITOR:-nano} "$HOME/.config/containers/systemd/valine-bot.container"'
sudo systemctl --user --machine=valine-bot@.host daemon-reload
sudo systemctl --user --machine=valine-bot@.host restart valine-bot.service
```

## env file

サンプルをダウンロードして、実際のパスワードに書き換えてからインストールします。

```sh
curl -fsSLo valine-bot.env \
  https://raw.githubusercontent.com/Valine3gDev/valine_bot/vX.Y.Z/deploy/valine-bot.env.sample

${EDITOR:-nano} valine-bot.env

install -D -m 600 valine-bot.env "$HOME/.config/containers/systemd/valine-bot.env"
rm -f valine-bot.env
```

## config file

`$HOME/valine-bot/config.toml` を直接参照します。このファイルはコンテナ内の `/app/config.toml` として read-only mount されます。

```sh
mkdir -p "$HOME/valine-bot"
chmod 700 "$HOME/valine-bot"

curl -fsSLo "$HOME/valine-bot/config.toml" \
  https://raw.githubusercontent.com/Valine3gDev/valine_bot/vX.Y.Z/config.sample.toml

${EDITOR:-nano} "$HOME/valine-bot/config.toml"
chmod 600 "$HOME/valine-bot/config.toml"
```

## 起動

```sh
systemctl --user daemon-reload
systemctl --user start valine-bot-db.service valine-bot.service
```

## 状態確認

```sh
systemctl --user status valine-bot-db.service
systemctl --user status valine-bot.service
```

## ログ確認

```sh
journalctl --user -axu valine-bot.service -f
journalctl --user -axu valine-bot-db.service -f
```

ユーザーを切り替えずに
```sh
sudo -u valine-bot journalctl --user -axu valine-bot.service -f
sudo -u valine-bot journalctl --user -axu valine-bot-db.service -f
```

## 停止

```sh
systemctl --user stop valine-bot.service valine-bot-db.service
```

## コンフィグ検証

```sh
podman run --rm \
  --read-only \
  --cap-drop=all \
  --userns=keep-id \
  --user "$(id -u):$(id -g)" \
  --volume "$HOME/valine-bot/config.toml:/app/config.toml:ro" \
  ghcr.io/valine3gdev/valine_bot:2.4.2 \
  --check-config
```
