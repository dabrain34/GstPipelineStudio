project('gst_pipeline_studio',
          version: '0.2.0',
    meson_version: '>= 0.50.0',
  default_options: [ 'warning_level=2',
                   ],
          license: 'GPL-3.0-or-later',
)
i18n = import('i18n')

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.0.0')
dependency('gstreamer-1.0', version: '>= 1.18')
dependency('gstreamer-base-1.0', version: '>= 1.18')
dependency('gstreamer-plugins-base-1.0', version: '>= 1.18')
dependency('gstreamer-plugins-bad-1.0', version: '>= 1.18')

find_program('cargo', required: true)
find_program('glib-compile-resources', required: true)

glib_compile_schemas = find_program('glib-compile-schemas', required: true)
desktop_file_validate = find_program('desktop-file-validate', required: false)
appstream_util = find_program('appstream-util', required: false)

version = meson.project_version()
prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')

datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'
podir = meson.source_root () / 'po'
gettext_package = meson.project_name()
base_id = 'org.freedesktop.dabrain34.GstPipelineStudio'

if get_option('profile') == 'development'
  application_id = base_id + '.Devel'
  profile = 'Devel'
  name_prefix = '(Development) '
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD').stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format (vcs_tag)
  endif
else
  profile = ''
  version_suffix = ''
  name_prefix = ''
  application_id = base_id
endif

cargo_sources = files(
  'Cargo.toml',
  'Cargo.lock',
)

subdir('data')
subdir('src')
subdir('po')


meson.add_dist_script(
  'build-aux/dist-vendor.sh',
  meson.source_root(),
  join_paths(meson.build_root(), 'meson-dist', meson.project_name() + '-' + version)
)

meson.add_install_script('build-aux/meson/postinstall.py')
