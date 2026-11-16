#!/bin/sh
set -eu

app_dir=$(cd "$(dirname "$0")/.." && pwd)
repo_dir=$(cd "$app_dir/.." && pwd)
build_dir="$app_dir/build"
bundle="$build_dir/Wally.app"
product=Wally
architectures="arm64 x86_64"

usage() {
    echo "usage: $(basename "$0") [--zip] [version]" >&2
    exit 64
}

fail() {
    echo "bundle: $*" >&2
    exit 1
}

version_from_cargo() {
    sed -n 's/^version *= *"\(.*\)"/\1/p' "$repo_dir/Cargo.toml" | head -n 1
}

parse_arguments() {
    make_zip=false
    version=""
    for argument in "$@"; do
        case "$argument" in
            --zip) make_zip=true ;;
            -h|--help) usage ;;
            -*) usage ;;
            *) [ -z "$version" ] || usage; version=${argument#v} ;;
        esac
    done
    [ -n "$version" ] || version=$(version_from_cargo)
    [ -n "$version" ] || fail "no version given and none found in Cargo.toml"
}

build_architecture() {
    swift build -c release --package-path "$app_dir" --product "$product" --arch "$1" >&2
    swift build -c release --package-path "$app_dir" --product "$product" --arch "$1" --show-bin-path
}

build_universal_binary() {
    set --
    for architecture in $architectures; do
        bin_path=$(build_architecture "$architecture")
        [ -x "$bin_path/$product" ] || fail "missing $bin_path/$product"
        set -- "$@" "$bin_path/$product"
        last_bin_path=$bin_path
    done
    mkdir -p "$bundle/Contents/MacOS"
    lipo -create -output "$bundle/Contents/MacOS/$product" "$@"
}

write_info_plist() {
    plist="$bundle/Contents/Info.plist"
    cp "$app_dir/Info.plist" "$plist"
    plutil -replace CFBundleShortVersionString -string "$version" "$plist"
    plutil -replace CFBundleVersion -string "$version" "$plist"
    plutil -lint "$plist" >/dev/null
    printf 'APPL????' > "$bundle/Contents/PkgInfo"
}

copy_resources() {
    resources="$bundle/Contents/Resources"
    mkdir -p "$resources"
    [ -f "$app_dir/Resources/AppIcon.icns" ] || fail "missing app/Resources/AppIcon.icns"
    cp "$app_dir/Resources/AppIcon.icns" "$resources/"
    found_localization=false
    for localization in "$app_dir"/Resources/*.lproj; do
        [ -d "$localization" ] || continue
        ditto "$localization" "$resources/$(basename "$localization")"
        found_localization=true
    done
    $found_localization || fail "no .lproj folder in app/Resources"
    for resource_bundle in "$last_bin_path"/*.bundle; do
        [ -d "$resource_bundle" ] || continue
        ditto "$resource_bundle" "$resources/$(basename "$resource_bundle")"
    done
}

sign_bundle() {
    xattr -cr "$bundle"
    codesign --force --deep --sign - "$bundle"
}

verify_bundle() {
    codesign --verify --deep --strict "$bundle"
    lipo -info "$bundle/Contents/MacOS/$product"
    for architecture in $architectures; do
        lipo "$bundle/Contents/MacOS/$product" -verify_arch "$architecture" || fail "binary lacks $architecture"
    done
}

zip_bundle() {
    archive="$build_dir/Wally-$version.zip"
    rm -f "$archive"
    ditto -c -k --norsrc --noextattr --noacl --keepParent "$bundle" "$archive"
    echo "$archive"
}

parse_arguments "$@"
rm -rf "$bundle"
mkdir -p "$build_dir"
build_universal_binary
write_info_plist
copy_resources
sign_bundle
verify_bundle
echo "$bundle ($version)"
if $make_zip; then
    zip_bundle
fi
