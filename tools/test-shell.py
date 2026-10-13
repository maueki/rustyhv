#!/usr/bin/env python3
"""QEMU にシリアル入力を送り、EL1 シェルの応答を確認する。"""

import argparse
import os
from pathlib import Path
import selectors
import subprocess
import time
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    build = ["cargo", "build", "--offline", "--features", "shell"]
    if args.release:
        build.append("--release")
    subprocess.run(build, cwd=root, env=dict(os.environ, CARGO_TARGET_DIR="target"), check=True)
    profile = "release" if args.release else "debug"
    kernel = root / "target" / "aarch64-unknown-none-softfloat" / profile / "ferrovisor"
    config = tomllib.loads((root / ".cargo" / "config.toml").read_text())
    command = config["target"]["aarch64-unknown-none-softfloat"]["runner"] + [str(kernel)]
    with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE) as process:
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        prompt = b"ferrovisor> "

        def receive(prompts=1):
            output = bytearray()
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                for key, _ in selector.select(max(0, deadline - time.monotonic())):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        raise AssertionError(f"QEMU exited: {process.poll()}")
                    output.extend(chunk)
                    if output.count(prompt) >= prompts and output.endswith(prompt):
                        assert b"PANIC" not in output, output
                        return bytes(output)
            raise AssertionError(f"Timed out waiting for prompt: {bytes(output)!r}")

        def check(name, data, expected, prompts=1):
            process.stdin.write(data)
            process.stdin.flush()
            output = receive(prompts)
            for text in expected:
                assert text in output, (name, output)
            assert output.count(prompt) == prompts, (name, output)
            print(f"PASS: {name}", flush=True)
            return output

        try:
            boot = receive()
            assert b"CurrentEL: EL1\r\nFerrovisor shell (EL1). Type help.\r\n" in boot, boot
            check("help / LF", b"help\n", [b"echo TEXT  Print TEXT\r\n"])
            check("info / CRLF", b"info\r\n", [b"Shell: EL1, console: EL2 via HVC\r\n"])
            check("echo / spaces", b"  echo hello  world  \r", [b"\r\nhello  world\r\n"])
            check("Backspace", b"helx\x08p\r", [b"\x08 \x08", b"Show commands\r\n"])
            check("Delete", b"helx\x7fp\r", [b"\x08 \x08", b"Show commands\r\n"])
            check("empty line", b"\r\n", [b"\r\n"])
            check("unknown command", b"missing\r", [b"Unknown command; type help\r\n"])
            check("invalid arguments", b"info extra\r", [b"This command takes no arguments\r\n"])
            check("128-byte line", b"echo " + b"x" * 123 + b"\r", [b"\r\n" + b"x" * 123 + b"\r\n"])
            overflow = check("overflow", b"echo " + b"x" * 124 + b"\r", [b"Line too long (max 128 bytes); discarded\r\n"])
            assert b"\r\n" + b"x" * 123 + b"\r\n" not in overflow, overflow
            check("recovery after overflow", b"echo recovered\r", [b"\r\nrecovered\r\n"])
            check("Ctrl+C", b"echo canceled\x03", [b"^C\r\n"])
            check("recovery after Ctrl+C", b"echo ready\r", [b"\r\nready\r\n"])
            check("non-ASCII input ignored", b"echo a\xffb\r", [b"\r\nab\r\n"])
            check("two commands", b"echo one\r\necho two\n", [b"\r\none\r\n", b"\r\ntwo\r\n"], prompts=2)
            # 実際の runner 設定で QEMU の終了キーも確認する。
            process.stdin.write(b"\x01x")
            process.stdin.flush()
            assert process.wait(timeout=3) == 0
            print("PASS: Ctrl+A X exits QEMU", flush=True)
        finally:
            selector.close()
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
