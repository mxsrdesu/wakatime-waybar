"""GTK3 preview with fixture data from the real Rust executable; not live Waybar.
Usage: preview.py BINARY OUTPUT_DIRECTORY CURRENT_WAYBAR_STYLE
Run on an isolated Xvfb display. Menu actions use temporary command stubs.
"""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk

binary, output, base_style = map(Path, sys.argv[1:])
binary = binary.resolve()
output.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[1]
profile = tempfile.TemporaryDirectory(prefix="wakatime-preview-")
home = Path(profile.name)
cache = home / "cache/waybar-wakatime"
cache.mkdir(parents=True)
log = home / "commands"
for name in ("xdg-open", "pkill", "desktop"):
    script = home / name
    script.write_text(f'#!{shutil.which("sh")}\nprintf "%s\\n" "{name}:$*" >> "{log}"\n')
    script.chmod(0o700)
env = dict(os.environ, HOME=str(home), XDG_CACHE_HOME=str(home / "cache"),
           PATH=str(home), WAKATIME_DESKTOP_EXECUTABLE=str(home / "desktop"))
now = time.time()
data = {"date": datetime.now(timezone.utc).strftime("%Y-%m-%d"), "timezone": "UTC",
        "total_seconds": 26162.608, "fetched_at": now}

def fixture(error=None, with_data=True):
    state = {"attempt_at": now - 10, "next_request_at": now + 120}
    if with_data:
        state["data"] = data
    if error:
        state["error"] = error
    (cache / "status.json").write_text(json.dumps(state))
    return json.loads(subprocess.check_output([str(binary), "status"], env=env))

samples = [fixture(), fixture("network"), fixture("pending", False), fixture("configuration", False)]
fixture()  # initial menu actions operate on a real current-day cache
css = base_style.read_text().replace("#custom-dnd,", "#custom-dnd,\n#custom-wakatime,")
icon_css = (root / "examples/style.css").read_text().replace('url("wakatime/icon.svg")', f'url("{root / "assets/icon.svg"}")')
provider = Gtk.CssProvider()
provider.load_from_data((css + "\n" + icon_css).encode())
Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
window = Gtk.Window()
window.set_name("waybar")
window.set_title("wakatime fixture preview")
window.set_decorated(False)
window.set_default_size(860, 370)
window.move(0, 0)
outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
window.add(outer)
labels = []
for caption, sample in zip(("ready", "stale", "pending", "unavailable"), samples):
    row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
    outer.pack_start(row, False, False, 0)
    for name, text in (("clock", "Mon 19 Oct  07:00 PM"), ("custom-pomodoro", "25:00"), ("custom-wakatime", sample["text"]), ("custom-dnd", "DND: off")):
        label = Gtk.Label(label=text)
        label.set_name(name)
        label.set_has_tooltip(False)
        if name == "custom-wakatime":
            for cls in sample["class"]:
                label.get_style_context().add_class(cls)
            labels.append(label)
        row.pack_start(label, False, False, 0)
    row.pack_start(Gtk.Label(label=caption), False, False, 0)
builder = Gtk.Builder.new_from_file(str(root / "assets/menu.xml"))
menu = builder.get_object("menu")
actions = []

def activate(item, action):
    result = subprocess.run([str(binary), action], env=env, capture_output=True, text=True, check=True)
    actions.append(action)
    if action == "refresh":
        refreshed = json.loads(result.stdout)
        assert refreshed["text"] == "7h 16m !"
        assert refreshed["class"] == ["stale"]
        assert json.loads((cache / "status.json").read_text())["error"] == "configuration"

for action in ("dashboard", "refresh", "settings"):
    builder.get_object(action).connect("activate", activate, action)

def position(menu, *unused):
    x, y = labels[0].translate_coordinates(window, 0, 0)
    return x, y + labels[0].get_allocated_height(), True

def capture():
    pixbuf = Gdk.pixbuf_get_from_window(Gdk.get_default_root_window(), 0, 0, 860, 370)
    assert pixbuf is not None
    pixbuf.savev(str(output / "preview.png"), "png", [], [])
    menu.popdown()
    GLib.timeout_add(150, capture_states)
    return False

def capture_states():
    pixbuf = Gdk.pixbuf_get_from_window(Gdk.get_default_root_window(), 0, 0, 860, 370)
    pixbuf.savev(str(output / "states.png"), "png", [], [])
    for action in ("dashboard", "refresh", "settings"):
        builder.get_object(action).activate()
    assert actions == ["dashboard", "refresh", "settings"]
    commands = log.read_text()
    assert "xdg-open:https://wakatime.com/dashboard" in commands
    assert "pkill:-RTMIN+8" in commands
    assert "desktop:wakatime://settings/" in commands
    print("GTK3 preview: real Rust JSON in four states; native menu callbacks execute CLI; refresh persisted a real cache error with retained total. External commands were stubs.")
    menu.popdown()
    Gtk.main_quit()
    return False

def popup():
    menu.show_all()
    menu.popup(None, None, position, None, 0, Gtk.get_current_event_time())
    GLib.timeout_add(250, capture)
    return False

icon = GdkPixbuf.Pixbuf.new_from_file(str(root / "assets/icon.svg"))
pixels = icon.get_pixels()
assert icon.get_has_alpha()
opaque = [pixels[i:i + 4] for i in range(0, len(pixels), 4) if pixels[i + 3] > 200]
assert opaque and all(abs(p[0] - 235) <= 1 and abs(p[1] - 219) <= 1 and abs(p[2] - 178) <= 1 for p in opaque)
window.show_all()
GLib.timeout_add(250, popup)
Gtk.main()
profile.cleanup()
