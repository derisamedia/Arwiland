fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/arwiland.ico");

    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon_with_id("assets/arwiland.ico", "1");
            res.set("CompanyName", "Siti Arwiyah,S. Kom");
            res.set("ProductName", "Arwiland");
            res.set("FileDescription", "Arwiland - Dynamic Island & Living Notch");
            res.set("LegalCopyright", "Copyright (c) Siti Arwiyah,S. Kom");
            res.set("OriginalFilename", "arwiland.exe");
            res.set("InternalName", "arwiland");
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
