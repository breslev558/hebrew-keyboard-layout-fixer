fn main() {
    // icon, version information and manifest (asInvoker, PerMonitorV2, Common Controls v6)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=res/app.rc");
        println!("cargo:rerun-if-changed=res/app.manifest");
        println!("cargo:rerun-if-changed=res/klf.ico");
        println!("cargo:rerun-if-changed=res/klf-pause.ico");
        embed_resource::compile("res/app.rc", embed_resource::NONE).manifest_required().unwrap();
    }
}
