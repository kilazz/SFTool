## 1. PAK Archive & Virtual File System (VFS)

### 1.1. SpellForce 1 — `MASSIVE PAKFILE V 4.0`
The Phenomic VFS mounts archives sequentially (`sf0.pak` through `sf35.pak` up to `sfXX.pak`). Archives mounted later override identically named files in earlier archives.

#### Binary Layout Overview
```text
+-----------------------------------------------------------------------+
| Offset 00..28  | Magic & Version ('MASSIVE PAKFILE V 4.0\r\n\0')      |
+-----------------------------------------------------------------------+
| Offset 28..72  | Static Phenomic Engine Header Template (44 bytes)    |
+-----------------------------------------------------------------------+
| Offset 72..76  | Custom IEEE 802.3 CRC32 Checksum (Little-Endian)      |
+-----------------------------------------------------------------------+
| Offset 76..80  | File Count (uint32)                                  |
+-----------------------------------------------------------------------+
| Offset 80..84  | Root Search Index (uint32, typically meshes.txt)     |
+-----------------------------------------------------------------------+
| Offset 84..88  | Data Section Start Offset (uint32)                   |
+-----------------------------------------------------------------------+
| Offset 88..92  | Total Archive Size (uint32, 4096-byte aligned)       |
+-----------------------------------------------------------------------+
| Offset 92..EOF | File Table -> String Table -> Data Payload           |
+-----------------------------------------------------------------------+
```

#### Core Engine Structures
```rust
// 44-byte Phenomic Runtime Header Template (Offsets 28..72)
const PHENOMIC_HEADER_TEMPLATE: [u8; 44] = [
    0x00, 0x00, 0x00, 0x00, 0xb0, 0xff, 0x12, 0x00, 0x08, 0x6f, 0x40, 0x00, 0x38, 0xc1, 0x40, 0x00,
    0xff, 0xff, 0xff, 0xff, 0x40, 0x28, 0x32, 0x00, 0x52, 0x48, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00,
    0xda, 0x31, 0x40, 0x00, 0x1f, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
];

// File Table Entry (16 bytes per file)
#[repr(C, packed)]
struct SF1FileEntry {
    file_size: u32,
    data_offset: u32,       // Relative to data_start_offset
    name_offset: u32,       // Low 24 bits: offset in String Table
    dir_offset: u32,        // Low 24 bits: directory string offset (0 or 0x00FFFFFF = root)
}
```

#### The In-Engine 16-Bit K&R Path Hash (`FUN_004a4180`)
Inside `SpellForce.exe`, file lookup is accelerated by prepending a 2-byte hash to each filename in the String Table:

$$\text{hash} = \left( \sum (\text{hash} \times 31 + \text{char}) \right) \mathbin{\text{AND}} \text{0xFFFF}$$

* **Sorting Comparator:** Binary search inside the archive requires records to be sorted strictly by:
  `hash_high_byte -> hash_low_byte -> reversed_path_string`.
* **String Table Alignment Bug (`D3DERR_INVALIDCALL`):**
  DirectX 8/9 vertex and index buffer upload routines expect file data offsets to be aligned to **4-byte DWORD boundaries**. If the String Table ends on an unaligned byte, the graphics driver crashes upon reading meshes.
  * **Fix:** The String Table must be padded with zeroes:

    $$\text{pad} = (4 - (\text{str\\_table\\_len} \pmod 4)) \pmod 4$$

#### Custom CRC32 Algorithm
Standard IEEE 802.3 polynomial (`0xEDB88320`), but **without final bitwise XOR inversion**:
```rust
fn calculate_sf1_crc(data: &[u8], prev_crc: u32) -> u32 {
    let mut crc = prev_crc;
    for &b in data {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ (b as u32)) & 0xFF) as usize];
    }
    crc
}
```

---

### 1.2. SpellForce 2 — `PAK\x01`
* **Signature:** `b"PAK\x01"` (4 bytes).
* **Header:**
  * `0x04`: Directory Offset (`u32`)
  * `0x08`: Uncompressed Directory Size (`u32`)
  * `0x0C`: Compressed Directory Size (`u32`)
* **Directory Structure:** Zlib-compressed stream containing:
  * `num_files: i32`
  * Repeated `num_files` times:
    * `name_length: i32`
    * `filename_bytes: [u8; name_length]` (Windows-1252/1251, lowercase, backslashes)
    * `offset: u32`
    * `next_offset: u32` (`size = next_offset - offset`)

---

## 2. CFF Database Container Engine

### 2.1. Container Signatures & Versions
1. **SpellForce 1 Classic (2003, *The Order of Dawn*):**
   * Signature: `0xDD72C502` (`b"\x02\xc5\x72\xdd"`).
2. **SpellForce 1 Platinum Edition (1.52–1.54):**
   * Signature: `0xDD72DD12` (`b"\x12\xdd\x72\xdd"`).
   * Verified by header check: `h2 == 2 && h3 == 2 && h4 == 1 && h5 == 0`.
3. **SpellForce 2 (v3.0):**
   * Signature: `0xDD72DD12` (`b"\x12\xdd\x72\xdd"`).
   * Version: `0x00000003` at offset 4, followed by 12 reserved zero bytes.

---

### 2.2. The 8,192 Chunk Limit (`CChunkFile` Object Layout)
Inside `SpellForce.exe`, `CGameDatabase::Load` allocates:

$$\text{Allocation Size} = \mathbf{\text{0x2803C}}\text{ bytes (163,900 bytes)}.$$

#### Architectural Deconstruction
* Base structure / VFS handles: `0x38` bytes.
* Chunk Table of Contents (TOC) array: `0x28000` bytes (163,840 bytes).
* Stride per TOC entry: **20 bytes**.

$$\text{Max Chunks} = \frac{163,840}{20} = \mathbf{8,192\text{ chunks}}.$$

* Offset `0x28038`: Active chunk count (`uint32`).
* Search algorithm (`FUN_005a4670`): Scans the 20-byte TOC array matching:
  `chunk_id == target_id && occurrence == 0 && c_type == target_type`.

---

### 2.3. Chunk Headers Specification

#### SpellForce 2 Chunk Header (16 bytes)
```rust
#[repr(C, packed)]
pub struct SF2ChunkHeader {
    pub chunk_id: u32,     // e.g. 0x234E (Abilities)
    pub schema_flag: u16,  // 0x0001
    pub comp_size: u32,    // Zlib payload size
    pub element_type: u16, // 1 = u8/bytes, 2 = u16/UTF16, 4 = u32/Int/Float (Csimbi mystery flag)
    pub uncomp_size: u32,  // Decompressed payload size
}
```

#### SpellForce 1 Chunk Header (12 bytes base + 4 bytes optional)
```rust
#[repr(C, packed)]
pub struct SF1ChunkHeader {
    pub chunk_id: u16,     // e.g. 0x07DC (2012)
    pub occurrence: i16,   // Usually 0
    pub comp_flag: i16,    // 0 = Uncompressed, 1 = Zlib compressed
    pub comp_size: i32,    // Size on disk
    pub c_type: i16,       // Standard = 1, EXCEPT 0x0800 where c_type == 3!
    // If comp_flag != 0:
    // pub uncomp_size: i32
}
```

---

### 2.4. Reversed SF1 Chunk Registry & Record Strides
Disassembled from `CGameDatabase::Load` (`FUN_0091a930`):

| Chunk ID (Hex) | Chunk ID (Dec) | Required `c_type` | Record Stride | Data Layout & In-Engine Function |
| :---: | :---: | :---: | :---: | :--- |
| **`0x07DC`** | 2012 | 1 | **69 bytes** | `u16 id, u8 flag, char[64] mesh, u16 extra` (`2dGfxItems`) |
| **`0x07E1`** | 2017 | 1 | **6 bytes** | `u16 key -> vector<u16, u8>` (`SpellMultiMap`) |
| **`0x07E2`** | 2018 | 1 | **4 bytes** | `u16 spell_id, u16 scroll_id` (`SpellsBiMap`) |
| **`0x07F7`** | 2039 | 1 | **4 bytes** | `u16 key, u16 val` (`TextDialogueMap`) |
| **`0x07FC`** | 2044 | 1 | **3 bytes** | `u8 key, u16 val` (`TypeCategoryMap`) |
| **`0x07FF`** | 2047 | 1 | **5 bytes** | `u16 key -> vector<u16, u8>` (`EntityLinkMap`) |
| **`0x0800`** | 2048 | **3** ⚠️ | **15 bytes** | `u8 key, u32 f1, u32 f2, u16 f3, u32 f4` (`ComplexProperties`) |
| **`0x0801`** | 2049 | 1 | **2 bytes** | Flat array of `u16` words (`WordValuesArray`) |
| **`0x080A`** | 2058 | 1 | **4 bytes** | `u16 str_id, u16 text_id` (`LocalizedStringIds`) |
| **`0x080B`** | 2059 | 1 | **6 bytes** | `u32 id, u16 param` (`AudioSpeechParams`) |
| **`0x080E`** | 2062 | 1 | **9 bytes** | `u16 compound_key, u32 f1, u16 f2, u8 f3` |
| **`0x0818`** | 2072 | 1 | **4 bytes** | `u8/u16 key, u16 val` (`SystemLookupMap` / Cascade terminator) |

---

## 3. Localized Strings & The `Fixed566` Engine

### 3.1. Block Layout (566 bytes per record)
* `0x00..0x04` (`4 bytes`): `str_id` (32-bit unsigned integer).
* `0x04..0x36` (`50 bytes`): Metadata / Reserved flags.
* `0x36..0x236` (`512 bytes`): Null-terminated string buffer.

### 3.2. Mathematical Bitmask Architecture of `str_id`

$$\text{str\\_id} = (\text{CampaignID} \ll 24) \mid (\text{LanguageID} \ll 16) \mid \text{BaseID}$$

```rust
let base_id = (str_id & 0xFFFF) as u16;          // Bits 0..15: String index
let language_id = ((str_id >> 16) & 0xFF) as u8; // Bits 16..23: Language Slot
let campaign_id = (str_id >> 24) as u8;          // Bits 24..31: Campaign / Addon
```

#### Language Slots Table
| Slot | Tag | Language | Primary Encoding |
| :---: | :---: | :--- | :--- |
| **0** | `DE` | German | Windows-1252 |
| **1** | `EN` | English | Windows-1252 |
| **2** | `FR` | French | Windows-1252 |
| **3** | `ES` | Spanish | Windows-1252 |
| **4** | `IT` | Italian | Windows-1252 |
| **5+** | `RU` / `AR` / `PL` | Custom / Community | Windows-1251 / Custom |

#### Campaign IDs
* `0`: *The Order of Dawn* (Base Game)
* `1`: *The Breath of Winter* (Addon 1) — previously misidentified as `EXT` due to missing `& 0xFF` mask (`str_id >> 16 == 256`).
* `2`: *Shadow of the Phoenix* (Addon 2).

### 3.3. Encoding Collision Prevention
* **The Bug:** Characters in range `0xC0..=0xFF` represent Cyrillic in `Windows-1251`, but Western European accented letters and umlauts (`ä, ö, ü, ß, é, è, ê`) in `Windows-1252`.
* **Resolution Rule:**
  * Slots `0..=4` must strictly be decoded via **Windows-1252**.
  * Slot `5` (or user text containing Cyrillic characters `\u{0400}..=\u{04FF}`) is encoded via **Windows-1251**.

---

## 4. Dual Engine Loaders Architecture

Inside `SpellForce.exe`, `GameData.cff` is loaded by two separate subsystems:

```text
                           data\GameData.cff
                                  │
         ┌────────────────────────┴────────────────────────┐
         ▼                                                 ▼
   FUN_0091a930                                      FUN_00a6eab0
   Master Gameplay Database                          Sound & Speech Engine
   ├── 2dGfxItems (0x07DC)                           ├── Executes 'sound/speech/langid.lua'
   ├── SpellsBiMap (0x07E2)                          ├── Reads 'LangId' & 'ClientLanguage'
   ├── LocalizedStringIds (0x080A)                   └── Mounts localized speech & audio banks
   └── 15-step sequential cascade
```

---

## 5. Visual Asset Linkage Pipeline

### 5.1. Item Presentation (`2dGfxItems` 0x07DC)
Each item connects to graphic assets through its 69-byte descriptor:
```text
Item Record (ItemID)
     │
     └── 2dGfxItems [ItemID, Flag]
              │
              ├── Flag 1: Inventory Scroll / Item Icon (ui_item_...msh -> .dds)
              └── Flag 2: Spellbook / Action Bar Icon (ui_spell_...msh -> .dds)
```

### 5.2. Spell Linking (`SpellsBiMap` 0x07E2)
Spell casting logic is strictly decoupled from inventory scrolls:

$$\text{Spell ID (Combat Spell Entity)} \xleftrightarrow[\text{0x07E2}]{\text{BiMap}} \text{Scroll ID (Inventory Item Entity)}$$

---

## 6. Historical Context & The Patch 1.61 Incident

1. **The 2018 Steam Crisis:** Valve notified THQ Nordic that digital CD-keys for *SpellForce 1* were exhausted, threatening immediate store delisting unless the legacy SecuROM/JoWooD DRM key check was excised.
2. **Missing v1.54 Source Code:** THQ Nordic's acquisition of bankrupt JoWooD's servers yielded only the **v1.50** source tree (build 60653, summer 2005). The final v1.54 sources produced by EA Phenomic for *Platinum Edition* remained locked in EA's LTO tape archives.
3. **The 1.61 Regression:** Patch 1.61 was branched off v1.50, inadvertently resurrecting 15-year-old bugs already resolved in 1.54 (Farlorn's Hope spawn bug, missing health bar rendering on modern GPUs, savegame format breakage).
