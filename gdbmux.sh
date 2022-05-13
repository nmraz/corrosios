#!/bin/bash

cargo image
tmux split-pane -v "cargo gdb-attach"
cargo qemu --headless --gdbserver
