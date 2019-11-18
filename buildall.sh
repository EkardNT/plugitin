set -x
set -e

$(cd cool_plugin/ && cargo build --target wasm32-unknown-unknown)

$(cd groovy_host && cargo build)
