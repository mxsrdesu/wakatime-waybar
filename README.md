# wakatime-waybar

a little daily wakatime counter for waybar.

## install

requires linux, waybar and rust 1.89 or newer.

```sh
cargo install --path . --locked
```

add `~/.cargo/bin` to waybar's path. dashboard uses `xdg-open`;
refresh uses `pkill` from procps. settings opens `wakatime-desktop`;
set `WAKATIME_DESKTOP_EXECUTABLE` if yours lives elsewhere.

copy `assets/menu.xml` and `assets/icon.svg` to `~/.config/waybar/wakatime/`.
merge [the module](examples/waybar.jsonc) into your config and add
`custom/wakatime` between pomodoro and dnd. signal 8 must be free.

for nix, use `nix build` or [home manager](examples/home-manager.nix).
the nix package includes menu assets, `xdg-open` and `pkill`.
the patched desktop app is optional: pass `wakatimeDesktop` to `package.nix`
or set its executable yourself. this repo doesn't install a tracker.

## usage

shows today's account total: `7h 16m`, `16m` or a real `0m`.
reads the existing key from `~/.wakatime.cfg` and uses wakatime's timezone.
updates every two minutes; panels share a cache and fetch lock.

left-click opens dashboard, refresh and settings.
on failure, today's last value stays with `!`; without one, it shows `—`.
`waybar-wakatime status` prints json, including english diagnostics.
`refresh` respects rate limits and coalesces clicks within five seconds.

cache: `$XDG_CACHE_HOME/waybar-wakatime`, or `~/.cache/waybar-wakatime`.
only totals, dates, timestamps and fixed error codes are saved.
existing python cache files are readable. keys and project names aren't saved.

## style

use your waybar config and external css for colours, fonts, borders and spacing.
add `#custom-wakatime` to your shared module selectors.
classes: `ready`, `stale`, `loading`, `error`.
[the css example](examples/style.css) only adds icon placement and state colours;
it isn't installed into your active stylesheet.

for a different icon colour:

```sh
python3 scripts/make-icon.py ~/.config/waybar/wakatime/icon.svg --color '#ebdbb2'
```

python is only a helper for recolouring the icon, not a runtime dependency.
the original macos menu icon and its licence are in [assets](assets/LICENSE.icon).
source: `wakatime/macos-wakatime@8c70917f84690dd43b4f818fc84f97b23948666d`,
`WakaTime/Resources/Assets.xcassets/WakaTime.imageset/64.png`.

## checks

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

[bsd 3-clause licence](LICENSE). the icon has its own licence.
