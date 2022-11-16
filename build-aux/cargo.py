#!/usr/bin/env python3

import sys
import subprocess
import os
import shutil

env = os.environ

MESON_BUILD_ROOT=sys.argv[1]
MESON_SOURCE_ROOT=sys.argv[2]
CARGO_TARGET_DIR = os.path.join (MESON_BUILD_ROOT, "target")
env["CARGO_TARGET_DIR"] = CARGO_TARGET_DIR
CARGO_HOME = os.path.join (CARGO_TARGET_DIR, "cargo-home")
env["CARGO_HOME"] = CARGO_HOME
OUTPUT=sys.argv[3]
BUILDTYPE=sys.argv[4]
APP_BIN=sys.argv[5]


if BUILDTYPE  == "release":
    print("RELEASE MODE")
    CMD = ['cargo', 'build', '--manifest-path', os.path.join(MESON_SOURCE_ROOT, 'Cargo.toml'), '--release']
    subprocess.run(CMD, env=env)
    shutil.copyfile(os.path.join(CARGO_TARGET_DIR, "release", APP_BIN), OUTPUT)
else:
    print("DEBUG MODE")
    CMD = ['cargo', 'build', '--manifest-path', os.path.join(MESON_SOURCE_ROOT, 'Cargo.toml')]
    subprocess.run(CMD, env=env)
    shutil.copyfile(os.path.join(CARGO_TARGET_DIR, "debug", APP_BIN), OUTPUT)


