export EXECUTABLE_PATH='target/debug/chinese-chess-bevy-rs'

cargo build --features 'bevy/dynamic_linking' $@

patchit() {
    export ORIGIN_DYLIB="$(patchelf --print-needed "$EXECUTABLE_PATH" | grep -x -E 'libbevy_dylib.*')"
    # export ORIGIN_LIBSTD="$(patchelf --print-needed "$EXECUTABLE_PATH" | grep -x -E 'libstd.*')"
    
    patchelf --replace-needed "$ORIGIN_DYLIB" "libbevy_dylib.so" "$EXECUTABLE_PATH"
    # patchelf --replace-needed "$ORIGIN_LIBSTD" "libstd.so" "$EXECUTABLE_PATH"
    patchelf --set-rpath "$(patchelf --print-rpath $EXECUTABLE_PATH):target/debug" "$EXECUTABLE_PATH"
}

if [ -r "$EXECUTABLE_PATH" ]; then
    patchit
fi

export RELEASE="target/release/chinese-chess-bevy-rs"
if [ -r "$RELEASE" ]; then
    export EXECUTABLE_PATH="$RELEASE"
    patchit
fi
