#!/usr/bin/env python3
"""Build this checkout and install LazyAPI for the current user."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


REPOSITORY = Path(__file__).resolve().parents[1]


def atomic_copy(source, destination, mode):
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{destination.name}-", dir=destination.parent)
    os.close(descriptor)
    try:
        shutil.copyfile(source, temporary)
        os.chmod(temporary, mode)
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def build():
    process = subprocess.Popen(
        ["cargo", "build", "--release", "--locked", "--bin", "lazyapi",
         "--message-format=json-render-diagnostics"],
        cwd=REPOSITORY,
        stdout=subprocess.PIPE,
        text=True,
    )
    executable = None
    for line in process.stdout:
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message["target"]["name"] == "lazyapi"
                and message.get("executable")):
            executable = Path(message["executable"])
    status = process.wait()
    if status:
        raise SystemExit(status)
    if executable is None:
        raise SystemExit("Cargo did not report a LazyAPI executable.")
    return executable


def main():
    binary = build()
    version = subprocess.check_output([str(binary), "--version"], text=True).strip()
    if not version.startswith("lazyapi "):
        raise SystemExit(f"Unexpected executable identity: {version}")

    installed = Path.home() / ".local/bin/lazyapi"
    data_root = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
    config_root = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    state = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state") / "lazyapi"
    state.mkdir(parents=True, exist_ok=True, mode=0o700)

    with tempfile.TemporaryDirectory(prefix="lazyapi-install-") as temporary:
        staging = Path(temporary)
        generation_env = {**os.environ, "XDG_CONFIG_HOME": str(staging / "config")}
        for shell in ("bash", "zsh", "fish"):
            subprocess.run(
                [str(binary), "completions", shell, str(staging)],
                env=generation_env, check=True, stdout=subprocess.DEVNULL,
            )
        subprocess.run(
            [str(binary), "man", str(staging)],
            env=generation_env, check=True, stdout=subprocess.DEVNULL,
        )
        assets = [
            (staging / "lazyapi.bash", data_root / "bash-completion/completions/lazyapi"),
            (staging / "_lazyapi", data_root / "zsh/site-functions/_lazyapi"),
            (staging / "lazyapi.fish", config_root / "fish/completions/lazyapi.fish"),
            (staging / "lazyapi.1", data_root / "man/man1/lazyapi.1"),
            (REPOSITORY / "LICENSE", data_root / "lazyapi/LICENSE"),
        ]
        for source, destination in assets:
            atomic_copy(source, destination, 0o644)
        if installed.is_file():
            atomic_copy(installed, state / "previous/lazyapi", 0o700)
        atomic_copy(binary, installed, 0o755)

    expected_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    if hashlib.sha256(installed.read_bytes()).hexdigest() != expected_hash:
        raise SystemExit("Installed executable checksum does not match the build.")
    subprocess.run([str(installed), "--version"], check=True)
    record = {
        "repository": str(REPOSITORY),
        "binary": str(installed),
        "version": version,
        "sha256": expected_hash,
        "installed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    }
    with tempfile.TemporaryDirectory(prefix="lazyapi-record-") as temporary:
        metadata = Path(temporary) / "install.json"
        metadata.write_text(json.dumps(record, indent=2) + "\n")
        atomic_copy(metadata, state / "install.json", 0o600)
    print(f"Installed {installed}")
    print("Collections and environments were left untouched.")
    if shutil.which("lazyapi") != str(installed):
        print(f"Add {installed.parent} to PATH to use this installation.")


if __name__ == "__main__":
    main()
