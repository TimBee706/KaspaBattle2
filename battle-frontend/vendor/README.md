# Vendor Dependencies

This folder contains pre-built dependencies that are necessary for the `battle-frontend` to compile without requiring the entire `rusty-kaspa` monorepo. 

## `kaspa-wasm`
Pre-compiled WebAssembly bindings for the Kaspa protocol (`kaspa-wasm`). 
It was decoupled from `file:../rusty-kaspa/wasm/web/kaspa` to enable standalone CI/CD deployments (like Vercel) where the parent rust repository is not available.

To rebuild this manually:
```bash
cd ../rusty-kaspa/wasm
./build-web.sh
cp -r web/kaspa ../../battle-frontend/vendor/kaspa-wasm
```
