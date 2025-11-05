#!/bin/bash

# This script builds heimdallr and then copies the exec to ~/.config/niri

cargo build --release;
cp ~/Repositories/heimdallr/target/release/heimdallr ~/.config/niri/ \
    && echo -e "\n\033[0;32m\033[1mheimdallr built and copied to ~/.config/niri\033[0m" \
    || echo -e "\n\033[0;31m\033[1mheimdallr copying failed\033[0m";