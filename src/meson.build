conf = configuration_data()
conf.set_quoted('APP_ID', application_id)
conf.set_quoted('VERSION', version + version_suffix)

configure_file(
    input: 'config.rs.in',
    output: 'config.rs',
    configuration: conf
)

# Copy the config.rs output to the source directory.
run_command(
  'cp',
  join_paths(meson.build_root(), 'src', 'config.rs'),
  join_paths(meson.source_root(), 'src', 'config.rs'),
  check: true
)

rust_sources = files(
  'gps/pipeline.rs',
  'gps/element.rs',
  'gps/pad.rs',
  'gps/mod.rs',
  'graphmanager/graphview.rs',
  'graphmanager/link.rs',
  'graphmanager/mod.rs',
  'graphmanager/node.rs',
  'graphmanager/port.rs',
  'ui/about.rs',
  'ui/elements.rs',
  'ui/logger.rs',
  'ui/message.rs',
  'ui/mod.rs',
  'ui/properties.rs',
  'ui/treeview.rs',
  'app.rs',
  'common.rs',
  'logger.rs',
  'macros.rs',
  'main.rs',

  'settings.rs',
)

sources = [cargo_sources, rust_sources]

cargo_script = find_program(join_paths(meson.source_root(), 'build-aux/cargo.sh'))
cargo_release = custom_target(
  'cargo-build',
  build_by_default: true,
  input: sources,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    cargo_script,
    meson.build_root(),
    meson.source_root(),
    '@OUTPUT@',
    get_option('buildtype'),
    meson.project_name(),
  ]
)
