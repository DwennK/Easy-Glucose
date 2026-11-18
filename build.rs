fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "Easy Glucose");
        resource.set("FileDescription", "Easy Glucose · Dexcom Share companion");
        if let Err(error) = resource.compile() {
            panic!("Failed to compile Windows resources: {error}");
        }
    }
}
