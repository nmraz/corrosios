#!/bin/sh

# This script is a workaround for the C/C++ extension's debug server launch
# behavior, which always launches the server in the *debugger's* directory (as
# opposed to some more sane CWD). This script ensures that we are at the project
# root before invoking cargo.

SCRIPT=$(readlink -f "$0")
cd $(dirname $SCRIPT)
cargo "$@"
