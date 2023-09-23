# Create a release

- Update to the given version:
  - meson.build
  - cargo.toml
  - VERSION
  - index.html
  - And rebuild to regenerate the cargo.lock

- create a tag on gitlab
- Fetch the package from the `linux release` job or you can make it manually with:
  - meson builddir -Dbuildtype=release
  - ninja -C builddir dist
- Upload the package .xz file and the sha256 to gitlab release page in the release notes
see https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio/-/releases/0.3.2

# flathub

https://github.com/flathub/org.freedesktop.dabrain34.GstPipelineStudio

  - Need to update the package and the sha256 from the release page, ie https://gitlab.freedesktop.org/dabrain34/GstPipelineStudio/-/releases/0.3.2
  - Create a pull request with the package update
  - Wait at lest 2-3 hours after merging to get the update available.
