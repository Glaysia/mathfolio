"""Exercise the real Linux window in an owned Xvfb session (no Windows UI).

Requires Xvfb, openbox, xdotool, dbus-daemon and Pillow on the Linux test host.
Session records/PNGs belong in ignored artifacts/. Commands operate only on the
display/processes created by start, never an existing user desktop.
"""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def run(args, env=None):
    return subprocess.run(args, env=env, check=True, text=True, capture_output=True, timeout=15).stdout.strip()


def spawn(args, env, log):
    with open(log, "ab") as output:
        return subprocess.Popen(args, env=env, stdout=output, stderr=output, start_new_session=True)


def wait_until(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.1)
    raise TimeoutError("GUI test condition did not become ready")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["start", "click", "move", "type", "key", "snapshot", "windows", "stop"])
    parser.add_argument("session", type=Path)
    parser.add_argument("arguments", nargs="*")
    args = parser.parse_args()
    if args.command == "start":
        args.session.mkdir(parents=True, exist_ok=False)
        application = Path(args.arguments[0]).resolve()
        display_number = next(n for n in range(88, 120) if not Path(f"/tmp/.X{n}-lock").exists())
        env = os.environ.copy()
        runtime = args.session.resolve() / "runtime"
        runtime.mkdir(mode=0o700)
        env.update(DISPLAY=f":{display_number}", XDG_RUNTIME_DIR=str(runtime), XDG_SESSION_TYPE="x11",
                   XDG_CURRENT_DESKTOP="GNOME", LIBGL_ALWAYS_SOFTWARE="1", WINIT_UNIX_BACKEND="x11")
        server = spawn(["Xvfb", env["DISPLAY"], "-screen", "0", "1440x1050x24", "-nolisten", "tcp"], env, args.session / "xvfb.log")
        wait_until(lambda: Path(f"/tmp/.X11-unix/X{display_number}").exists())
        bus = run(["dbus-daemon", "--session", "--fork", "--print-address=1", "--print-pid=1"], env).splitlines()
        env["DBUS_SESSION_BUS_ADDRESS"] = bus[0]
        run(["dbus-update-activation-environment", "DISPLAY", "XDG_RUNTIME_DIR", "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP"], env)
        manager = spawn(["openbox"], env, args.session / "openbox.log")
        source = args.session.resolve() / "worksheet.mathcraft"
        source.write_text(json.dumps(dict(version=2, title="GUI test worksheet", width=816.0, height=1056.0,
                                         grid=19.2, next_id=1, regions=[], native_archive=None)))
        app = spawn([str(application), "--open", str(source)], env, args.session / "application.log")

        def window():
            if app.poll() is not None:
                raise RuntimeError(f"Application exited {app.returncode}; inspect application.log")
            try:
                return run(["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)], env).splitlines()[0]
            except (subprocess.CalledProcessError, IndexError):
                return None

        window_id = wait_until(window)
        run(["xdotool", "windowactivate", "--sync", window_id], env)
        record = dict(display=env["DISPLAY"], environment={k: env[k] for k in ["DISPLAY", "XDG_RUNTIME_DIR", "XDG_CURRENT_DESKTOP", "DBUS_SESSION_BUS_ADDRESS"]},
                      xvfb_pid=server.pid, manager_pid=manager.pid, bus_pid=int(bus[1]), application_pid=app.pid,
                      window_id=window_id, worksheet=str(source))
        (args.session / "session.json").write_text(json.dumps(record, indent=2))
        print(json.dumps(record))
        return
    record = json.loads((args.session / "session.json").read_text())
    env = os.environ.copy()
    env.update(record["environment"])
    if args.command in ("click", "move"):
        run(["xdotool", "mousemove", "--sync", *args.arguments[:2]], env)
        if args.command == "click":
            run(["xdotool", "click", "1"], env)
    elif args.command == "type":
        run(["xdotool", "type", "--clearmodifiers", "--delay", "70", "--", args.arguments[0]], env)
    elif args.command == "key":
        run(["xdotool", "key", "--clearmodifiers", *args.arguments], env)
    elif args.command == "windows":
        for window_id in run(["xdotool", "search", "--onlyvisible", "--name", ".*"], env).splitlines():
            print(window_id, run(["xdotool", "getwindowname", window_id], env))
    elif args.command == "snapshot":
        from PIL import ImageGrab
        destination = args.session / args.arguments[0]
        ImageGrab.grab(xdisplay=record["display"]).save(destination)
        print(destination)
    elif args.command == "stop":
        for key in ["application_pid", "manager_pid", "bus_pid", "xvfb_pid"]:
            try:
                environment = Path(f"/proc/{record[key]}/environ").read_bytes().split(b"\0")
                if f"DISPLAY={record['display']}".encode() not in environment:
                    raise RuntimeError("Refusing to stop a process outside the owned display")
                os.kill(record[key], signal.SIGTERM)
            except (ProcessLookupError, FileNotFoundError):
                pass


if __name__ == "__main__":
    main()
