//! On Windows, gives encore.exe its icon and the details Explorer shows (name, version).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=packaging/windows/encore.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("packaging/windows/encore.ico")
            .set("ProductName", "Encore")
            .set("FileDescription", "Encore");
        if let Err(error) = resource.compile() {
            panic!("could not embed the Windows icon: {error}");
        }
    }
}
