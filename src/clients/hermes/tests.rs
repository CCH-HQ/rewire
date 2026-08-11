use super::*;

#[test]
fn windows_default_directory_uses_local_app_data_then_home_fallback() {
    let home = Path::new("C:/Users/fixture");
    assert_eq!(
        windows_default_directory(Some(std::ffi::OsStr::new("D:/HermesData")), home),
        Path::new("D:/HermesData/hermes")
    );
    assert_eq!(
        windows_default_directory(None, home),
        Path::new("C:/Users/fixture/AppData/Local/hermes")
    );
}
