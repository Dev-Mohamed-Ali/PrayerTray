fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_resource::compile("prayertray.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
        println!("cargo:rerun-if-changed=prayertray.rc");
        println!("cargo:rerun-if-changed=prayertray.manifest");
        println!("cargo:rerun-if-changed=../app.ico");
    }
}
