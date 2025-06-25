#!/bin/bash

for T in \
  x86_64-unknown-linux-gnu \
  x86_64-pc-windows-gnu \
  # aarch64-unknown-linux-gnu \
  # aarch64-apple-darwin
do
  cross build --release --target $T
done
