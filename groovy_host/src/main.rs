static WASM_BYTES: &'static [u8] = include_bytes!("../../cool_plugin/target/wasm32-unknown-unknown/debug/cool_plugin.wasm");

fn main() {
    println!("{}", plugitin::greeting());
}
