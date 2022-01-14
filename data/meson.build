subdir('icons')

desktop_conf = configuration_data()
desktop_conf.set('icon', base_id)
desktop_file = configure_file(
  input: '@0@.desktop.in'.format(base_id),
  output: '@BASENAME@',
  configuration: desktop_conf
)

if desktop_file_validate.found()
  test(
    'validate-desktop',
    desktop_file_validate,
    args: [
      desktop_file
    ],
  )
endif

install_data(
  desktop_file,
  install_dir: datadir / 'applications'
)

# Appdata
appdata_conf = configuration_data()
appdata_conf.set('app-id', application_id)
appdata_conf.set('gettext-package', gettext_package)
appdata_file = i18n.merge_file (
  input: configure_file(
    input: '@0@.appdata.xml.in.in'.format(base_id),
    output: '@BASENAME@',
    configuration: appdata_conf
  ),
  output: '@0@.appdata.xml'.format(application_id),
  po_dir: podir,
  install: true,
  install_dir: datadir / 'metainfo'
)
# Validate Appdata
appstream_util = find_program('appstream-util', required: false)
if appstream_util.found()
  test(
    'validate-appdata', appstream_util,
    args: [
      'validate-relax', '--nonet', appdata_file.full_path()
    ]
  )
endif