conf = configuration_data()
conf.set_quoted('APP_ID', application_id)
conf.set_quoted('VERSION', version + version_suffix)

configure_file(
    input: 'config.rs.in',
    output: 'config.rs',
    configuration: conf
)

# Copy the config.rs output to the source directory.
run_command(python3, '-c', 'import shutil; shutil.copyfile("@0@", "@1@")'.format(join_paths(meson.project_build_root(), 'src', 'config.rs'), join_paths(meson.project_source_root(), 'src', 'config.rs')))

rust_sources = files(
  'gps/player.rs',
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

cargo_script = find_program(join_paths(meson.project_source_root(), 'build-aux/cargo.py'))

app_name = meson.project_name()
if host_system == 'windows'
  app_name += '.exe'
endif

cargo_release = custom_target(
  'cargo-build',
  build_by_default: true,
  input: sources,
  output: app_name,
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    cargo_script,
    meson.project_build_root(),
    meson.project_source_root(),
    '@OUTPUT@',
    get_option('buildtype'),
    app_name,
  ]
)
