#!/usr/bin/env python3

"""Run a command with piped stdin and a real controlling terminal on stdout."""

import fcntl
import os
import pty
import struct
import subprocess
import sys
import termios


def attach_controlling_terminal() -> None:
    os.setsid()
    fcntl.ioctl(1, termios.TIOCSCTTY, 0)
    os.tcsetpgrp(1, os.getpgrp())


def main() -> int:
    if len(sys.argv) < 2:
        raise SystemExit("usage: pipe-with-tty.py COMMAND [ARGUMENT ...]")

    piped_input = sys.stdin.buffer.read()
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    read_input, write_input = os.pipe()
    process = subprocess.Popen(
        sys.argv[1:],
        stdin=read_input,
        stdout=slave,
        stderr=slave,
        preexec_fn=attach_controlling_terminal,
        close_fds=True,
    )
    os.close(read_input)
    os.close(slave)

    try:
        with os.fdopen(write_input, "wb", closefd=True) as child_input:
            child_input.write(piped_input)
        while True:
            try:
                output = os.read(master, 8192)
            except OSError as error:
                if error.errno == 5:  # Linux PTYs report EIO after the slave closes.
                    break
                raise
            if not output:
                break
            sys.stdout.buffer.write(output)
            sys.stdout.buffer.flush()
    finally:
        os.close(master)

    return process.wait()


if __name__ == "__main__":
    raise SystemExit(main())
