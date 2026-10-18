"""Drives the engine through a real ibus-daemon: one input context, steps from a JSON script, and every signal the engine
sends printed as one JSON line each. Adapted from the earlier Linux port (shanjie-multiplatform linux/tests).

Script:
- "client": the input context's client name (default "shanjie-test"); the engine's app identity is its last ':' part.
- "settings": text written to $XDG_CONFIG_HOME/shanjie/settings.ini before the engine starts.
- "keys": [[keyval, keycode, state], ...], each sent as a press and then a release unless the entry has a fourth element
  "press" or "release"; ["focus_out"] / ["focus_in"] / ["reset"] call those; ["surrounding", text, cursor] sets the
  surrounding text; ["purpose", n] sets the content type's purpose; ["property", name, state] activates a menu item;
  ["wait", seconds] pumps the loop; ["type", "su3cl3"] types ASCII keycaps (US positions; ' ' is Space).
- "expect_commits": the commits in order; the run fails when they differ.
- "expect_settings": lines settings.ini must hold after the run.
"""
import json, os, subprocess, sys, time
import gi
gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus

RELEASE = 1 << 30
SHIFT = 1 << 0
# evdev codes of the US keycaps (linux/input-event-codes.h), unshifted and shifted.
ROWS = [("1234567890-=", "!@#$%^&*()_+", 2), ("qwertyuiop[]", "QWERTYUIOP{}", 16), ("asdfghjkl;'`", 'ASDFGHJKL:"~', 30),
        ("zxcvbnm,./", "ZXCVBNM<>?", 44)]
CAPS = {" ": (0x20, 57, 0), "\\": (0x5c, 43, 0), "|": (0x7c, 43, SHIFT)}
for plain, shifted, first in ROWS:
    for i, (a, b) in enumerate(zip(plain, shifted)):
        CAPS[a] = (ord(a), first + i, 0)
        CAPS[b] = (ord(b), first + i, SHIFT)
engine_bin, script_path = sys.argv[1], sys.argv[2]
address = os.environ["IBUS_ADDRESS"]
daemon = subprocess.Popen(["ibus-daemon", "--single", "--panel=disable", "--emoji-extension=disable",
                           "--config=disable", "--cache=none", "--address", address] + (["--verbose"] if os.environ.get("SHANJIE_DAEMON_LOG") else []),
                          stdout=subprocess.DEVNULL, stderr=None if os.environ.get("SHANJIE_DAEMON_LOG") else subprocess.DEVNULL)
engine = None
log = []

def pump(seconds=0.05):
    ctx = GLib.MainContext.default()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        while ctx.iteration(False):
            pass
        time.sleep(0.005)

def wait_for(predicate, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return True
        pump(0.05)
    return False

def text(t):
    return t.get_text() if t is not None else None

script = json.load(open(script_path))
try:
    bus = None
    def connected():
        global bus
        bus = bus or IBus.Bus.new()
        return bus.is_connected()
    if not wait_for(connected, 10):
        raise SystemExit("private ibus-daemon did not start")
    if "settings" in script:
        folder = os.path.join(os.environ["XDG_CONFIG_HOME"], "shanjie")
        os.makedirs(folder, exist_ok=True)
        with open(os.path.join(folder, "settings.ini"), "w", encoding="utf-8") as f:
            f.write(script["settings"])
    engine = subprocess.Popen([engine_bin], env=dict(os.environ))
    if not wait_for(lambda: bus.name_has_owner("org.freedesktop.IBus.Shanjie"), 30):
        raise SystemExit("engine did not register")
    ic = bus.create_input_context(script.get("client", "shanjie-test"))
    ic.set_capabilities(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.FOCUS | IBus.Capabilite.SURROUNDING_TEXT | IBus.Capabilite.PROPERTY)
    ic.connect("commit-text", lambda _ic, t: log.append({"commit": text(t)}))
    ic.connect("update-preedit-text", lambda _ic, t, cursor, visible: log.append({"preedit": text(t), "cursor": cursor, "visible": visible}))
    ic.connect("hide-preedit-text", lambda _ic: log.append({"preedit_hidden": True}))
    def lookup(_ic, table, visible):
        n = table.get_number_of_candidates()
        log.append({"lookup": [text(table.get_candidate(i)) for i in range(n)], "visible": visible,
                    "cursor": table.get_cursor_pos() if table.is_cursor_visible() else None,
                    "vertical": table.get_orientation() == IBus.Orientation.VERTICAL})
    ic.connect("update-lookup-table", lookup)
    ic.connect("hide-lookup-table", lambda _ic: log.append({"lookup_hidden": True}))
    ic.connect("update-auxiliary-text", lambda _ic, t, visible: log.append({"aux": text(t), "visible": visible}))
    ic.connect("hide-auxiliary-text", lambda _ic: log.append({"aux_hidden": True}))
    def props(_ic, plist):
        items = []
        i = 0
        while True:
            p = plist.get(i)
            if p is None:
                break
            sub = p.get_sub_props()
            entry = {"key": p.get_key(), "label": text(p.get_label()), "state": int(p.get_state()), "sensitive": p.get_sensitive()}
            if sub is not None and sub.get(0) is not None:
                entry["sub"] = []
                j = 0
                while sub.get(j) is not None:
                    q = sub.get(j)
                    entry["sub"].append({"key": q.get_key(), "label": text(q.get_label()), "state": int(q.get_state()), "sensitive": q.get_sensitive()})
                    j += 1
            items.append(entry)
            i += 1
        log.append({"properties": items})
    ic.connect("register-properties", props)
    ic.connect("forward-key-event", lambda _ic, kv, kc, st: log.append({"forward": [kv, kc, st]}))
    ic.connect("delete-surrounding-text", lambda _ic, off, n: log.append({"delete_surrounding": [off, n]}))
    ic.focus_in()
    # A runtime-registered component is only found by SetGlobalEngine (bus_ibus_impl's register_engine_list),
    # not by an input context's SetEngine; this daemon is private, so the global engine is ours.
    bus.set_global_engine("shanjie")
    pump(0.5)
    desc = ic.get_engine()
    if desc is None or desc.get_name() != "shanjie":
        raise SystemExit("input context did not switch to shanjie: %r" % (desc.get_name() if desc else None))
    for entry in script["keys"]:
        if entry[0] in ("focus_in", "focus_out", "reset"):
            log.append({"step": entry[0]})
            getattr(ic, entry[0])()
            pump(0.2)
            continue
        if entry[0] == "purpose":
            ic.set_content_type(entry[1], 0)
            pump(0.1)
            continue
        if entry[0] == "property":
            log.append({"step": "property", "name": entry[1]})
            ic.property_activate(entry[1], entry[2])
            pump(0.3)
            continue
        if entry[0] == "type":
            for ch in entry[1]:
                keyval, keycode, state = CAPS[ch]
                for phase in ("press", "release"):
                    st = state | (RELEASE if phase == "release" else 0)
                    handled = ic.process_key_event(keyval, keycode, st)
                    log.append({"key": [keyval, keycode, st], "handled": handled})
                    pump(0.02)
            pump(float(script.get("settle", 0.05)))
            continue
        if entry[0] == "wait":
            pump(float(entry[1]))
            continue
        if entry[0] == "surrounding":
            ic.set_surrounding_text(IBus.Text.new_from_string(entry[1]), entry[2], entry[2])
            pump(0.1)
            continue
        keyval, keycode, state = entry[:3]
        phases = [entry[3]] if len(entry) > 3 else ["press", "release"]
        for phase in phases:
            st = state | (RELEASE if phase == "release" else 0)
            handled = ic.process_key_event(keyval, keycode, st)
            log.append({"key": [keyval, keycode, st], "handled": handled})
            pump(0.05)
        pump(float(script.get("settle", 0.05)))
    pump(float(script.get("final_wait", 0.3)))
    for item in log:
        print(json.dumps(item, ensure_ascii=False))
    if "expect_commits" in script:
        commits = [item["commit"] for item in log if "commit" in item]
        if commits != script["expect_commits"]:
            print("FAIL commits %r != expected %r" % (commits, script["expect_commits"]), file=sys.stderr)
            sys.exit(1)
    if "expect_settings" in script:
        path = os.path.join(os.environ["XDG_CONFIG_HOME"], "shanjie", "settings.ini")
        lines = open(path, encoding="utf-8").read().splitlines() if os.path.exists(path) else []
        missing = [l for l in script["expect_settings"] if l not in lines]
        if missing:
            print("FAIL settings.ini lacks %r (has %r)" % (missing, lines), file=sys.stderr)
            sys.exit(1)
finally:
    if engine is not None:
        engine.terminate()
    daemon.terminate()
