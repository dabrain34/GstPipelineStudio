# Create a release

- Update to the given version:
  - meson.build
  - cargo.toml
- create a tag on gitlab
- meson builddir -Dbuildtype=release
- ninja -C buiddir dist
- upload the package and the sha256 to gitlab for Flatpak in the release notes
