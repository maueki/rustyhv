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
    kernel = root / "target" / "aarch64-unknown-none-softfloat" / profile / "rustyhv"
    config = tomllib.loads((root / ".cargo" / "config.toml").read_text())
    command = config["target"]["aarch64-unknown-none-softfloat"]["runner"] + [str(kernel)]
    with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE) as process:
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        prompt = b"rustyhv> "

        def receive(prompts=1, ending=prompt):
            output = bytearray()
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                for key, _ in selector.select(max(0, deadline - time.monotonic())):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        raise AssertionError(f"QEMU exited: {process.poll()}")
                    output.extend(chunk)
                    if output.count(prompt) >= prompts and output.endswith(ending):
                        assert b"PANIC" not in output, output
                        return bytes(output)
            raise AssertionError(f"Timed out waiting for prompt: {bytes(output)!r}")

        def check(name, data, expected, prompts=1, verbose=True):
            process.stdin.write(data)
            process.stdin.flush()
            output = receive(prompts)
            for text in expected:
                assert text in output, (name, output)
            assert output.count(prompt) == prompts, (name, output)
            if verbose:
                print(f"PASS: {name}", flush=True)
            return output

        try:
            boot = receive()
            assert b"CurrentEL: EL1\r\nrustyhv shell (EL1). Type help.\r\n" in boot, boot
            # Linux の /proc で QEMU 全体の CPU 時間を測る。入力待機で1コアを占有しない。
            def cpu_seconds():
                fields = Path(f"/proc/{process.pid}/stat").read_text().rsplit(")", 1)[1].split()
                return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")

            before = cpu_seconds()
            time.sleep(1)
            used = cpu_seconds() - before
            assert used < 0.25, f"Idle consumed {used:.2f} CPU seconds in 1 second"
            print(f"PASS: idle CPU {used:.2f} seconds / 1 wall second", flush=True)
            # 1文字でも起床することを、入力を分割して確認する。
            for byte in b"help":
                process.stdin.write(bytes([byte]))
                process.stdin.flush()
                assert receive(prompts=0, ending=bytes([byte])) == bytes([byte])
                time.sleep(0.02)
            check("single-byte wakeup / LF", b"\n", [b"echo TEXT  Print TEXT\r\n"])
            check("info / CRLF", b"info\r\n", [b"Shell: EL1, console: EL2 via HVC\r\n"])
            check("echo / spaces", b"  echo hello  world  \r", [b"\r\nhello  world\r\n"])
            check("Backspace", b"helx\x08p\r", [b"\x08 \x08", b"Show commands\r\n"])
            check("Delete", b"helx\x7fp\r", [b"\x08 \x08", b"Show commands\r\n"])
            check("empty line", b"\r\n", [b"\r\n"])
            check("unknown command", b"missing\r", [b"Unknown command; type help\r\n"])
            check("invalid arguments", b"info extra\r", [b"This command takes no arguments\r\n"])
            app_return = b"\r\nHello from app!\r\n" + prompt
            check("run hello at EL0", b"run hello\r", [app_return])
            check("shell resumes after app", b"echo app returned\r", [b"\r\napp returned\r\n"])
            check("run hello again", b"run   hello\r", [app_return])
            for _ in range(20):
                check("repeated EL0 app", b"run hello\r", [app_return], verbose=False)
            print("PASS: 20 EL0 app exits and shell returns", flush=True)
            check("missing app name", b"run\r", [b"Usage: run hello\r\n"])
            check("unknown app", b"run missing\r", [b"Unknown app; available: hello\r\n"])
            extra = check("extra app arguments", b"run hello extra\r", [b"Usage: run hello\r\n"])
            assert b"Hello from app!" not in extra, extra
            check("128-byte line", b"echo " + b"x" * 123 + b"\r", [b"\r\n" + b"x" * 123 + b"\r\n"])
            overflow = check("overflow", b"echo " + b"x" * 124 + b"\r", [b"Line too long (max 128 bytes); discarded\r\n"])
            assert b"\r\n" + b"x" * 123 + b"\r\n" not in overflow, overflow
            check("recovery after overflow", b"echo recovered\r", [b"\r\nrecovered\r\n"])
            check("Ctrl+C", b"echo canceled\x03", [b"^C\r\n"])
            check("recovery after Ctrl+C", b"echo ready\r", [b"\r\nready\r\n"])
            check("non-ASCII input ignored", b"echo a\xffb\r", [b"\r\nab\r\n"])
            check("two commands", b"echo one\r\necho two\n", [b"\r\none\r\n", b"\r\ntwo\r\n"], prompts=2)
            # 待機に入る前後の入力を繰り返し、保留割込みを取りこぼさないことを確認。
            for _ in range(20):
                check("repeated wakeup", b"echo wake\r", [b"\r\nwake\r\n"], verbose=False)
            print("PASS: 20 repeated wakeups", flush=True)
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
