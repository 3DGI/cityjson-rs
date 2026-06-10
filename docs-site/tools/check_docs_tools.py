from __future__ import annotations

import shutil
import subprocess
import sys

REQUIRED_COMMANDS = ["doxygen", "dot", "uv", "node", "npm", "cargo", "rustdoc", "rustup", "cbindgen"]


def command_output(args: list[str]) -> str:
    result = subprocess.run(args, check=True, capture_output=True, text=True)
    return result.stdout.strip() or result.stderr.strip()


def main() -> None:
    failures: list[str] = []
    for command in REQUIRED_COMMANDS:
        if shutil.which(command) is None:
            failures.append(f"missing required docs tool: {command}")

    if shutil.which("rustdoc") is not None:
        try:
            version = command_output(["rustdoc", "+nightly", "--version"])
        except (subprocess.CalledProcessError, FileNotFoundError) as error:
            failures.append(f"nightly rustdoc is unavailable: {error}")
        else:
            if "nightly" not in version:
                failures.append(f"rustdoc +nightly did not report a nightly toolchain: {version}")

    if shutil.which("cargo") is not None:
        try:
            command_output(["cargo", "+nightly", "rustdoc", "--help"])
        except (subprocess.CalledProcessError, FileNotFoundError) as error:
            failures.append(f"cargo +nightly rustdoc is unavailable: {error}")

    if shutil.which("rustup") is not None:
        try:
            installed_targets = command_output(["rustup", "target", "list", "--toolchain", "nightly", "--installed"])
        except (subprocess.CalledProcessError, FileNotFoundError) as error:
            failures.append(f"could not inspect nightly Rust targets: {error}")
        else:
            if "wasm32-unknown-unknown" not in installed_targets.splitlines():
                failures.append(
                    "missing required nightly Rust target: wasm32-unknown-unknown "
                    "(run `rustup target add --toolchain nightly wasm32-unknown-unknown`)"
                )

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        raise SystemExit(1)

    print("docs tool setup ok")


if __name__ == "__main__":
    main()
