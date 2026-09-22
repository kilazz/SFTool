## SFTool
Utility for **SpellForce 1/2** modding, written in Rust and powered by the Slint GUI framework.

## Features

### 📦 PAK Archive Management
* **Interactive TreeView:** Explore archive structures with a fully collapsible directory explorer.
* **Unpack:** Extract individual `.pak` files (auto-detects SpellForce 1/2 formats).
* **Pack:** Compile directories into valid SpellForce 1 and SpellForce 2 `.pak` archives.
* **Batch Operations:** Unpack all `.pak` archives in a folder, or pack folders ending in `_extracted` back to `.pak`.

### 🗄️ CFF Database Container Engine
* **Container Packaging:** Unpack `.cff` containers into raw `.dat` chunks, and pack them back with zlib compression.
* **Localization Exporter:** Auto-detect database schemas:
  * **Format A** (String Table — UTF-16LE),
  * **Format B** (Table-Based — Multi-string with parameters),
  * **Format C** (Developer Table — ANSI), and Fixed 566 structures.
* **Translation Suite:** Export text datasets to JSON files, and compile edited JSONs back into binary chunks.

### 📜 Lua 4.0 Scripting Suite
* **Decompile:** Batch decompile Lua 4.0 bytecode to readable `.lua` scripts.
* **Diagnostics:** Multi-threaded syntax checking via `luac4 -p`.
* **Formatter:** Auto-format code and indent scripts to match original game sources.

### 🔍 Binary Chunk Inspector
* **Scanner:** Search printable strings across raw `.dat` chunks.
* **Hex Modifier:** Read and edit values (Byte, Int16, Int32, Float32, String) directly in binary files.

### Compilation
* To compile and run the application:
```bash
cargo run
cargo build --release
```
