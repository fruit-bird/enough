# enough
A CLI tool to take control over distractions (currently only for macOS).

## Install
```bash
cargo install --git https://github.com/fruit-bird/enough
```

## Usage
Through the config file you create at `~/.config/enough/enough.yaml` (or by running `enough init`), you can define different profiles with websites and apps to block. You can then run `enough` to start blocking distractions.

```yaml
default-profile: lock-in
profiles:
  lock-in:
    duration: 1h30m
    websites:
      - https://www.youtube.com
      - https://reddit.com
```

Then run `enough` to start blocking distractions:

```bash
sudo enough block --profile=lock-in
sudo enough block --duration=2h # overrides duration, uses default profile
```

## CLI Commands
```
Usage: enough <COMMAND>

Commands:
  init         Initialize by creating a sample config file
  block        Block specified websites and apps
  status       Show current status
  profiles     List available profiles
  completions  Generate shell completions
  help         Print this message or the help of the given subcommand(s)
```

## What is This?
Yk the drill, every project is a learning opportunity. [SelfControl](https://github.com/SelfControlApp/selfcontrol/) is an amazing app that had me wondering how it worked. So I made this.

I later learned that [Raycast Focus](https://www.raycast.com/core-features/focus) is a similar thing that does it admittedly better. I will definitely be using that instead, as my implementation is definitely neither optimized nor stress-tested.

Regardless I still like seeing that config file in my dotfiles 😁
